//! Virtual memory: address spaces built on the `s2_mem` physmap.
//!
//! Every user address space is a private PML4 that shares the kernel half
//! (PML4[256..512] -- the physmap and future kernel higher-half mappings) and
//! owns a private PDPT under PML4[0]. Entry 0 of that PDPT is a copy of the
//! kernel's supervisor identity map (0..32 MiB); everything the user touches
//! lives at PDPT index >= 1 so the shared identity tables are never polluted.
//!
//! Layout contract (BMO ABI, stable):
//! ```text
//!   1 GiB  USER_IMAGE_BASE   BEX sections (code/rodata/data/bss)
//!   2 GiB  USER_STACK_TOP    user stack, grows down (mapped in F2)
//!   3 GiB  CHANNEL_VA_BASE   16 BMO Channel pages, U/S shared with Ring 0
//! ```

use super::{phys, phys_to_virt, PAGE};

pub const PTE_PRESENT: u64 = 1 << 0;
pub const PTE_WRITABLE: u64 = 1 << 1;
pub const PTE_USER: u64 = 1 << 2;
pub const PTE_HUGE: u64 = 1 << 7;
/// **Esta hoja es NUESTRA: liberarla al destruir el espacio de direcciones.**
///
/// El bit 9 es uno de los tres que la arquitectura deja libres al sistema
/// operativo (9, 10 y 11) -- el hardware no lo mira nunca.
///
/// # Por que un bit y no una lista
///
/// Al morir un proceso hay que devolver sus marcos, y hasta el 14-08 no se
/// devolvia ninguno: la imagen, la pila de usuario y las tablas se quedaban
/// puestas para siempre. La pregunta que lo bloqueaba no era "donde estan" sino
/// **"cuales son mios"** -- porque en el mismo espacio conviven el framebuffer
/// (que es MMIO: devolverlo al asignador de RAM es corrupcion) y los marcos
/// prestados, que por diseno sobreviven al que los presto.
///
/// La salida obvia era llevar una lista por proceso. Pero **la tabla de paginas
/// YA ES esa lista**: tiene todos los marcos, uno por entrada, mantenida por el
/// hardware y sin poder desincronizarse. Lo unico que le faltaba era una columna
/// que dijera de quien es cada uno. Es una columna mas en una tabla que ya
/// existe, no una estructura nueva que mantener en dos sitios.
///
/// # Y el valor por defecto es NO, a proposito
///
/// [`map_page`] no lo pone; hay que pedirlo con [`map_page_propia`]. O sea que
/// olvidarse de marcar algo **fuga un marco**, y marcar de mas **lo libera dos
/// veces**. Lo primero se ve en `mem` y se arregla; lo segundo entrega memoria
/// viva a otro programa y el fallo aparece tres arranques despues y en otro
/// sitio. La duda se resuelve por el lado que solo cuesta RAM.
pub const PTE_NUESTRA: u64 = 1 << 9;
/// * En una PTE de 4 KiB el bit 7 **no** es "pagina grande": es el bit alto
/// del indice de PAT. Con `PWT`(3) y `PCD`(4) a cero, ponerlo selecciona la
/// entrada **4** de la tabla -- la que `s1_cpu` deja en Write-Combining.
///
/// El mismo numero significa dos cosas distintas segun el nivel de tabla, y
/// por eso lleva nombre propio: en una PDE seria `PS` y convertiria la entrada
/// en una pagina de 2 MiB.
pub const PTE_PAT_4K: u64 = 1 << 7;
const ADDR_MASK: u64 = 0x000F_FFFF_FFFF_F000;

