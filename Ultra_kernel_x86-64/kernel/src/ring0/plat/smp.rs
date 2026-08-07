//! **Despertar los otros núcleos.** El bring-up de SMP, del lado bueno de la
//! frontera.
//!
//! ═══ Por qué esto está aquí y no en `s1_cpu` ═══
//!
//! Había un `smp_startup()` en `faggin/s1_cpu` que **nunca se llamó**, y al ir a
//! llamarlo se vio que no estaba sólo sin usar: estaba **del lado equivocado de
//! `ExitBootServices`**.
//!
//! Antes de EBS el firmware sigue vivo y **los otros núcleos son suyos** — UEFI
//! los tiene aparcados en su propio *MP Services*. Mandarles INIT+SIPI por
//! debajo es quitárselos a alguien que todavía trabaja, y lo siguiente que hacía
//! `s1_cpu` era volver a llamar al firmware. Además escribía en `0x7000`, que
//! antes de EBS tampoco es nuestro, y **hablaba sólo por `ser_print!`**: en esta
//! máquina no hay cable serie, así que se habría llamado y no se habría visto
//! nada.
//!
//! Aquí, en el kernel, no hay firmware: la máquina es entera de BMO, la memoria
//! baja **ya está reservada** (`phys::init` reserva `<1 MiB` con el comentario
//! *"future SMP trampoline lives here"*) y CABINA pinta en pantalla.
//!
//! ═══ ⚠️ Y por qué NO se copió el código viejo ═══
//!
//! Porque leído de cerca **no funcionaba**, y conviene que quede escrito para
//! que nadie lo rescate:
//!
//! 1. **El trampolín estaba ensamblado como código de 64 bits** (`mov rax, …`,
//!    `retfq`) y un AP arranca en **modo real de 16 bits**. Ahí un prefijo REX
//!    no existe: `0x48` es `dec ax`. Ejecutaba basura desde la primera
//!    instrucción.
//! 2. **Las tablas de páginas se pisaban entre ellas**: PML4 en `0x7000` ocupa
//!    4 KiB —hasta `0x8000`— y el PDPT se ponía en `0x7100`, dentro. Y el bucle
//!    escribía 2048 entradas desde `0x7200`, que se comen el trampolín de
//!    `0x8000` y los datos de `0x8100`.
//! 3. **El contador de vivos vivía en `0x7FF8`**, dentro de la propia PML4 que
//!    el paso anterior ponía a cero.
//! 4. La GDT no tenía **segmento de datos de 32 bits**: usaba el selector
//!    `0x18`, que en esa tabla era el código de 64 bits.
//!
//! ═══ Lo que hace esta versión, y lo que se apoya en lo que ya existe ═══
//!
//! No construye tablas de páginas: **usa las del kernel**. El kernel ya
//! identity-mapea `0..32 MiB`, que es todo lo que el trampolín necesita ver, así
//! que el AP carga el mismo `CR3` que el BSP y llega a 64 bits dentro del mismo
//! espacio de direcciones. Un problema menos y, sobre todo, **una tabla menos
//! que pueda quedarse desincronizada**.
//!
//! ═══ ⚠️ SIN PROBAR EN METAL ═══
//!
//! Esto es código que corre antes de que exista nada, en el único CPU que hay.
//! Por eso **no se llama en el arranque**: se pide a mano con la orden `smp`. El
//! arranque queda exactamente igual de fiable que ayer, y si el trampolín está
//! mal lo que se cuelga es un comando, no la máquina al encenderla. La salida es
//! un reinicio a botón.

use core::sync::atomic::{AtomicU32, Ordering};

use crate::ring0::mm::HIGH_MEM_BASE;

// ── El mapa de la memoria baja que usamos ────────────────────────────
//
// Todo dentro del primer MiB, que `phys::init` reserva entero. Las páginas
// están separadas a propósito: el fallo del código viejo fue solaparlas.

