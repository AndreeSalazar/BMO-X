//! **CARRIL AMARILLO** -- la unica de `phys` que ESCRIBE en la memoria.
//!
//! [carril]  AMARILLO  su techo es ESPEJO del de `vmm::caminable`: se tocan las dos o ninguna
//!
//! [cuesta]  MAQUINA -- escribe 4 KiB por el physmap. Fuera del espejo es una
//!           pantalla azul; dentro y del vecino son 4 KiB de memoria viva a
//!           cero, EN SILENCIO. La segunda es la peor.
//!
//! [riesgo]  ESPEJO -- juzga el MISMO numero que `mm::vmm::amarilla::caminable`.
//!           El 30-08 cada una tenia su techo --16 GiB contra 64 TiB-- y la
//!           maquina se paro dos veces. Si este techo cambia, ese cambia con el.
//!
//! [prueba]  bmo-fisica-juicio
//!
//! # *** POR QUE ESTO ES UN CARRIL Y NO UNA FUNCION MAS DE `phys`
//!
//! Porque `phys` es un contable y esto es un albanil. Todo lo demas del modulo
//! enciende y apaga bits de un bitmap; **esta es la unica que desreferencia una
//! direccion fisica y escribe**. Un fallo alli reparte mal la RAM y se nota; un
//! fallo aqui borra la memoria de otro y no se nota hasta tres arranques
//! despues.
//!
//! ** Vivio dos dias en `ring0/critic/`, junto a su gemela. Ya no hace falta:
//! las dos preguntan a `bmo-fisica-juicio`, que **no tiene ni una constante de
//! tamano** --el espejo se le pasa en cada llamada--, asi que no hay dos numeros
//! que mantener de acuerdo. Lo que las ata sigue escrito, arriba, en `[riesgo]`.

use super::super::{phys_to_virt, PAGE, PHYSMAP_SIZE};
use bmo_fisica_juicio::se_puede_caminar;

/// Zero a frame through the physmap.
///
/// # *** LA MISMA COTA QUE `free_frame`, Y AQUI SE ESCRIBE (2026-08-30)
///
/// Esto no tenia ninguna. Y las dos funciones se llaman **una detras de otra**,
/// en cuatro sitios distintos, sobre el mismo numero:
///
/// ```text
///    mm::phys::zero_frame(marco);   <- 4 KiB ESCRITOS, sin comprobar nada
///    mm::phys::free_frame(marco);   <- rechaza >= MAX_PHYS
/// ```
///
/// ** Dos jueces del mismo valor con dos criterios, y el que NO comprobaba era
/// el que escribe. Es la forma exacta del fallo que paro la maquina el mismo
/// dia --`caminable` a 64 TiB contra `free_frame` a 16 GiB-- encontrada a
/// proposito buscando el gemelo. La ley que lo nombra es L6f, clase `ESPEJO`.
///
/// ## Lo que costaria, y son dos cosas distintas
///
/// ```text
///    fuera del physmap    #PF de escritura desde el kernel -> pantalla azul
///    dentro y del vecino  4 KiB de memoria viva a cero, EN SILENCIO
/// ```
///
/// *** La segunda es la peor y es la que no da ninguna pantalla. Por eso la
/// cota va aqui dentro y no en los ocho llamantes: un guardian que hay que
/// acordarse de poner no es un guardian.
///
/// [!] No puede estorbar a nadie: el asignador **no entrega** marcos por encima
/// de `MAX_PHYS`, asi que un `phys` que no pase por aqui no salio de el.
pub fn zero_frame(phys: u64) {
    // ** EL MISMO JUEZ QUE `caminable`, y eso es el fichero entero.
    //
    // El 30-08 esta comparaba contra `MAX_PHYS` y la de arriba contra un
    // `1 << 46` local. Ahora las dos preguntan lo mismo al mismo sitio, asi que
    // **no pueden volver a discrepar**: no hay dos numeros que mantener de
    // acuerdo, hay uno.
    if phys % PAGE != 0 || !se_puede_caminar(phys, PHYSMAP_SIZE).se_puede() {
        crate::ring0::cabina::fault("mm", "zero_frame sobre un marco que no existe", phys);
        return;
    }
    unsafe {
        core::ptr::write_bytes(phys_to_virt(phys) as *mut u8, 0, PAGE as usize);
    }
}