pub const USER_IMAGE_BASE: u64 = 0x0000_0000_4000_0000;
pub const USER_STACK_TOP: u64 = 0x0000_0000_8000_0000;
/// **Lo que de verdad se mapea de pila a un proceso de Ring 3.**
///
/// [!] Esta constante decia `0x10_0000` (1 MiB) y **no la usaba nadie**:
/// `proc.rs` mapeaba 16 paginas por su cuenta. O sea que el kernel declaraba un
/// megabyte y entregaba sesenta y cuatro kilobytes, y nada en el arbol podia
/// contradecirlo porque los dos numeros no se cruzaban en ningun sitio.
///
/// Se cobro el 2026-08-14: el compositor paso a tener un marco de 94.208 bytes
/// y murio en la sonda de pila con `#PF` en `rip=0x4000001B`, sin escribir una
/// linea. Ahora hay **un solo numero**: `proc.rs` deriva sus paginas de aqui y
/// la autopsia compara contra el mismo limite. El valor no cambia -- lo que
/// cambia es que ya no puede mentir.
pub const USER_STACK_SIZE: u64 = 0x1_0000;
/// El byte mas bajo de la pila que EXISTE. Por debajo de esto no hay pagina, y
/// un `#PF` aqui abajo es un desbordamiento de pila y no otra cosa.
pub const USER_STACK_BOTTOM: u64 = USER_STACK_TOP - USER_STACK_SIZE;
pub const CHANNEL_VA_BASE: u64 = 0x0000_0000_C000_0000;
/// Donde se mapea el framebuffer en el espacio de quien reclame la pantalla.
/// Por encima de los estuarios y con sitio de sobra: 4K x 4K x 4 B son 64 MiB
/// y aqui hay un hueco entero de 1 GiB antes del limite del canonical bajo.
pub const FRAMEBUFFER_VA_BASE: u64 = 0x0000_0000_D000_0000;
/// Donde empiezan los bloques que un proceso PIDE (`KIND_MEMORIA`).
///
/// Detras del framebuffer y con 256 MiB de hueco antes: el tope por peticion
/// son 64 MiB y hay cuatro peticiones, asi que el peor caso cabe entero sin
/// acercarse a nada. Cada proceso avanza su propio cursor desde aqui -- dos
/// bloques del mismo proceso no se pisan y cada uno tiene su rango.
pub const MEMORIA_VA_BASE: u64 = 0x0000_0000_E000_0000;

static mut KERNEL_PML4: u64 = 0;

/// Kernel-half PML4 slots that were still empty at init and could not get a
/// pre-populated PDPT (allocator exhaustion -- should never happen at boot).
static mut KERNEL_HALF_HOLES: u32 = 0;

/// Capture the boot CR3 (installed by `s2_mem`) and **pre-populate the whole
/// kernel half**: every empty PML4 slot in [256..512) gets a zeroed,
/// supervisor-only PDPT right now, *before any user address space exists*.
///
/// This is the invariant that makes `new_address_space`'s entry copy a true
/// share-by-pointer forever: user PML4s copy these 256 entries once, so any
/// higher-half mapping the kernel adds later (physmap growth toward 1 TiB,
/// MMIO windows, a kernel heap) lands *inside* a PDPT every address space
/// already points at -- visible under every CR3, no per-process patching.
/// Cost: <=256 frames (1 MiB), once.
pub fn init() {
    unsafe { KERNEL_PML4 = read_cr3() };
    let kernel = table(kernel_pml4());
    for i in 256..512 {
        if kernel[i] & PTE_PRESENT == 0 {
            match phys::alloc_frame() {
                Some(f) => {
                    phys::zero_frame(f);
                    // Supervisor-only on purpose: no PTE_USER at any level of
                    // the kernel half, ever.
                    kernel[i] = (f & ADDR_MASK) | PTE_PRESENT | PTE_WRITABLE;
                }
                None => unsafe { KERNEL_HALF_HOLES += 1 },
            }
        }
    }
    // Fresh top-level entries: reload CR3 so no stale paging-structure cache
    // survives (cheap, once at boot).
    switch_to(kernel_pml4());
    if unsafe { KERNEL_HALF_HOLES } != 0 {
        crate::ring0::dev::console::serial_write(
            "[vmm] WARN: kernel-half pre-population incomplete\n",
        );
    }
}

/// Number of kernel-half PML4 slots left unpopulated at init (0 = healthy).
pub fn kernel_half_holes() -> u32 {
    unsafe { KERNEL_HALF_HOLES }
}

pub fn read_cr3() -> u64 {
    let v: u64;
    unsafe { core::arch::asm!("mov {}, cr3", out(reg) v, options(nostack)); }
    v & ADDR_MASK
}

pub fn kernel_pml4() -> u64 {
    unsafe { KERNEL_PML4 }
}

/// Load CR3 with another address space. Only safe while running on
/// kernel-owned mappings (physmap / identity), which exist in every
/// address space the process loader creates.
pub fn switch_to(pml4: u64) {
    unsafe { core::arch::asm!("mov cr3, {}", in(reg) pml4, options(nostack)) };
}

/// Page-table frame as a mutable 512-entry array, through the physmap.
fn table(phys: u64) -> &'static mut [u64; 512] {
    unsafe { &mut *(phys_to_virt(phys) as *mut [u64; 512]) }
}