/// El trampolín copiado. La SIPI lleva vector `0x08` → el AP empieza aquí.
const TRAMPOLIN: u64 = 0x8000;
/// Los datos que el BSP deja para el AP. Página distinta de la del código.
const DATOS: u64 = 0x9000;
/// Pila temporal de 32 bits, en la cola de la página de datos.
///
/// Sólo la usa el trampolín, que la lleva escrita a mano porque no puede llamar
/// a nada. Está aquí para que **el mapa de la página esté completo en un sitio**:
/// si alguien mueve `DATOS`, este número también se mueve.
#[allow(dead_code)]
const PILA_TMP: u32 = 0x9FF0;
/// Primera pila de AP. 4 KiB cada una, hacia arriba.
const PILAS: u64 = 0xA000;

// Desplazamientos dentro de `DATOS`. Los mismos números están escritos a mano
// en el trampolín, que no puede llamar a nada: si se toca uno, se tocan los dos.
const OFF_GDTR: u64 = 0x00; // limit u16 + base u64
const OFF_IDTR: u64 = 0x10; // limit u16 + base u64
const OFF_CR3: u64 = 0x20;
const OFF_ENTRADA: u64 = 0x28;
const OFF_PILA: u64 = 0x30;
const OFF_GDT: u64 = 0x40; // la tabla en sí, 5 entradas

/// La GDT que usa el AP para subir de 16 a 64 bits.
///
/// Cinco entradas y **cada una hace falta**: la de datos de 32 bits es la que
/// no estaba en la versión vieja.
const GDT: [u64; 5] = [
    0,                        // 0x00 null
    0x0000_9B00_0000_FFFF,    // 0x08 código 16-bit
    0x00CF_9A00_0000_FFFF,    // 0x10 código 32-bit
    0x00CF_9200_0000_FFFF,    // 0x18 datos 32-bit
    0x0020_9B00_0000_0000,    // 0x20 código 64-bit (L=1)
];

core::arch::global_asm!(
    r#"
.section .text.smp_tramp,"ax"
.globl smp_tramp_ini
.globl smp_tramp_fin

// El AP entra AQUI en modo real de 16 bits, con CS:IP = 0x0800:0x0000.
// Todo lo que toca son direcciones absolutas conocidas: este codigo se ejecuta
// copiado en 0x8000, no donde el enlazador lo puso.
.code16
smp_tramp_ini:
    cli
    xor ax, ax
    mov ds, ax
    mov es, ax
    mov ss, ax

    // lgdt con base de 32 bits: hace falta el prefijo de tamaño de operando.
    .byte 0x66
    lgdt [0x9000]

    // Modo protegido.
    mov eax, cr0
    or eax, 1
    mov cr0, eax

    // Salto lejano a 32 bits: 0x66 0xEA imm32 imm16. Se escribe a mano porque
    // el destino es "donde va a estar copiado", no donde se ensambla.
    .byte 0x66, 0xEA
    .long 0x8000 + (smp_pm32 - smp_tramp_ini)
    .word 0x10

.code32
smp_pm32:
    mov ax, 0x18
    mov ds, ax
    mov es, ax
    mov ss, ax
    mov esp, 0x9FF0

    // PAE, que el modo largo exige.
    mov eax, cr4
    or eax, 0x20
    mov cr4, eax

    // ★ El CR3 DEL KERNEL. No se construye una tabla nueva: la del kernel ya
    // identity-mapea 0..32 MiB, que es todo lo que hay que ver desde aqui.
    mov eax, [0x9020]
    mov cr3, eax

    // EFER: LME (modo largo) + NXE. NXE va ANTES de cargar paginacion porque
    // las tablas del kernel usan el bit NX, y sin NXE ese bit esta "reservado"
    // y el primer acceso seria un #PF que nadie sabria explicar.
    mov ecx, 0xC0000080
    rdmsr
    or eax, 0x900
    wrmsr

    // Paginacion: aqui se activa el modo largo de verdad.
    mov eax, cr0
    or eax, 0x80000000
    mov cr0, eax

    // Salto lejano a 64 bits: 0xEA imm32 imm16.
    .byte 0xEA
    .long 0x8000 + (smp_lm64 - smp_tramp_ini)
    .word 0x20

.code64
smp_lm64:
    mov rax, 0x9010
    lidt [rax]
    mov rsp, [0x9030]
    mov rax, [0x9028]
    jmp rax

smp_tramp_fin:
.code64
"#
);

unsafe extern "C" {
    static smp_tramp_ini: u8;
    static smp_tramp_fin: u8;
}

// ── Lo que el AP hace al llegar ──────────────────────────────────────

