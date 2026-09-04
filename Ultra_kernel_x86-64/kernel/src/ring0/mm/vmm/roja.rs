//! **CARRIL ROJO** -- un fallo aqui para la maquina y no deja autopsia.
//!
//! [carril]  ROJO      el nombre del fichero ya lo decia; la etiqueta lo hace comprobable
//!
//! [cuesta]  MAQUINA -- aqui vive el CR3 del kernel y el desmontaje de un
//!           espacio muerto. Ya paro la maquina DOS veces: el `#GP` del 25-08
//!           y el `#PF` del 30-08, las dos en `destroy_address_space`.
//!
//! [riesgo]  AJENO -- los numeros que camina no los escribe este fichero: salen
//!           de las tablas de pagina y del `cr3` de una ranura de tarea MUERTA.
//!
//! [prueba]  bmo-fisica-juicio
//!
//! # Que hay aqui, y por que no se toca sin pagar
//!
//! ```text
//!    KERNEL_PML4 / init      el suelo. Si esto sale mal no hay arranque
//!    switch_to / read_cr3    cambiar el CR3 en caliente
//!    table                   convertir una FISICA en `&mut [u64; 512]`. Es el
//!                            unico deref crudo del modulo
//!    get_or_create           escribir una entrada de tabla
//!    new_address_space       y destroy_address_space
//! ```
//!
//! ** `table` es la pieza mas pequena y la mas roja de todas: cuatro lineas que
//! convierten un numero en un puntero. Todo lo demas de este modulo confia en
//! que lo que le pasa a esa funcion ya paso por un juez.

use super::verde::{
    fisica_exacta, translate, ADDR_MASK, PTE_HUGE, PTE_PRESENT, PTE_USER, PTE_WRITABLE,
    USER_IMAGE_BASE,
};
use super::amarilla::{map_page, map_page_propia, unmap_page, PTE_NUESTRA};
use super::super::{phys, phys_to_virt, PAGE};


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
            match phys::alloc_frame_de(phys::Duenno::Tabla) {
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
pub(super) fn table(phys: u64) -> &'static mut [u64; 512] {
    unsafe { &mut *(phys_to_virt(phys) as *mut [u64; 512]) }
}

/// Create an empty user address space. Returns the physical PML4 address.
/// Kernel entries are shared read-write (supervisor-only leaves); the user