/// Create an empty user address space. Returns the physical PML4 address.
/// Kernel entries are shared read-write (supervisor-only leaves); the user
/// half starts empty except for the copied identity entry.
pub fn new_address_space() -> Option<u64> {
    let pml4 = phys::alloc_frame()?;
    let pdpt = match phys::alloc_frame() {
        Some(f) => f,
        None => {
            phys::free_frame(pml4);
            return None;
        }
    };
    phys::zero_frame(pml4);
    phys::zero_frame(pdpt);

    let kernel = table(kernel_pml4());
    let user = table(pml4);
    // Share the entire kernel half (physmap lives at index 256..). Since
    // `init` pre-populated every slot, this copy is share-by-pointer of the
    // PDPTs themselves: kernel-half mappings added *after* this process was
    // created are still visible under its CR3.
    for i in 256..512 {
        user[i] = kernel[i];
    }
    // PTE_USER here is required so user mappings under PDPT[1..] are
    // reachable; the identity region stays supervisor because the copied
    // PDPT[0] entry and its huge-page leaves do not carry PTE_USER.
    user[0] = pdpt | PTE_PRESENT | PTE_WRITABLE | PTE_USER;
    let kernel_pdpt = table(kernel[0] & ADDR_MASK);
    let user_pdpt = table(pdpt);
    user_pdpt[0] = kernel_pdpt[0];
    Some(pml4)
}

fn get_or_create(t: &mut [u64; 512], idx: usize, flags: u64) -> Result<u64, ()> {
    let e = t[idx];
    if e & PTE_PRESENT != 0 {
        if e & PTE_HUGE != 0 {
            return Err(());
        }
        return Ok(e & ADDR_MASK);
    }
    let f = phys::alloc_frame().ok_or(())?;
    phys::zero_frame(f);
    t[idx] = (f & ADDR_MASK) | flags;
    Ok(f)
}

/// Map one 4 KiB page. `user` sets U/S on every level touched; `writable`
/// controls the leaf's R/W bit. Fails on misalignment or a huge-page
/// collision (which would mean the VA overlaps the kernel identity map).
pub fn map_page(pml4: u64, va: u64, pa: u64, user: bool, writable: bool) -> Result<(), ()> {
    map_page_tipo(pml4, va, pa, user, writable, false, false)
}

/// Igual, pero declarando que **el marco es de este espacio de direcciones** y
/// hay que devolverlo cuando el espacio se destruya. Ver [`PTE_NUESTRA`].
///
/// Se pide con esto lo que sale de `phys::alloc_frame` para un proceso concreto
/// y no lo sabe nadie mas: **la imagen y la pila de usuario**. No se pide para:
///
/// - el **framebuffer**, que es MMIO y no salio del asignador de RAM;
/// - lo **prestado**, que sobrevive al que lo presto por diseno;
/// - los bloques de `KIND_MEMORIA`, que tienen dueno explicito -- `obj::memory`
///   los ficha con su fisica y los libera el mismo, **y ademas pregunta antes
///   si estan prestados**. Marcarlos aqui seria liberarlos dos veces y saltarse
///   esa pregunta.
pub fn map_page_propia(pml4: u64, va: u64, pa: u64, user: bool, writable: bool) -> Result<(), ()> {
    map_page_tipo(pml4, va, pa, user, writable, false, true)
}

/// Igual, pero eligiendo **Write-Combining** para esta pagina.
///
/// Se usa para el framebuffer y nada mas: es donde se escriben millones de
/// pixeles seguidos y donde juntar las escrituras cambia el orden de magnitud.
/// Para memoria normal seria lo contrario de lo que se quiere -- WC no garantiza
/// el orden de las escrituras, y eso en una estructura de datos es un bug.
pub fn map_page_wc(pml4: u64, va: u64, pa: u64, user: bool, writable: bool) -> Result<(), ()> {
    map_page_tipo(pml4, va, pa, user, writable, true, false)
}

