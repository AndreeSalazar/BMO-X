//! **EL RENGLON DE LOS GESTOS SOBRE ESTRATOS** -- crear, borrar, renombrar.
//!
//! === Por que es un fichero, y por que se llama asi ===
//!
//! Por L6a: `syscall/mod.rs` esta en la linea base del censo y **no puede
//! crecer**. Pero el corte no es por tamano -- es que este brazo del despacho
//! dejo de ser "crear un fichero" el dia que la maquina de abajo aprendio
//! cuatro verbos.
//!
//! Se llamaba `TASK_OP_ES_CREAR`, y un renglon que tambien BORRA no se puede
//! seguir llamando "crear": un nombre que miente es peor que uno feo, porque el
//! que lo lee deja de comprobar.
//!
//! === Un renglon, cuatro verbos ===
//!
//! Igual que abajo. `fsys::estratos::escribir::aplicar` es UNA maquina con un
//! `Gesto` de cuatro variantes; aqui hay UNA operacion con cuatro subordenes.
//! **La forma de la puerta es la forma del codigo que sirve**, y eso no es
//! estetica: dos puertas para una maquina es como una de las dos se queda sin
//! el arreglo que se le hizo a la otra.
//!
//! ```text
//!   LIMPIAR     vacia el renglon del contenido
//!   DATOS       mete ocho bytes en el (contenido, o el nombre nuevo)
//!   FICHERO     crea un fichero con lo acumulado dentro
//!   CARPETA     crea una carpeta vacia
//!   QUITAR      quita una entrada
//!   RENOMBRAR   le cambia el nombre a una entrada
//! ```
//!
//! === ** LA RUTA LLEVA EL DESTINO ENTERO, y el kernel la parte ===
//!
//! El renglon de la ruta trae `datos/notas/x.txt` y aqui se corta en
//! `("datos/notas", "x.txt")`. La alternativa era un tercer renglon para el
//! nombre, y no hace falta: **una ruta ya contiene su ultimo tramo**.
//!
//! Ademas es lo que uno escribe. `borra datos/notas/x.txt` es una frase; una
//! carpeta por un canal y un nombre por otro es un formulario.
//!
//! [!] `RENOMBRAR` es el unico que necesita dos nombres, y el segundo va por el
//! renglon del CONTENIDO. No es un apano: ese renglon lleva una cuenta explicita
//! de bytes, asi que un nombre entra tal cual y sin ambiguedad.
//!
//! === Y CABINA no esta aqui ===
//!
//! A proposito. El aviso de que un gesto fallo vive en `escribir::aplicar`, que
//! es por donde pasan los cuatro: ponerlo en cada brazo de este `match` seria
//! cuatro sitios donde olvidarse del quinto. Aqui solo se anota QUIEN lo pidio,
//! que es lo unico que este lado sabe y el otro no.

use super::ops::*;
use super::{datos_limpiar, datos_meter, datos_tomar, ruta_tomar};
use crate::ring0::fsys::estratos::escribir::{self, Gesto};

/// Parte `a/b/c.txt` en `("a/b", "c.txt")`.
///
/// Sin tramo final devuelve `None`: `borra datos/` no dice que borrar, y
/// adivinarlo --tomar `datos` como el objetivo-- seria borrar la carpeta cuando
/// se pidio borrar algo de dentro.
fn partir(ruta: &str) -> Option<(&str, &str)> {
    let ruta = ruta.trim_end_matches(['/', '\\']);
    if ruta.is_empty() {
        return None;
    }
    match ruta.rfind(['/', '\\']) {
        Some(i) => {
            let nombre = &ruta[i + 1..];
            if nombre.is_empty() {
                None
            } else {
                Some((&ruta[..i], nombre))
            }
        }
        // Sin barras: esta en la raiz, y la raiz es la ruta vacia.
        None => Some(("", ruta)),
    }
}