/// half starts empty except for the copied identity entry.
pub fn new_address_space() -> Option<u64> {
    // ** LAS TABLAS SE PIDEN DICIENDO QUE SON TABLAS. Desde aqui, el
    // asignador puede contestar *"ese marco no es tuyo"* en vez de solo
    // *"ya estaba libre"*. Ver `phys::duenno`.
    let pml4 = phys::alloc_frame_de(phys::Duenno::Tabla)?;
    let pdpt = match phys::alloc_frame_de(phys::Duenno::Tabla) {
        Some(f) => f,
        None => {
            phys::free_frame_de(pml4, phys::Duenno::Tabla);
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


pub(super) fn get_or_create(t: &mut [u64; 512], idx: usize, flags: u64) -> Result<u64, ()> {
    let e = t[idx];
    if e & PTE_PRESENT != 0 {
        if e & PTE_HUGE != 0 {
            return Err(());
        }
        return Ok(e & ADDR_MASK);
    }
    let f = phys::alloc_frame_de(phys::Duenno::Tabla).ok_or(())?;
    phys::zero_frame(f);
    t[idx] = (f & ADDR_MASK) | flags;
    Ok(f)
}

/// Map one 4 KiB page. `user` sets U/S on every level touched; `writable`
/// controls the leaf's R/W bit. Fails on misalignment or a huge-page

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
/// **Se puede caminar por esta direccion fisica?** -- vive en el CARRIL
/// AMARILLO de al lado, con su gemela `phys::zero_frame`: las dos juzgan el
/// mismo numero, y cambiar una sin la otra fue el bug del 30-08.
use super::amarilla::caminable;


/// **Es esto una tabla de paginas, de verdad?** El juez que faltaba ANTES de
/// bajar un nivel.
///
/// *** `caminable` contesta *cabe en el espejo?*, y se estaba usando como
/// *esto es una tabla?*. Son preguntas VECINAS y no la misma, y el 04-09 la
/// diferencia costo un arbol entero: el marco `4D2000` cabia perfectamente en
/// el espejo y lo que tenia dentro era `push r15; push r14; push r12`.
///
/// ** Y comprobar al SOLTAR llega tarde. Para cuando `free_frame_de` puede
/// objetar, este recorrido ya bajo por la tabla falsa, ya leyo 512 casillas de
/// codigo como si fueran direcciones y ya llamo a `zero_frame` sobre las que
/// cayeron dentro de los 16 GiB. **El unico sitio donde la pregunta evita el
/// dano es antes de bajar.**
///
/// [!] `Anonimo` pasa. Las tablas de un espacio creado antes de que existiera
/// la etiqueta no llevan ninguna, y rechazarlas dejaria de desmontar espacios
/// buenos -- una fuga a cambio de una sospecha. Solo se corta cuando la tabla
/// dice ser OTRA COSA, que es el caso que no tiene explicacion inocente.
fn es_tabla(fisica: u64, nivel: &'static str) -> bool {
    let q = phys::duenno_de(fisica);
    if q == phys::Duenno::Tabla || q == phys::Duenno::Anonimo {
        return true;
    }
    crate::ring0::cabina::fault("vmm", nivel, fisica);
    crate::ring0::cabina::fault("vmm", q.nombre(), 0);
    false
}

pub fn destroy_address_space(pml4: u64) -> (u64, u64) {
    // La estacion 17, y la unica que no vive en `revoke_all`: el espacio se
    // destruye despues, en `reap`. Se apunta con pid 0 porque aqui ya no hay
    // pid -- solo un PML4 y un cadaver.
    crate::ring0::core::desmontaje::entra(17, 0);
    let mut hojas = 0u64;
    let mut tablas = 0u64;
    let mut ya_libres = 0u64;
    // *** SE DESTRUYE DOS VECES? La pregunta que dejo servida el 04-09.
    //
    // Al morir DOOM, este recorrido encontro TRECE casillas malas EN LA MISMA
    // tabla (`4D2000`), y la cabecera de `amarilla::caminable` ya tenia escrito
    // lo que eso significa:
    //
    // > tres casillas malas en LA MISMA tabla -> ese marco NO es una tabla
    //
    // Y las entradas eran MAQUINA, no direcciones: `5053544156415741` leido en
    // little-endian es `41 57 41 56 41 54 53 50`, o sea
    // `push r15; push r14; push r12; push rbx; push rax`. Un prologo de
    // funcion. Ese marco tiene CODIGO dentro, y `phys` dijo ademas que ya
    // estaba libre: se solto, se volvio a entregar, alguien cargo un programa
    // encima, y este recorrido seguia teniendolo por una tabla.
    //
    // ** De las dos explicaciones posibles, esta linea elige:
    //
    //    el PML4 ya esta LIBRE  -> el espacio se destruye DOS VECES, y lo que
    //                              se camina la segunda vez es memoria de otro
    //    el PML4 esta OCUPADO   -> el arbol es suyo, y quien suelta tablas sin
    //                              desenlazarlas es OTRO
    //
    // [!] Y NO ES SOLO UNA SONDA. La segunda vuelta llama a `zero_frame` sobre
    // cada hoja que pase `caminable`, y `caminable` solo comprueba que la
    // direccion CABE en el espejo. Una hoja de basura que caiga dentro de los
    // 16 GiB **le borra 4 KiB a un proceso vivo**. Cortar aqui deja de hacer
    // eso, se descubra lo que se descubra despues.
    if let Some(true) = phys::esta_libre(pml4) {
        crate::ring0::cabina::fault(
            "vmm", "PML4 YA estaba LIBRE: el espacio se destruye DOS VECES", pml4);
        return (hojas, tablas);
    }
    // *** LA UNICA DIRECCION QUE ENTRA DE FUERA, Y ERA LA UNICA SIN JUEZ.
    //
    // ** Los cuatro niveles de abajo --PDPT, PD, PT y la hoja-- pasan por
    // `caminable`. Este no: `pml4` es el `cr3` que `reap` saca de la ranura de
    // una tarea MUERTA, o sea el valor con mas motivos para estar pisado de
    // todos los que toca esta funcion, y era el unico que se dereferenciaba a
    // ciegas. Un guardian que vigila las cuatro puertas de dentro y deja la de
    // la calle abierta no es un guardian.
    //
    // [!] Y si no es caminable NO SE LIBERA. `free_frame` lo rechazaria solo
    // --tiene el techo bueno-- pero devolver aqui deja el numero en CABINA en
    // vez de dejarlo pasar en silencio: una tarea que muere con el `cr3` roto
    // es un hallazgo, no una limpieza mas.
    if !caminable(pml4, "PML4: el CR3 de la tarea no es caminable", pml4, 0, 0) {
        return (hojas, tablas);
    }
    let user = table(pml4);
    let e0 = user[0];
    if e0 & PTE_PRESENT != 0 {
        let pdpt_phys = e0 & ADDR_MASK;
        if !caminable(pdpt_phys, "PML4: entrada fuera del physmap", e0, pml4, 0) {
            return (hojas, tablas);
        }
        // ** CABER EN EL ESPEJO NO ES SER UNA TABLA. `caminable` contesta lo
        // primero; esto lo segundo. Un marco LIBRE que sigue enlazado como
        // tabla es el hallazgo entero, y caminarlo es leer lo de otro.
        if let Some(true) = phys::esta_libre(pdpt_phys) {
            crate::ring0::cabina::fault("vmm", "PDPT enlazado pero YA LIBRE", pdpt_phys);
            return (hojas, tablas);
        }
        if !es_tabla(pdpt_phys, "PDPT que NO es una tabla") {
            return (hojas, tablas);
        }
        let pdpt = table(pdpt_phys);
        for i3 in 1..512 {
            let e = pdpt[i3];
            if e & PTE_PRESENT == 0 || e & PTE_HUGE != 0 {
                continue;
            }
            let pd_phys = e & ADDR_MASK;
            if !caminable(pd_phys, "PDPT: entrada fuera del physmap", e, pdpt_phys, i3) {
                continue;
            }
            if let Some(true) = phys::esta_libre(pd_phys) {
                crate::ring0::cabina::fault("vmm", "PD enlazado pero YA LIBRE", pd_phys);
                continue;
            }
            if !es_tabla(pd_phys, "PD que NO es una tabla") {
                continue;
            }
            let pd = table(pd_phys);
            for i2 in 0..512 {
                let e2 = pd[i2];
                if e2 & PTE_PRESENT == 0 || e2 & PTE_HUGE != 0 {
                    continue;
                }
                // ** Y AQUI SE BAJA UN NIVEL MAS, que es lo que faltaba.
                let pt_phys = e2 & ADDR_MASK;
                if !caminable(pt_phys, "PD: entrada fuera del physmap", e2, pd_phys, i2) {
                    continue;
                }
                // *** ESTE es el nivel donde el Ryzen enseno el codigo: la
                // tabla `4D2000` del 04-09 era un PT.
                if let Some(true) = phys::esta_libre(pt_phys) {
                    crate::ring0::cabina::fault("vmm", "PT enlazado pero YA LIBRE", pt_phys);
                    continue;
                }
                // *** EL SITIO EXACTO DEL 04-09: `4D2000` era un PT.
                if !es_tabla(pt_phys, "PT que NO es una tabla") {
                    continue;
                }
                let pt = table(pt_phys);
                for i1 in 0..512 {
                    let hoja = pt[i1];
                    if hoja & PTE_PRESENT == 0 || hoja & PTE_NUESTRA == 0 {
                        continue;
                    }
                    let marco = hoja & ADDR_MASK;
                    // ** La hoja tambien: `zero_frame` escribe por el physmap, y
                    // una hoja con basura mata igual que una tabla.
                    if !caminable(marco, "HOJA: entrada fuera del physmap", hoja, pt_phys, i1) {
                        continue;
                    }
                    // ** UNA HOJA QUE YA ESTABA LIBRE NO SE BORRA.
                    //
                    // Se CUENTA en vez de decirse una por una: a 512 casillas
                    // por tabla, un arbol pisado llenaria CABINA de renglones
                    // iguales y taparia su propio mensaje --la leccion del cepo
                    // del 30-08--. El total sale al final, en una linea.
                    //
                    // [!] Y lo que se evita es lo caro: `zero_frame` sobre un
                    // marco que ya es de otro le borra 4 KiB a un proceso vivo.
                    // Ese es el sintoma que aparece tres arranques despues.
                    if let Some(true) = phys::esta_libre(marco) {
                        ya_libres += 1;
                        continue;
                    }
                    // Se limpia por el mismo motivo que en `obj::memory`: el
                    // asignador no limpia al entregar, asi que si no se limpia
                    // al devolver, el siguiente programa lee lo del anterior.
                    phys::zero_frame(marco);
                    phys::free_frame(marco);
                    hojas += 1;
                }
                phys::free_frame_de(pt_phys, phys::Duenno::Tabla);
                tablas += 1;
            }
            phys::free_frame_de(pd_phys, phys::Duenno::Tabla);
            tablas += 1;
        }
        phys::free_frame_de(pdpt_phys, phys::Duenno::Tabla);
        tablas += 1;
    }
    phys::free_frame_de(pml4, phys::Duenno::Tabla);
    tablas += 1;
    // ** Y el total de hojas que ya estaban libres. CERO tambien es respuesta:
    // dice que el arbol era suyo entero y que el problema no es este recorrido.
    if ya_libres != 0 {
        crate::ring0::cabina::fault(
            "vmm", "hojas enlazadas y YA LIBRES (NO borradas)", ya_libres);
    }
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