fn map_page_tipo(
    pml4: u64,
    va: u64,
    pa: u64,
    user: bool,
    writable: bool,
    combinar_escrituras: bool,
    nuestra: bool,
) -> Result<(), ()> {
    if va % PAGE != 0 || pa % PAGE != 0 {
        return Err(());
    }
    let i4 = ((va >> 39) & 0x1FF) as usize;
    let i3 = ((va >> 30) & 0x1FF) as usize;
    let i2 = ((va >> 21) & 0x1FF) as usize;
    let i1 = ((va >> 12) & 0x1FF) as usize;
    let mid = PTE_PRESENT | PTE_WRITABLE | if user { PTE_USER } else { 0 };

    let pt_phys = {
        let p = table(pml4);
        let pdpt_phys = get_or_create(p, i4, mid)?;
        let pdpt = table(pdpt_phys);
        let pd_phys = get_or_create(pdpt, i3, mid)?;
        let pd = table(pd_phys);
        get_or_create(pd, i2, mid)?
    };
    let pt = table(pt_phys);

    let mut entry = (pa & ADDR_MASK) | PTE_PRESENT;
    if writable {
        entry |= PTE_WRITABLE;
    }
    if user {
        entry |= PTE_USER;
    }
    if combinar_escrituras {
        entry |= PTE_PAT_4K;
    }
    if nuestra {
        entry |= PTE_NUESTRA;
    }
    let old = pt[i1];
    pt[i1] = entry;
    if old & PTE_PRESENT != 0 {
        unsafe { core::arch::asm!("invlpg [{}]", in(reg) va, options(nostack)) };
    }
    Ok(())
}

/// Remove a mapping. Returns the physical address that was mapped, if any.
/// Does not free table frames (they are recycled by the address space).
pub fn unmap_page(pml4: u64, va: u64) -> Option<u64> {
    let i4 = ((va >> 39) & 0x1FF) as usize;
    let i3 = ((va >> 30) & 0x1FF) as usize;
    let i2 = ((va >> 21) & 0x1FF) as usize;
    let i1 = ((va >> 12) & 0x1FF) as usize;

    let p = table(pml4);
    let e = p[i4];
    if e & PTE_PRESENT == 0 || e & PTE_HUGE != 0 {
        return None;
    }
    let pdpt = table(e & ADDR_MASK);
    let e = pdpt[i3];
    if e & PTE_PRESENT == 0 || e & PTE_HUGE != 0 {
        return None;
    }
    let pd = table(e & ADDR_MASK);
    let e = pd[i2];
    if e & PTE_PRESENT == 0 || e & PTE_HUGE != 0 {
        return None;
    }
    let pt = table(e & ADDR_MASK);
    let e = pt[i1];
    if e & PTE_PRESENT == 0 {
        return None;
    }
    pt[i1] = 0;
    unsafe { core::arch::asm!("invlpg [{}]", in(reg) va, options(nostack)) };
    Some(e & ADDR_MASK)
}

/// Resolve a virtual address through the tables. Debugging aid; returns the
/// physical base of the mapped page.
pub fn translate(pml4: u64, va: u64) -> Option<u64> {
    let i4 = ((va >> 39) & 0x1FF) as usize;
    let i3 = ((va >> 30) & 0x1FF) as usize;
    let i2 = ((va >> 21) & 0x1FF) as usize;
    let i1 = ((va >> 12) & 0x1FF) as usize;

    let p = table(pml4);
    let e = p[i4];
    if e & PTE_PRESENT == 0 {
        return None;
    }
    let pdpt = table(e & ADDR_MASK);
    let e = pdpt[i3];
    if e & PTE_PRESENT == 0 {
        return None;
    }
    if e & PTE_HUGE != 0 {
        return Some((e & 0x000F_FFFF_C000_0000) + (va & 0x3FFF_FFFF));
    }
    let pd = table(e & ADDR_MASK);
    let e = pd[i2];
    if e & PTE_PRESENT == 0 {
        return None;
    }
    if e & PTE_HUGE != 0 {
        return Some((e & 0x000F_FFFF_FFE0_0000) + (va & 0x1F_FFFF));
    }
    let pt = table(e & ADDR_MASK);
    let e = pt[i1];
    if e & PTE_PRESENT == 0 {
        return None;
    }
    Some(e & ADDR_MASK)
}

