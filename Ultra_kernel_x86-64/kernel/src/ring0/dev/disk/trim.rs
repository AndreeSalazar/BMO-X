//! **TRIM: devolverle sectores al disco.** El unico camino por el que BMO-X le
//! dice al aparato que algo dejo de importar.
//!
//! [eje]     CORRECCION -- lo pide una persona, no un demonio
//! [exige]   R-DISCO10 (sin TRIM el recolector trabaja para nadie), L7 (el
//!           formato no vive aqui), la seccion 9 de ESTRATOS (politica, no
//!           automatismo)
//!
//! # Que hace este fichero, y que NO hace
//!
//! Junta cuatro cosas que viven fuera: el **formato** (`bmo-trim`), el
//! **comando** (`bmo-ahci`), lo que **el disco declaro** (`perfil`) y **donde
//! se puede tocar** (la ventana de `bmo-block`). Aqui no se decide ni una de
//! las cuatro. Si este fichero crece, es que alguien esta decidiendo en el sitio
//! equivocado.
//!
//! # ** POR QUE TRIM PASA POR LOS MISMOS GUARDIANES QUE ESCRIBIR
//!
//! Porque **es destructivo**. No mueve bytes, asi que es facil leerlo como una
//! sugerencia amable al aparato -- y no lo es: despues de un TRIM, esos sectores
//! ya no tienen lo que tenian. Un TRIM sobre la ESP se lleva el `BOOTX64.EFI`
//! igual de bien que una escritura, y en esta maquina la ESP tambien lleva el
//! cargador de Windows del dueno.
//!
//! Asi que el orden de las puertas es el de `write` y una mas, que es propia:
//!
//! ```text
//!   hay disco?                      si no, no hay nada que recortar
//!   lo SOPORTA el aparato?          la palabra 169, leida y no supuesta
//!   esta armada la escritura?       el gate de identidad, el mismo
//!   cae dentro de una ventana?      el rango ENTERO, no tanda a tanda
//! ```
//!
//! # Donde esta BMO-X hoy, y por que eso importa
//!
//! El historial de corrupcion de TRIM es del TRIM **encolado** --la lista negra
//! `NO_NCQ_TRIM` de Linux-- y este driver esta en profundidad de cola 1: no
//! encola nada. O sea que hoy se manda la variante que **no tiene muertos
//! documentados**, y se manda antes de encender la cola a proposito. El capitulo
//! lo argumenta entero: `docs/componente/EL_DISCO_EXIGE.md`.

use super::*;
use bmo_trim::Rango;

/// Como acabo un recorte. Ninguna forma de "no" se parece a otra.
///
/// ** No es un `Result<u64, &str>` porque quien pregunta hace cosas distintas
/// con cada negativa: *"este disco no lo soporta"* es una propiedad del aparato
/// que no va a cambiar, *"no esta armado"* es un estado que se puede ganar, y
/// *"fuera de la ventana"* es un bug del que llama. Un texto suelto obliga a
/// leerlo para distinguirlas, y por la puerta de Ring 3 no cabe un texto.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Recorte {
    /// Se mando. Sectores cubiertos y ordenes que costo.
    Hecho { sectores: u64, ordenes: u64 },
    /// No hay disco listo.
    SinDisco,
    /// El disco no declara TRIM (palabra 169). No se manda **a ver si suena**.
    NoLoSoporta,
    /// El gate de identidad o la ventana dijeron que no. Trae su motivo.
    SinPermiso(&'static str),
    /// El rango no se puede ni escribir: cero sectores, o fuera de LBA48.
    RangoImposible,
    /// El disco fallo a mitad. Lleva lo que SI se llego a recortar.
    Fallo { sectores: u64 },
}

impl Recorte {
    pub fn name(self) -> &'static str {
        match self {
            Recorte::Hecho { .. } => "recortado",
            Recorte::SinDisco => "sin disco",
            Recorte::NoLoSoporta => "este disco no declara TRIM (palabra 169)",
            Recorte::SinPermiso(m) => m,
            Recorte::RangoImposible => "el rango no es representable (cero, o fuera de LBA48)",
            Recorte::Fallo { .. } => "el disco rechazo el DATA SET MANAGEMENT",
        }
    }
}

/// Sectores devueltos al disco desde el arranque.
static mut RECORTADOS: u64 = 0;
/// Ordenes de `DATA SET MANAGEMENT` mandadas desde el arranque.
static mut ORDENES: u64 = 0;