/// Cuántos APs han contestado.
static VIVOS: AtomicU32 = AtomicU32::new(0);
/// Qué APIC IDs contestaron, un bit cada uno.
///
/// El número solo no sirve: *"despertaron 4 de 5"* deja sin decir **cuál falta**,
/// y cuál falta es justo el dato que se necesita para mirar el siguiente.
static MASCARA: AtomicU32 = AtomicU32::new(0);

/// El APIC ID **por CPUID**, no por LAPIC.
///
/// A propósito: la MMIO del LAPIC vive en `0xFEE0_0000` y el kernel sólo la
/// alcanza por el physmap, en la mitad alta. Un AP recién llegado no tiene por
/// qué poder tocarla, y `CPUID.1:EBX[31:24]` da el mismo dato **sin un solo
/// acceso a memoria**.
fn apic_id() -> u32 {
    let ebx: u32;
    unsafe {
        core::arch::asm!(
            "push rbx",
            "cpuid",
            "mov {0:e}, ebx",
            "pop rbx",
            out(reg) ebx,
            inout("eax") 1u32 => _,
            out("ecx") _,
            out("edx") _,
            options(nostack),
        );
    }
    ebx >> 24
}

/// Donde aterriza el AP, ya en 64 bits. Se apunta y se para.
///
/// No toca **nada** del kernel: ni CABINA, ni el planificador, ni un driver.
/// Sólo dos atómicas. Es el contrato del documento `SMP_MAESTRO.md` — un obrero
/// que no comparte estado no puede correr una carrera, y por eso esto es seguro
/// con los 209 `static mut` que hay ahí fuera.
#[unsafe(no_mangle)]
pub extern "C" fn smp_ap_entrada() -> ! {
    let id = apic_id();
    MASCARA.fetch_or(1u32 << (id & 31), Ordering::SeqCst);
    VIVOS.fetch_add(1, Ordering::SeqCst);
    loop {
        unsafe { core::arch::asm!("cli; hlt", options(nomem, nostack)) };
    }
}

// ── El LAPIC del BSP ─────────────────────────────────────────────────

fn lapic() -> u64 {
    let lo: u32;
    let hi: u32;
    unsafe {
        core::arch::asm!("rdmsr", in("ecx") 0x1Bu32, out("eax") lo, out("edx") hi,
                         options(nomem, nostack));
    }
    let fisica = (((hi as u64) << 32) | lo as u64) & 0x000F_FFFF_FFFF_F000;
    // Por el physmap, igual que `timer.rs`: la MMIO no está en el identity map.
    HIGH_MEM_BASE + fisica
}

unsafe fn icr(destino: u32, orden: u32) {
    let base = lapic();
    unsafe {
        // ICR alto primero (el destino), y el bajo AL FINAL: escribir el bajo
        // es lo que DISPARA la IPI.
        core::ptr::write_volatile((base + 0x310) as *mut u32, destino << 24);
        core::ptr::write_volatile((base + 0x300) as *mut u32, orden);
        // Esperar a que se envíe (bit 12 = Delivery Status).
        let mut vueltas = 0u32;
        while core::ptr::read_volatile((base + 0x300) as *const u32) & (1 << 12) != 0
            && vueltas < 100_000
        {
            vueltas += 1;
            core::hint::spin_loop();
        }
    }
}

fn esperar_us(us: u64) {
    let hz = crate::ring0::task::scheduler::tsc_freq();
    if hz == 0 {
        for _ in 0..200_000 {
            core::hint::spin_loop();
        }
        return;
    }
    let hasta = crate::ring0::task::scheduler::rdtsc() + (hz / 1_000_000) * us;
    while crate::ring0::task::scheduler::rdtsc() < hasta {
        core::hint::spin_loop();
    }
}

// ── El bring-up ──────────────────────────────────────────────────────