/// **La direccion FISICA EXACTA de `va`**, sea cual sea el tamano de pagina.
///
/// == [!] Por que existe, y por que no vale [`translate`] ==
///
/// `translate` **no contesta lo mismo segun el tamano de pagina**, y eso no se
/// ve leyendo su firma:
///
/// | mapeo | lo que devuelve |
/// |---|---|
/// | pagina de 4 KiB | la **base** de la pagina, sin el desplazamiento |
/// | pagina de 2 MiB o 1 GiB | la direccion **exacta**, desplazamiento incluido |
///
/// Su documentacion dice "the physical base of the mapped page", que es cierto
/// para el primer caso y no para los otros dos. Mientras sus dos usuarios eran
/// mapear paginas --donde lo que hace falta es la base-- y el autodiagnostico, la
/// diferencia no se notaba.
///
/// ** Con DMA si se nota, y de la peor manera. El HBA escribe donde se le diga:
/// sumarle el desplazamiento a una respuesta que ya lo llevaba dentro apunta
/// unos bytes mas alla, y como el physmap del kernel esta montado con paginas de
/// 2 MiB, ese es justo el caso de cualquier buffer que viva ahi --las pilas de
/// tarea, por ejemplo--. El resultado no seria una lectura mala: seria el disco
/// escribiendo encima de memoria de otro.
///
/// Asi que la pregunta se hace con su propio nombre. `translate` se queda como
/// esta porque sus usuarios quieren la base; quien quiera la direccion, pide la
/// direccion.
pub fn fisica_exacta(pml4: u64, va: u64) -> Option<u64> {
    let i4 = ((va >> 39) & 0x1FF) as usize;
    let i3 = ((va >> 30) & 0x1FF) as usize;
    let i2 = ((va >> 21) & 0x1FF) as usize;
    let i1 = ((va >> 12) & 0x1FF) as usize;

    let p = table(pml4);
    let e = p[i4];
    if e & PTE_PRESENT == 0 {
        return None;
    }
    let pdpt = table(e & ADDR_MASK);
    let e = pdpt[i3];
    if e & PTE_PRESENT == 0 {
        return None;
    }
    if e & PTE_HUGE != 0 {
        return Some((e & 0x000F_FFFF_C000_0000) + (va & 0x3FFF_FFFF));
    }
    let pd = table(e & ADDR_MASK);
    let e = pd[i2];
    if e & PTE_PRESENT == 0 {
        return None;
    }
    if e & PTE_HUGE != 0 {
        return Some((e & 0x000F_FFFF_FFE0_0000) + (va & 0x1F_FFFF));
    }
    let pt = table(e & ADDR_MASK);
    let e = pt[i1];
    if e & PTE_PRESENT == 0 {
        return None;
    }
    // Y aqui esta la diferencia entera: la base MAS el desplazamiento.
    Some((e & ADDR_MASK) + (va & (super::PAGE - 1)))
}

/// **Destruir un espacio de direcciones: sus tablas y las hojas que son suyas.**
///
/// Devuelve `(hojas, tablas)` -- cuantos marcos de datos y cuantos de tablas se
/// devolvieron al asignador. Los dos numeros existen para que
/// [`self_test`] y el panel puedan **comprobar** que la cuenta cuadra en vez de
/// creersela.
///
/// # Lo que hacia antes, y por que no bastaba
///
/// Esta funcion liberaba **solo las tablas**, con una nota que decia *"las hojas
/// son del que las pidio y las libera el primero"*. La nota describia un
/// reparto correcto... que nadie cumplia: `proc.rs` reservaba la imagen y la
/// pila de usuario marco a marco y **no las apuntaba en ningun sitio**, asi que
/// no habia quien las liberara. Y encima esto no se llamaba nunca al morir un
/// proceso: su unico llamante era `self_test`.
///
/// Ahora las hojas se reconocen por [`PTE_NUESTRA`], que es una columna en la
/// tabla que el hardware ya mantiene. Lo que no lleva el bit no se toca **y eso
/// es la mitad del trabajo**: por ahi pasan el framebuffer (MMIO) y los marcos
/// prestados, y devolver cualquiera de los dos al asignador seria mucho peor que
/// la fuga que esto arregla.
///
/// # Los dos sitios por donde NO se baja, y los dos son a vida o muerte
///
/// 1. **Solo `PML4[0]`.** Los indices 256..512 son la mitad del kernel, que
///    `new_address_space` copia **por puntero** en todo espacio: bajar por ahi
///    liberaria las tablas del propio kernel con todo corriendo encima.
/// 2. **`PDPT[0]` se salta.** Es la region de identidad, tambien compartida y
///    tambien copiada del kernel. De ahi que el bucle empiece en 1.
///
/// [!] No hay `invlpg` ni cambio de CR3: el espacio que se destruye **no puede
/// estar en vigor**. Quien llama tiene que garantizarlo, y por eso esto vive en
/// `reap` --que corre despues del cambio de contexto-- y no en `revoke_all`,
/// que corre todavia dentro del syscall del moribundo y con SU CR3 puesto.
pub fn destroy_address_space(pml4: u64) -> (u64, u64) {
    let mut hojas = 0u64;
    let mut tablas = 0u64;
    let user = table(pml4);
    let e0 = user[0];
    if e0 & PTE_PRESENT != 0 {
        let pdpt_phys = e0 & ADDR_MASK;
        let pdpt = table(pdpt_phys);
        for i3 in 1..512 {
            let e = pdpt[i3];
            if e & PTE_PRESENT == 0 || e & PTE_HUGE != 0 {
                continue;
            }
            let pd_phys = e & ADDR_MASK;
            let pd = table(pd_phys);
            for i2 in 0..512 {
                let e2 = pd[i2];
                if e2 & PTE_PRESENT == 0 || e2 & PTE_HUGE != 0 {
                    continue;
                }
                // ** Y AQUI SE BAJA UN NIVEL MAS, que es lo que faltaba.
                let pt_phys = e2 & ADDR_MASK;
                let pt = table(pt_phys);
                for i1 in 0..512 {
                    let hoja = pt[i1];
                    if hoja & PTE_PRESENT == 0 || hoja & PTE_NUESTRA == 0 {
                        continue;
                    }
                    let marco = hoja & ADDR_MASK;
                    // Se limpia por el mismo motivo que en `obj::memory`: el
                    // asignador no limpia al entregar, asi que si no se limpia
                    // al devolver, el siguiente programa lee lo del anterior.
                    phys::zero_frame(marco);
                    phys::free_frame(marco);
                    hojas += 1;
                }
                phys::free_frame(pt_phys);
                tablas += 1;
            }
            phys::free_frame(pd_phys);
            tablas += 1;
        }
        phys::free_frame(pdpt_phys);
        tablas += 1;
    }
    phys::free_frame(pml4);
    tablas += 1;
    (hojas, tablas)
}