/// `(sectores, ordenes)` desde el arranque.
///
/// ** Los dos, y no solo el primero. Un mismo numero de sectores en una orden o
/// en trescientas dice cosas distintas del techo que declara el disco (la
/// palabra 105), y esa division es la unica pista si un dia recortar se vuelve
/// lento.
pub fn cuentas_trim() -> (u64, u64) { unsafe { (RECORTADOS, ORDENES) } }

/// **Recorta `[lba, lba+sectores)`.** El rango es de LBA absolutos del disco.
///
/// El bucle de tandas esta aqui y no en el driver porque es donde se tiene el
/// dueno del disco: se toma UNA vez para todas las ordenes. Soltarlo entre
/// tandas dejaria que otra tarea pisara la ranura 0 en mitad de un recorte.
pub fn recortar(lba: u64, sectores: u64) -> Recorte {
    if !is_ready() {
        return Recorte::SinDisco;
    }
    // ** LO QUE EL DISCO DIJO, y no lo que nos convenga. Sin esto, un disco que
    // no soporta DSM contestaria con un error de task file que se leeria como
    // "el driver esta roto".
    if !perfil::trim_soportado() {
        return Recorte::NoLoSoporta;
    }
    // El MISMO guardian que la escritura, con el rango entero. Ver
    // `bmo_block::ventana::decidir_rango`: preguntar por trozos dejaria sin
    // mirar el final del rango.
    if let Err(why) = bmo_block::ventana::decidir_rango(
        is_ready(),
        write_armed(),
        data_partition().map(|w| (w.first_lba, w.last_lba)),
        ventana_estratos(),
        lba,
        sectores,
    ) {
        return Recorte::SinPermiso(why);
    }
    let Some(mut rango) = Rango::nuevo(lba, sectores) else {
        return Recorte::RangoImposible;
    };

    let dma = unsafe { DMA_PHYS };
    if dma == 0 {
        return Recorte::SinDisco;
    }

    let mut hechos = 0u64;
    let mut ordenes = 0u64;
    let mut roto = false;
    {
        // El disco es de UNO cada vez, y aqui durante TODAS las tandas: soltarlo
        // entre dos ordenes dejaria que otra tarea pisara la ranura 0 --y la
        // pagina de rebote-- en mitad del recorte.
        let _testigo = tomar_disco();

        // La misma pagina de rebote que las lecturas: 4 KiB son 8 bloques de
        // payload, o sea hasta 512 descriptores. El techo de verdad lo pone el
        // disco con la palabra 105, y por eso se le pregunta.
        let buf = unsafe {
            core::slice::from_raw_parts_mut(mm::phys_to_virt(dma) as *mut u8, 4096)
        };
        let techo = perfil::trim_bloques_max();

        while let Some(tanda) = rango.siguiente(buf, techo) {
            if let Err(e) = unsafe { bmo_ahci::trim_phys(unsafe { PORT }, dma, tanda.bloques) } {
                // El LBA donde se quedo, que es lo unico util para mirarlo con
                // otra herramienta. Lo de antes ya esta recortado de verdad.
                crate::ring0::cabina::fault("disk", e.name(), lba + hechos);
                roto = true;
                break;
            }
            hechos += tanda.sectores;
            ordenes += 1;
        }
        unsafe {
            RECORTADOS += hechos;
            ORDENES += ordenes;
        }
    }

    // ** Y LA BARRERA DETRAS, que no es paranoia.
    //
    // Un TRIM aceptado esta en la cache del aparato igual que una escritura, y
    // este disco no tiene condensadores: `FLUSH CACHE` es lo unico que tiene
    // para terminar lo que empezo (`DISCO_JUICIO_SOLO_BARRERA`). Sin esto, el
    // sistema diria "devuelto" de algo que el disco todavia no ha asumido.
    //
    // [!] Va FUERA del bloque de arriba a proposito: `flush` toma el disco por su
    // cuenta, y pedirlo teniendolo ya seria una peticion anidada -- que el dueno
    // atiende, pero avisando en CABINA de algo que aqui no es un fallo.
    //
    // Un fallo aqui NO invalida el recorte: los sectores estan igual de libres.
    // Se dice y se sigue.
    if hechos > 0 && !flush() {
        crate::ring0::cabina::warn("disk", "el recorte no se pudo vaciar al plato", hechos);
    }
    if roto {
        return Recorte::Fallo { sectores: hechos };
    }
    Recorte::Hecho { sectores: hechos, ordenes }
}
