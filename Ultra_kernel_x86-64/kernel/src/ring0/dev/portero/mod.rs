//! **EL PORTERO DEL BUS: que hay enchufado, y quien puede alcanzar la RAM.**
//!
//! [carril]  VERDE     ni una linea de codigo: dos `mod` y sus reexportaciones
//!
//! [cuesta]  NADA -- aqui no se ejecuta nada. Lo unico que puede estar mal es
//!           un nombre, y un nombre mal escrito no llega a arrancar: lo para el
//!           compilador (L6e)
//!
//! [riesgo]  -- ninguno declarado.
//!
//! # *** POR QUE ESTE `mod.rs` ES VERDE Y EL DE `mm/titular/` ES ROJO
//!
//! Los dos son la puerta de una carpeta de carriles, y no valen lo mismo:
//!
//! ```text
//!    mm/titular/mod.rs   lleva `indice`, `de_byte` y el byte por marco.
//!                        CODIGO que los tres carriles EJECUTAN        ROJO
//!    este                lleva dos `mod` y tres nombres.
//!                        Nada que ejecutar                           VERDE
//! ```
//!
//! ** La regla *"un cimiento no puede ser mas verde que el mas rojo de los que
//! lo usan"* habla de codigo compartido, no de una lista de nombres. Un fichero
//! que solo dice donde esta cada cosa no puede fallar en marcha -- o compila, o
//! no hay arranque.
//!
//! # LOS DOS CARRILES, Y ESTA VEZ EL REPARTO NO ES UNA OPINION
//!
//! ```text
//!    verde.rs   EL CENSO    recorre el bus y CUENTA. No escribe ni un bit
//!    roja.rs    EL DURO     ESCRIBE en la configuracion de PCI: le retira el
//!                           bit de maestro a quien no deberia tenerlo
//! ```
//!
//! *** Y estan en dos ficheros por la misma frase que separo este portero del
//! del USB: **apuntar no puede cambiar la decision**. Mientras contar y cerrar
//! vivieran en el mismo sitio, el dia que el censo se equivocara el error se
//! cobraria en el bus. Ahora el que cuenta no sabe escribir.
//!
//! > Un censo que ademas actua deja de ser un censo el dia que se equivoca.
//!
//! # Los dos porteros que ya habia, y este hace tres
//!
//! ```text
//!    dev/usb/portero.rs   que LLEGO a un puerto, y que se le contesto
//!    portero/verde.rs     que HAY en la placa, y para que hay codigo
//!    portero/roja.rs      quien alcanza la RAM Y NO TENDRIA QUE PODER
//! ```

mod roja;
mod verde;

pub use roja::{adoptado, ajenos, duro, Cerrojo, EL_CERROJO};
pub use verde::{censar, stats, APARATOS_CENSADOS};