/// End-to-end check: allocate, build an address space, map, translate,
/// write through the physmap, unmap, destroy, free. Returns false on the
/// first failed step. Safe to run from the serial shell at any time.
/// Devuelve `(ok, sobrantes)`: si los pasos salieron bien, y **cuantos marcos
/// no volvieron al asignador**.
///
/// # Por que la segunda cifra, que es la que faltaba
///
/// Esto contestaba `true`/`false` y con eso no se distingue *"funciono"* de
/// *"funciono y se dejo tres marcos por el camino"*. Justo esa diferencia es la
/// fuga que estuvo abierta hasta el 14-08 y que **encontro el dueno mirando
/// `mem`**, no ningun contador nuestro. Un instrumento que no puede ver el fallo
/// que ya paso una vez no es un instrumento.
///
/// Se mide con `phys::stats()` antes y despues del ciclo completo, que es la
/// unica fuente que no puede mentir: es el propio mapa de bits del asignador.
///
/// ** `sobrantes = 0` es la respuesta buena.** Cualquier otra cosa es memoria
/// que el sistema ya no sabe que tiene, y da igual de quien fuera.
///
/// [!] La pagina de datos se mapea con [`map_page_propia`] **a proposito**: asi
/// esta prueba recorre el camino nuevo --el que devuelve las hojas-- y no el de
/// antes. Si se mapeara con `map_page` a secas, `sobrantes` saldria 1 y estaria
/// bien: seria un marco que su dueno tiene que liberar aparte.
pub fn self_test() -> (bool, u64) {
    let (_, libres_antes) = phys::stats();
    let frame = match phys::alloc_frame() {
        Some(f) => f,
        None => return (false, 0),
    };
    let aspace = match new_address_space() {
        Some(s) => s,
        None => {
            phys::free_frame(frame);
            return (false, 0);
        }
    };
    let va = USER_IMAGE_BASE;
    let mut ok = map_page_propia(aspace, va, frame, true, true).is_ok();
    if ok {
        ok = translate(aspace, va) == Some(frame);
    }
    if ok {
        unsafe {
            let p = phys_to_virt(frame) as *mut u64;
            p.write_volatile(0xB400_0000_0000_0001);
            ok = p.read_volatile() == 0xB400_0000_0000_0001;
        }
    }
    // ** Se desmapea a mano, asi que la hoja ya NO esta en la tabla cuando se
    // destruye el espacio: la libera esta linea, no el destructor. Es el reparto
    // correcto --quien desmapea se queda con la pieza-- y de paso comprueba que
    // el destructor no libera dos veces lo que ya no esta.
    if unmap_page(aspace, va) != Some(frame) {
        ok = false;
    }
    if ok {
        ok = translate(aspace, va).is_none();
    }
    destroy_address_space(aspace);
    phys::free_frame(frame);
    let (_, libres_despues) = phys::stats();
    let sobrantes = libres_antes.saturating_sub(libres_despues);
    (ok, sobrantes)
}