/// Sirve una suborden del renglon. Devuelve la generacion nueva, o `0`.
///
/// ** El `0` es "no se hizo" y no trae el motivo, igual que antes: el motivo va
/// a CABINA, que es donde caben las frases. Ring 3 no puede hacer nada distinto
/// con "no cabe" que con "esa ruta no existe" salvo ensenarselo a una persona,
/// y para eso esta F11.
pub(super) fn servir(pid: u32, arg0: u64, arg1: u64) -> u64 {
    match arg0 & 0xFF {
        ES_GESTO_LIMPIAR => {
            datos_limpiar(pid);
            0
        }
        // `arg1` son los ocho bytes y los bits altos de `arg0` CUANTOS valen. La
        // ruta se corta en el primer cero porque en una ruta un cero no puede
        // aparecer; en un contenido SI, asi que aqui la cuenta es explicita o se
        // entregaria la mitad de un fichero.
        ES_GESTO_DATOS => datos_meter(pid, arg1, arg0 >> 8) as u64,
        ES_GESTO_FICHERO => hacer(pid, "fichero nuevo", |ruta, datos| {
            let (dir, nombre) = partir(ruta)?;
            Some(Gesto::Fichero { nombre, datos })
                .map(|g| escribir::aplicar(dir, g))
        }),
        ES_GESTO_CARPETA => hacer(pid, "carpeta nueva", |ruta, _| {
            let (dir, nombre) = partir(ruta)?;
            Some(escribir::aplicar(dir, Gesto::Carpeta { nombre }))
        }),
        ES_GESTO_QUITAR => hacer(pid, "quitar una entrada", |ruta, _| {
            let (dir, nombre) = partir(ruta)?;
            Some(escribir::aplicar(dir, Gesto::Quitar { nombre }))
        }),
        ES_GESTO_COPIA => hacer(pid, "copiar un fichero de FAT32", |ruta, datos| {
            let (dir, nombre) = partir(ruta)?;
            // El ORIGEN viene por el renglon del contenido, igual que el nombre
            // nuevo de `renombrar`. Son dos nombres y ningun byte de fichero.
            let origen = core::str::from_utf8(datos).ok()?;
            if origen.is_empty() {
                return None;
            }
            Some(escribir::aplicar(dir, Gesto::Copia { nombre, origen }))
        }),
        // ** El unico que NO parte la ruta: aqui no hay destino, hay un
        // NOMBRE. Partirlo por la ultima barra convertiria `copia de ayer` en
        // otra cosa el dia que alguien use una barra en un nombre.
        ES_GESTO_MARCAR => {
            let nombre = ruta_tomar(pid);
            datos_tomar(pid);
            crate::ring0::cabina::info("estratos", "marcar la version", pid as u64);
            escribir::marcar(nombre).unwrap_or(0)
        }
        // El unico que no necesita ningun renglon: el numero cabe en `arg1`.
        // Pedir una ruta para esto habria sido inventar un texto donde ya hay
        // un entero.
        ES_GESTO_VOLVER => {
            crate::ring0::cabina::info("estratos", "volver a una version", pid as u64);
            escribir::volver(arg1 as usize).unwrap_or(0)
        }
        ES_GESTO_RENOMBRAR => hacer(pid, "renombrar una entrada", |ruta, datos| {
            let (dir, viejo) = partir(ruta)?;
            // El nombre nuevo viene por el renglon del contenido.
            let nuevo = core::str::from_utf8(datos).ok()?;
            if nuevo.is_empty() {
                return None;
            }
            Some(escribir::aplicar(dir, Gesto::Renombrar { viejo, nuevo }))
        }),
        // Una suborden que no existe contesta cero y no un fallo: quien la mande
        // se entera igual, y un `unsupported` obligaria al que llama a
        // distinguir dos formas de "no paso nada".
        _ => 0,
    }
}

/// El molde de los cuatro verbos: anotar quien lo pide, vaciar los renglones y
/// llamar.
///
/// * Los renglones se vacian SIEMPRE, salga bien o mal. Si no, un gesto que
/// falla le dejaria la ruta puesta al siguiente -- y el siguiente puede ser un
/// `quitar`.
fn hacer(
    pid: u32,
    que: &'static str,
    construir: impl FnOnce(&str, &[u8]) -> Option<Result<u64, crate::ring0::fsys::estratos::WriteError>>,
) -> u64 {
    let ruta = ruta_tomar(pid);
    let datos = datos_tomar(pid);
    // Lo unico que este lado sabe y el otro no: QUIEN lo ha pedido. El resto de
    // la historia --que paso y por que-- lo cuenta `escribir::aplicar`.
    crate::ring0::cabina::info("estratos", que, pid as u64);
    match construir(ruta, datos) {
        Some(Ok(g)) => g,
        Some(Err(_)) => 0,
        // La ruta no daba para un gesto: sin tramo final, o un nombre nuevo que
        // no es texto. Se dice aqui porque `aplicar` no llega a verlo.
        None => {
            crate::ring0::cabina::warn("estratos", "la ruta del gesto no vale", pid as u64);
            0
        }
    }
}
