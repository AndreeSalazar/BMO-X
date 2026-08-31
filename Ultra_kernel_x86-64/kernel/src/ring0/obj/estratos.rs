//! **LA PUERTA DE ESTRATOS PARA UN `Archivo`** -- tramo 1.1 del plan.
//!
//! [carril]  AMARILLO  la puerta de ESTRATOS para un Archivo
//!
//! generacion: nieto -- CADENA DE LLAMADAS, no tuberia: esta etiqueta dice
//! cuanto SABE esta pieza, no quien importa a quien, y por eso el
//! guardian de L7 no la juzga (ver L7c en `META-KERNEL_HARD.md`).
//! no sabe: quien lo llamo ni por que
//!
//! === El agujero que tapa ===
//!
//! Desde el 18-08 se puede ESCRIBIR un fichero en ESTRATOS desde Ring 3, y no
//! se podia volver a leer. El kernel si sabia --`est::open` + `est::read`, que
//! usa `task/launch.rs` para arrancar un `.bex` de ESTRATOS-- pero **no habia
//! puerta**: `obj::file` resolvia rutas solo por `fsys::fs`, o sea FAT32.
//!
//! Escribir sin poder releer es media funcion. Esto es la otra mitad, y es la
//! mas barata del tramo porque faltaba la puerta, no el motor.
//!
//! === La regla de a que volumen apunta una ruta ===
//!
//! **ESTRATOS primero; si no esta ahi, FAT32.** No es una regla nueva: es la
//! que `task::launch::Fuente::abrir` lleva usando para localizar un binario, y
//! aplicarla tambien aqui es lo que hace que abrir un fichero y ejecutarlo
//! resuelvan al MISMO fichero. Dos reglas distintas para la misma ruta seria la
//! peor de las opciones.
//!
//! [!] Y el riesgo, dicho: un nombre que exista en los dos volumenes resuelve a
//! ESTRATOS. Hoy es posible --`escribe` crea en FAT32 y `nuevo` en ESTRATOS, y
//! los dos volumenes tienen un `apps/`-- asi que puede pasar. Se acepta porque
//! ya pasaba con lo mas peligroso que hay (ejecutar un programa), y porque la
//! alternativa --inventar un prefijo de volumen-- seria una segunda convencion
//! para el mismo problema.
//!
//! === Por que el fichero entra ENTERO en el buffer ===
//!
//! Un archivo de FAT32 se refleja: se guarda un cursor de doce bytes y los
//! bytes van del disco al que los pide, cuando los pide (ver la cabecera de
//! `file.rs`). Aqui no, y no es por vagancia:
//!
//! 1. **Un fichero de ESTRATOS no es una cadena.** Es un atributo `:datos` con
//!    su arbol de indireccion, y `est::read` lo reconstruye entero de una vez
//!    (`bmo_estratos::read::descender`). No hay un "siguiente trozo" que pedir
//!    sin rehacer el recorrido.
//! 2. ~~**Hoy miden como mucho 96 bytes**~~ -- **YA NO, y esta nota ha
//!    VENCIDO**. Decia *"cuando el tramo 1.2 suba ese techo, esta decision hay
//!    que volver a mirarla"*, y el 1.2 se hizo el 19-08: `flujo` parte el
//!    contenido en bloques y `ES_GESTO_ORIGEN` lo entrega desde Ring 3. Un
//!    fichero de ESTRATOS puede medir MiB.
//!
//! ** LO QUE SE DECIDE AL VOLVER A MIRARLA (2026-08-20): se sigue trayendo
//! ENTERO, y el tope deja de ser un numero para ser una pregunta al asignador.
//! `reserve` pide marcos FISICOS CONTIGUOS del tamano del fichero: un `.bex` de
//! 4 MiB son 1.024 paginas seguidas, y si la RAM esta fragmentada contesta
//! `ERROR_TOO_LARGE`. No se rompe -- se niega, que es lo correcto.
//!
//! Se mantiene porque el cliente de hoy es `launch`, y **un binario se necesita
//! entero**: leerlo por trozos no ahorraria nada. El dia que haya un VISOR la
//! cuenta cambia --una pantalla de texto son dos KiB de un fichero que puede
//! medir cuatro MiB-- y entonces hara falta leer un RANGO. Esa es la nota nueva,
//! y como la de antes, se escribe en vez de suponerse.

use crate::ring0::fsys::estratos as est;

/// Busca `ruta` en ESTRATOS. Devuelve el nodo y lo que mide su `:datos`.
///
/// `None` es "aqui no esta", y **no es un error**: el que llama sigue por
/// FAT32. Por eso no hay motivo que dar -- el motivo lo dara el otro volumen si
/// tampoco lo tiene, que es donde se puede distinguir "no existe" de "eso es
/// una carpeta".
pub(super) fn buscar(ruta: &str) -> Option<(est::Nodo, usize)> {
    if !est::is_mounted() {
        return None;
    }
    let n = est::open(ruta)?;
    // Un directorio NO se abre como archivo. Se contesta que aqui no esta y
    // FAT32 tendra su turno; si alli tampoco, el error que llegue sera suyo.
    // Colarlo como archivo de cero bytes seria peor: un `lee apps` devolveria
    // un handle vacio en vez de decir que eso es una carpeta.
    if n.tipo != bmo_estratos::objects::Tipo::Directorio {
        let mide = n
            .attr(bmo_estratos::objects::ATTR_DATOS)
            .map(|a| a.size as usize)
            .unwrap_or(0);
        return Some((n, mide));
    }
    None
}

/// Lee el contenido de `n` en `dst`. Devuelve los bytes que entraron.
///
/// Cero es un contenido perfectamente valido --un fichero vacio-- y tambien lo
/// que se contesta si el nodo no tiene `:datos`. Las dos cosas se ven igual
/// desde fuera y esta bien que asi sea: en los dos casos no hay nada que leer.
pub(super) fn leer(n: &est::Nodo, dst: &mut [u8]) -> usize {
    est::read(n, dst).unwrap_or(0)
}
