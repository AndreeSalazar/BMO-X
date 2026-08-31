//! **CARRIL AMARILLO** -- va a cambiar, y al cambiar ARRASTRA A OTRO.
//!
//! [carril]  AMARILLO  el nombre del fichero ya lo decia; la etiqueta lo hace comprobable
//!
//! [cuesta]  MAQUINA -- escribe entradas de tabla de paginas. Un bit de mas
//!           en `map_page_tipo` es una ventana, no un fallo.
//!
//! [riesgo]  ESPEJO -- los tres envoltorios (`map_page`, `map_page_propia`,
//!           `map_page_wc`) son la MISMA decision escrita tres veces sobre
//!           `map_page_tipo`. Tocar uno sin los otros es como se separan.
//!
//! [prueba]  bmo-fisica-juicio
//!
//! # *** POR QUE ESTO ES AMARILLO Y NO ROJO: EL CAMBIO YA ESTA ESCRITO
//!
//! No es que pueda cambiar. **Esta pendiente, con nombre y con cuenta**, en la
//! cabecera de `PTE_NX` de aqui abajo:
//!
//! > *"Hacen falta TRES estados, y eso es un parametro mas en `map_page_tipo` y
//! > en sus cuatro llamantes. No es dificil; es que no se puede hacer a medias."*
//!
//! Ese es el carril amarillo entero: una pieza que **se sabe que se va a tocar**
//! y que **arrastra a cuatro sitios** cuando se toque. Sin este fichero, quien
//! venga a cerrar `rodata` tendria que descubrir los cuatro por su cuenta -- y
//! la cabecera de `PTE_NX` avisa de que el arreglo obvio abre un agujero peor
//! que el que cierra.

use super::roja::{get_or_create, table};
use super::verde::{ADDR_MASK, PTE_HUGE, PTE_PRESENT, PTE_USER, PTE_WRITABLE};
use super::super::{phys, PAGE};

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

/// **El bit 63: esta pagina NO se ejecuta.**
///
/// # *** W^X, Y AQUI NO HACE FALTA NI UN PARAMETRO NUEVO (2026-08-24)
///
/// La regla se llama `W^X` --escribible O ejecutable, nunca las dos-- y esa
/// frase es literalmente la condicion que ya se pasa a esta funcion:
///
/// ```text
///    map_page(.., writable = true)   -> datos, pila, canal, framebuffer
///    map_page(.., writable = false)  -> el codigo del .bex, y solo el
/// ```
///
/// ** El cargador ya lo calculaba: `writable = flags & SECTION_FLAG_EXEC == 0`.
/// O sea que la informacion de que es codigo lleva ahi desde el principio, y
/// solo faltaba escribir el otro lado de la moneda en la tabla de paginas.
///
/// *** LO QUE COMPRA, Y ES LA MITAD DE UNA EXPLOTACION: sin esto, quien
/// consiga escribir en cualquier sitio escribe instrucciones y salta a ellas.
/// Con esto tiene que construir la cadena con trozos de codigo que YA existen
/// --ROP-- que es un trabajo de otro orden de magnitud.
///
/// [!] Y el codigo se escribe POR EL PHYSMAP, no por la VA del proceso
/// (`admitir.rs` usa `phys_to_virt`), asi que mapear la seccion de codigo sin
/// permiso de escritura no estorba a cargarla. Esa decision ya estaba tomada.
///
/// # [!!] LA TRAMPA PARA EL QUE VENGA A CERRAR `rodata`
///
/// Hoy hay DOS estados y por eso basta un `bool`: escribible-y-no-ejecutable, o
/// ejecutable-y-no-escribible. `rodata` cae en el primero -- **se mapea
/// escribible**, que no es correcto pero como mucho deja que un programa
/// corrompa sus propias constantes; no cruza ninguna frontera.
///
/// *** Y quien vaya a arreglarlo tiene que saber esto ANTES de tocarlo:
///
/// ```text
///    "rodata no deberia ser escribible"   ->  writable = false
///    y con la regla de aqui, eso lo vuelve EJECUTABLE
/// ```
///
/// ** O sea que el arreglo obvio abre un agujero peor que el que cierra: una
/// region de datos que el programa controla y que ademas se puede ejecutar es
/// exactamente lo que W^X existe para impedir.
///
/// Hacen falta TRES estados, y eso es un parametro mas en `map_page_tipo` y en
/// sus cuatro llamantes. No es dificil; es que no se puede hacer a medias.
pub const PTE_NX: u64 = 1 << 63;

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

/// **Se puede usar el bit NX?** Se lee `EFER.NXE` una vez y se recuerda.
///
/// ** Si sale que NO, se dice por CABINA con gravedad de fallo y se deja de
/// marcar. Degradar en silencio seria lo peor de las dos opciones: ni protege
/// ni se entera nadie -- y este arbol ya tiene escrito lo que pasa con eso en

/// `bmo_cripto::azar`, que por lo mismo se niega a tener respaldo.
pub(super) fn nx_disponible() -> bool {
    use core::sync::atomic::{AtomicU8, Ordering};
    static ESTADO: AtomicU8 = AtomicU8::new(0); // 0 sin mirar, 1 si, 2 no
    match ESTADO.load(Ordering::Relaxed) {
        1 => return true,
        2 => return false,
        _ => {}
    }
    // `IA32_EFER` = 0xC000_0080, y el bit 11 es NXE.
    let lo: u32;
    let hi: u32;
    unsafe {
        core::arch::asm!("rdmsr", in("ecx") 0xC000_0080u32, out("eax") lo, out("edx") hi,
                         options(nomem, nostack));
    }
    let _ = hi;
    let hay = lo & (1 << 11) != 0;
    if hay {
        ESTADO.store(1, Ordering::Relaxed);
    } else {
        ESTADO.store(2, Ordering::Relaxed);
        crate::ring0::cabina::fault(
            "mm",
            "EFER.NXE APAGADO: W^X no se puede aplicar y toda pagina sera ejecutable",
            lo as u64,
        );
    }
    hay
}


pub(super) fn map_page_tipo(
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
    // *** W^X. Ver `PTE_NX`: escribible y ejecutable son excluyentes, y la
    // condicion ya venia calculada desde el cargador.
    //
    // [!] Se pregunta si `EFER.NXE` esta puesto, y no se supone. Con NXE en
    // cero el bit 63 es RESERVADO: cada pagina marcada asi daria `#PF` por bit
    // reservado y **no arrancaria nada**. `s1_cpu` lo enciende, asi que esto
    // tendria que ser siempre cierto -- razon de mas para preguntarlo, porque
    // lo que "tendria que ser siempre cierto" es lo que nadie mira el dia que
    // deja de serlo. Si no esta, `nx_disponible()` lo GRITA por CABINA.
    if writable && nx_disponible() {
        entry |= PTE_NX;
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

