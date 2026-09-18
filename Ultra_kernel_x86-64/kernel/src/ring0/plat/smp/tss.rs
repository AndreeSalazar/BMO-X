//! **LA GDT Y EL TSS DE CADA OBRERO**: lo que hace falta para que un AP pueda
//! tomar una excepcion sin reiniciar la maquina.
//!
//! [carril]  ROJO      descriptores que el CPU lee al entrar en un fallo. Uno
//!                     mal puesto no da un fallo: da un TRIPLE FALLO
//! [consumo] NADA      corre una vez por obrero, al levantarse
//!
//! [cuesta]  MAQUINA -- si el TSS o la GDT de un obrero estan mal, el primer
//!           fallo de ese obrero reinicia el PC entero, que es lo que pasaba
//!           antes de este fichero (L6e)
//!
//! [riesgo]  UNICO -- cada obrero tiene SU tabla y SU pila, indexadas por su
//!           orden de llegada. Dos obreros con el mismo indice compartirian
//!           pila de fallo; el indice sale de un `fetch_add`, asi que no puede
//!           pasar, y por eso este es el unico riesgo que se declara (L6f)
//!
//! # *** POR QUE EXISTE (2026-09-18, A1.1 de PLAN_EL_BUS_APARTE)
//!
//! `tramp.rs` lo dejo escrito: *"un AP que toma una excepcion esta muerto de
//! una forma fea"*. Al ir a arreglarlo salieron TRES motivos, no dos, y el
//! peor no estaba en la lista:
//!
//! ```text
//!   1. la IDT del kernel manda cada fallo al selector 0x08 -- y en la GDT
//!      del trampolin 0x08 es CODIGO DE 16 BITS. Cargar eso en modo largo
//!      es #GP, y el #GP vuelve a cargarlo: triple fallo. NADIE lo sabia.
//!   2. los cuatro vectores que importan (#UD #DF #GP #PF) usan IST1, y el
//!      IST vive en el TSS -- y un AP no tenia TSS (TR = 0). El CPU no
//!      puede cambiar de pila: doble fallo, que tambien usa IST1: triple.
//!   3. sin CR4.OSXSAVE, un `xsave64` es #UD (`tramp.rs`). El camino del
//!      fallo ya no hace `xsave64` al entrar, pero se enciende igual, en
//!      `smp_ap_entrada`, con el XCR0 que el BSP ya midio.
//! ```
//!
//! Con esto, un obrero que hace `ud2` llega a `fault_dispatch`, y ahi hay una
//! rama para el: apunta en su ficha que fallo y se para SOLO. El BSP sigue,
//! el escritorio sigue, y `smp` lo cuenta. Es la prueba A1.2 (`smp tropezar`).
//!
//! # Lo que NO hace
//!
//! No le da al obrero un GS por-CPU ni pila de syscall: un obrero no entra
//! por SYSCALL ni recibe interrupciones (corre con IF=0). El dia que un AP
//! ejecute Ring 3 --el sub-director entero-- hara falta `PER_CPUS[i]` y
//! `rsp0`; esto deja el hueco (`rsp[0]`) a cero a proposito, para que ese dia
//! el primer trap desde Ring 3 falle DICIENDOLO y no escribiendo en la pila 0.

use core::sync::atomic::{AtomicU32, Ordering};

use super::ficha::MAX_OBREROS;

/// La pila de fallo de cada obrero. Cuatro KiB: la rama de un AP en
/// `fault_dispatch` son unas atomicas, no una pantalla azul.
const IST_BYTES: usize = 4096;

/// Los mismos selectores que el BSP (`faggin/s1_cpu/src/descriptors.rs`):
/// los stubs de la IDT comparan `cs` con `0x08` para saber si el fallo vino
/// del kernel, y un obrero es kernel.
pub const KERNEL_CS: u16 = 0x08;
pub const KERNEL_DS: u16 = 0x10;
pub const TSS_SEL: u16 = 0x28;

#[repr(C, packed)]
struct Tss {
    _r0: u32,
    rsp: [u64; 3],
    _r1: u64,
    ist: [u64; 7],
    _r2: u64,
    _r3: u16,
    iomap_base: u16,
}

const TSS_VACIO: Tss = Tss { _r0: 0, rsp: [0; 3], _r1: 0, ist: [0; 7], _r2: 0, _r3: 0, iomap_base: 0 };

#[repr(C, align(16))]
struct Gdt([u64; 7]);

#[repr(C, packed)]
struct Gdtr {
    limit: u16,
    base: u64,
}