/// Despierta a los demás núcleos y cuenta cuántos contestan.
///
/// Devuelve `(vivos, esperados)`. **No se llama en el arranque**: la pide la
/// orden `smp` del shell. Ver la cabecera del módulo.
pub fn despertar() -> (u32, u32) {
    let topo = crate::ring0::cpu_vendor::ryzen_5_5600x::bmo_cpu::topology();
    let esperados = match topo {
        Some(t) if t.total_threads > 1 => t.total_threads - 1,
        _ => {
            crate::ring0::cabina::warn("smp", "sin topologia: no se cuantos nucleos esperar", 0);
            return (0, 0);
        }
    };

    VIVOS.store(0, Ordering::SeqCst);
    MASCARA.store(0, Ordering::SeqCst);

    let yo = apic_id();

    unsafe {
        // 1. El trampolín, a 0x8000.
        let ini = core::ptr::addr_of!(smp_tramp_ini) as *const u8;
        let fin = core::ptr::addr_of!(smp_tramp_fin) as *const u8;
        let largo = fin as usize - ini as usize;
        if largo == 0 || largo > 0x1000 {
            crate::ring0::cabina::fault("smp", "el trampolin mide algo imposible", largo as u64);
            return (0, esperados);
        }
        for i in 0..largo {
            core::ptr::write_volatile(
                (TRAMPOLIN as *mut u8).add(i),
                core::ptr::read_volatile(ini.add(i)),
            );
        }

        // 2. La GDT, dentro de la página de datos.
        let gdt = (DATOS + OFF_GDT) as *mut u64;
        for (i, e) in GDT.iter().enumerate() {
            core::ptr::write_volatile(gdt.add(i), *e);
        }
        // GDTR: apunta a la copia de abajo, no a la del kernel — en modo real
        // la base es una dirección FÍSICA y tiene que caber en 32 bits.
        core::ptr::write_volatile((DATOS + OFF_GDTR) as *mut u16, (GDT.len() * 8 - 1) as u16);
        core::ptr::write_volatile((DATOS + OFF_GDTR + 2) as *mut u64, DATOS + OFF_GDT);

        // 3. La IDT: la del kernel, tal cual la tiene puesta el BSP ahora mismo.
        let mut idtr = [0u8; 10];
        core::arch::asm!("sidt [{}]", in(reg) idtr.as_mut_ptr(), options(nostack));
        core::ptr::copy_nonoverlapping(idtr.as_ptr(), (DATOS + OFF_IDTR) as *mut u8, 10);

        // 4. CR3 del kernel y la entrada de 64 bits.
        core::ptr::write_volatile((DATOS + OFF_CR3) as *mut u64, crate::ring0::mm::vmm::kernel_pml4());
        core::ptr::write_volatile(
            (DATOS + OFF_ENTRADA) as *mut u64,
            smp_ap_entrada as *const () as u64,
        );

        // 5. Y a llamar, de uno en uno. Cada AP recoge SU pila del mismo sitio,
        //    así que hay que dejarla puesta antes de cada SIPI y esperar a que
        //    la lea. De ahí que esto no sea un broadcast.
        for id in 0..esperados + 1 {
            if id == yo {
                continue;
            }
            core::ptr::write_volatile(
                (DATOS + OFF_PILA) as *mut u64,
                PILAS + (id as u64 + 1) * 0x1000,
            );

            // INIT, esperar 10 ms, y dos SIPI con vector 0x08 (= 0x8000).
            icr(id, 0x0000_4500);
            esperar_us(10_000);
            icr(id, 0x0000_4608);
            esperar_us(200);
            icr(id, 0x0000_4608);
            esperar_us(200);
        }
    }

    // 6. Contar, con tope. Un AP que no viene no puede colgar al que pregunta.
    let mut vueltas = 0u32;
    while VIVOS.load(Ordering::SeqCst) < esperados && vueltas < 1000 {
        esperar_us(1000);
        vueltas += 1;
    }

    let vivos = VIVOS.load(Ordering::SeqCst);
    let mascara = MASCARA.load(Ordering::SeqCst);

    if vivos == esperados {
        crate::ring0::cabina::info("smp", "todos los nucleos contestaron", vivos as u64 + 1);
    } else {
        crate::ring0::cabina::warn("smp", "faltan nucleos por contestar", (esperados - vivos) as u64);
        crate::ring0::cabina::warn("smp", "mascara de los que SI contestaron", mascara as u64);
    }
    (vivos, esperados)
}

/// Los que contestaron, para quien quiera pintarlo sin volver a despertar nada.
pub fn vivos() -> (u32, u32) {
    (VIVOS.load(Ordering::SeqCst), MASCARA.load(Ordering::SeqCst))
}