/// Solo la toca el CPU, al entrar en un fallo: el compilador nunca la lee.
#[repr(align(16))]
struct Pila(#[allow(dead_code)] [u8; IST_BYTES]);

static mut TSS: [Tss; MAX_OBREROS] = [TSS_VACIO; MAX_OBREROS];
static mut GDT: [Gdt; MAX_OBREROS] = [const { Gdt([0; 7]) }; MAX_OBREROS];
static mut IST1: [Pila; MAX_OBREROS] = [const { Pila([0; IST_BYTES]) }; MAX_OBREROS];

/// Cuantos obreros cargaron su TSS. Si es menor que los que entraron, alguno
/// llego con un indice fuera de tabla y ESE sigue sin poder fallar.
static CARGADOS: AtomicU32 = AtomicU32::new(0);

pub fn cargados() -> u32 {
    CARGADOS.load(Ordering::Relaxed)
}

/// Un descriptor de segmento plano de 64 bits. Copia de `s1_cpu`: el kernel
/// no enlaza esa etapa, y son seis lineas.
fn segmento(dpl: u8, codigo: bool) -> u64 {
    let mut d: u64 = 0xFFFF | (0x0F << 48);
    let mut a: u8 = 0x92 | (dpl << 5);
    if codigo {
        a |= 0x08;
    }
    d |= (a as u64) << 40;
    let f: u8 = if codigo { 0x0A } else { 0x0C };
    d |= (f as u64) << 52;
    d
}

/// El descriptor de sistema del TSS (16 bytes: dos entradas de la GDT).
fn descriptor_tss(addr: u64, size: u16) -> (u64, u64) {
    let mut lo: u64 = (size as u64) & 0xFFFF;
    lo |= (((size as u64) >> 16) & 0x0F) << 48;
    lo |= ((addr & 0xFFFF) as u64) << 16;
    lo |= (((addr >> 16) & 0xFF) as u64) << 32;
    lo |= (((addr >> 24) & 0xFF) as u64) << 56;
    lo |= 0x89u64 << 40;
    let hi: u64 = (addr >> 32) & 0xFFFF_FFFF;
    (lo, hi)
}

/// **Cargar la GDT y el TSS de este obrero.** Lo llama el propio obrero, en
/// su nucleo, nada mas aterrizar en 64 bits y antes de tocar nada.
///
/// `false` si el indice no cabe: entonces el obrero sigue con la GDT del
/// trampolin, y un fallo suyo sigue siendo el PC entero. Se cuenta.
///
/// # Safety
/// Cambia GDTR, CS, DS/ES/SS y TR del nucleo que lo ejecuta. Solo desde
/// `smp_ap_entrada`, una vez.
pub unsafe fn cargar(indice: u32) -> bool {
    let i = indice as usize;
    if i >= MAX_OBREROS {
        return false;
    }
    let tss = &mut *core::ptr::addr_of_mut!(TSS[i]);
    let ist = core::ptr::addr_of!(IST1[i]) as u64 + IST_BYTES as u64;
    tss.ist[0] = ist;
    // `rsp[0]` a cero a proposito: ver la cabecera.
    tss.iomap_base = core::mem::size_of::<Tss>() as u16;

    let gdt = &mut *core::ptr::addr_of_mut!(GDT[i]);
    gdt.0[0] = 0;
    gdt.0[1] = segmento(0, true);
    gdt.0[2] = segmento(0, false);
    gdt.0[3] = segmento(3, false);
    gdt.0[4] = segmento(3, true);
    let (lo, hi) = descriptor_tss(tss as *const Tss as u64, (core::mem::size_of::<Tss>() - 1) as u16);
    gdt.0[5] = lo;
    gdt.0[6] = hi;

    let gdtr = Gdtr { limit: (core::mem::size_of::<Gdt>() - 1) as u16, base: gdt as *const Gdt as u64 };
    // `lgdt` solo no recarga CS: el nucleo seguiria con el 0x20 del
    // trampolin cacheado, y el primer fallo cargaria 0x08 de ESTA tabla
    // mientras `cs` guardado diria 0x20. El `retfq` lo deja en 0x08 aqui.
    core::arch::asm!(
        "lgdt [{gdtr}]",
        "push {kcs}",
        "lea {tmp}, [rip + 55f]",
        "push {tmp}",
        "retfq",
        "55:",
        gdtr = in(reg) &gdtr,
        kcs = in(reg) KERNEL_CS as u64,
        tmp = out(reg) _,
    );
    core::arch::asm!(
        "mov ds, {0:x}", "mov es, {0:x}", "mov ss, {0:x}", "mov fs, {0:x}", "mov gs, {0:x}",
        in(reg) KERNEL_DS as u64,
    );
    core::arch::asm!("ltr {0:x}", in(reg) TSS_SEL as u64);
    CARGADOS.fetch_add(1, Ordering::Relaxed);
    true
}
