//! **El cursor de ESTRATOS**: recorrer el almacen desde Ring 3.
//!
//! === Que es y que NO es ===
//!
//! Un cursor, no un punado de handles. La ventana que lo usa mira un sitio a la
//! vez y lo que quiere es *bajar, subir y listar* -- y un puntero que se mueve
//! es exactamente eso. Una capability por nodo abierto pediria tabla, ciclo de
//! vida y revocacion para modelar lo mismo.
//!
//! * **No concede nada**: aqui no hay ni una operacion que escriba. Es el mismo
//! trato que [`info`] y que [`klog_texto`] -- contesta, no autoriza.
//!
//! [!] **El cursor es UNO y es del sistema**, no de este proceso. Dos ventanas
//! recorriendo el arbol a la vez se pisarian. Hoy solo lo usa el panel de
//! Datos; el dia que sean dos, esto pide un handle por cliente y se dice ahora
//! para que nadie lo descubra por el sintoma.
//!
//! Era un `pub mod` dentro de `lib.rs`. Mismo codigo, fichero propio.

use crate::*;

/// Tipo de un nodo, tal como lo cuenta el cursor.
pub const ARCHIVO: u64 = 0;
pub const DIRECTORIO: u64 = 1;
/// No hay nodo ahi, o no se pudo leer. **Es distinto de "es un archivo"**:
/// confundirlos pinta una caja para algo que no existe.
pub const NOTHING: u64 = 2;

fn pregunta(que: u64, arg: u64) -> u64 {
    invoke(CURRENT_TASK, OP_ES_NODO, que, arg, 0).value
}

/// Pone el cursor en la raiz. `false` si no hay volumen montado.
pub fn a_la_raiz() -> bool {
    pregunta(0x00, 0) != 0
}
/// Cuantos hijos tiene el nodo actual.
pub fn hijos() -> u64 {
    pregunta(0x01, 0)
}
/// Se quedo el listado corto? Un directorio truncado en silencio se ve
/// igual que uno con pocos archivos.
pub fn truncado() -> bool {
    pregunta(0x02, 0) != 0
}
/// Cuantos niveles se ha bajado desde la raiz.
pub fn hondo() -> u64 {
    pregunta(0x03, 0)
}
/// Tipo del nodo actual: [`ARCHIVO`], [`DIRECTORIO`] o [`NOTHING`].
pub fn tipo() -> u64 {
    pregunta(0x04, 0)
}
/// Tipo del hijo `i`.
pub fn hijo_tipo(i: u64) -> u64 {
    pregunta(0x05, i)
}
/// Baja al hijo `i`. `false` si no existe o no es un directorio.
pub fn entrar(i: u64) -> bool {
    pregunta(0x06, i) != 0
}
/// Vuelve al padre. `false` si ya estaba en la raiz.
pub fn subir() -> bool {
    pregunta(0x07, 0) != 0
}

// -- El DETALLE del hijo `i` -----------------------------------------

/// Bytes de su contenido. Un directorio contesta lo que ocupa su LISTA de
/// entradas, que es un dato distinto de lo que hay dentro.
pub fn hijo_bytes(i: u64) -> u64 {
    pregunta(0x08, i)
}
/// Cuantos atributos lleva. Es el numero que dice que un nodo de ESTRATOS
/// **es un conjunto de atributos** y no una carpeta con cosas.
pub fn hijo_atributos(i: u64) -> u64 {
    pregunta(0x09, i)
}
/// Lleva `:firma`? **Solo si la lleva, no si cuadra.**
pub fn hijo_firmado(i: u64) -> bool {
    pregunta(0x0A, i) != 0
}

/// Lo que contesta [`verificar`].
pub const FIRMA_AUSENTE: u64 = 0;
pub const FIRMA_CUADRA: u64 = 1;
pub const FIRMA_NO_CUADRA: u64 = 2;
pub const FIRMA_ILEGIBLE: u64 = 3;
/// * **El archivo no cabe en el buffer de verificacion**, y eso es un limite
/// NUESTRO, no una averia del disco.
///
/// Compartia numero con `FIRMA_ILEGIBLE`, asi que un archivo perfectamente sano
/// de mas de 256 KiB se pintaba **en rojo** como "no se pudo leer" -- y en esta
/// ventana el rojo significa *"hay un problema en el disco"*. El archivo se lee
/// sin problema; lo que no cabe es la comprobacion.
pub const FIRMA_NO_CABE: u64 = 4;

/// * Lee el hijo entero y compara su BLAKE3 con su `:firma`.
///
/// **Se pide a mano**: leer un archivo y hacerle un hash sesenta veces por
/// segundo convertiria un panel en un martillo sobre el disco.
///
/// [!] Demuestra que los bytes son los que se guardaron. **No demuestra
/// autenticidad** -- quien pueda escribir en el volumen puede cambiar el
/// archivo *y* recalcular su hash.
pub fn verificar(i: u64) -> u64 {
    pregunta(0x0B, i)
}

/// El nombre del nivel `nivel` de la ruta en `dst`. `0` es la raiz y
/// contesta vacio -- quien pinta escribe `/`.
///
/// Va por la misma puerta que [`hijo_nombre`] con un `1` en los bits altos:
/// es el mismo mecanismo pidiendo otra cosa, y una puerta por cada texto
/// que devuelva el sistema es como una superficie de dos syscalls acaba
/// teniendo treinta.
pub fn nombre_nivel(nivel: u64, dst: &mut [u8]) -> usize {
    texto_de(nivel | (1u64 << 32), dst)
}

/// El nombre del hijo `i` en `dst`. Devuelve cuantos bytes se escribieron.
///
/// Viaja de ocho en ocho, igual que el klog: la superficie congelada no
/// acepta punteros. Se para en el primer trozo vacio o cuando `dst` se
/// llena -- lo que pase antes.
pub fn hijo_nombre(i: u64, dst: &mut [u8]) -> usize {
    texto_de(i, dst)
}

/// El motor de los dos: saca un texto de ocho en ocho hasta que se acabe o
/// hasta que `dst` se llene, lo que pase antes.
fn texto_de(que: u64, dst: &mut [u8]) -> usize {
    let mut escritos = 0usize;
    let mut trozo = 0u64;
    while escritos < dst.len() {
        let w = invoke(CURRENT_TASK, OP_ES_TEXTO, que, trozo, 0).value;
        if w == 0 {
            break;
        }
        for b in w.to_le_bytes() {
            if b == 0 || escritos >= dst.len() {
                return escritos;
            }
            dst[escritos] = b;
            escritos += 1;
        }
        trozo += 1;
    }
    escritos
}

// == ** CREAR UN FICHERO ====================================================
//
// Lo primero de este modulo que ESCRIBE. Todo lo de arriba contesta preguntas
// --a la raiz, los hijos, los nombres-- y esto cambia el volumen.

pub const ES_CREAR_LIMPIAR: u64 = 0x00;
pub const ES_CREAR_DATOS: u64 = 0x01;
pub const ES_CREAR_HACER: u64 = 0x02;
/// Lo que cabe DENTRO del nodo, sin gastar un bloque de datos.
pub const ES_CREAR_MAX: u64 = 96;

/// **Crea `nombre` con `datos` dentro.** Devuelve la generacion nueva, o `0`.
///
/// === Como cruza, y por que asi ===
///
/// El nombre por el renglon de [`OP_RUTA`] --el mismo que ya usan `ejecutar` y
/// los dos de archivo-- y el contenido por el suyo, de 8 en 8. La superficie
/// congelada no acepta punteros y no hay `copy_from_user`: pasar una direccion
/// de Ring 3 obligaria al kernel a traducirla contra el espacio del llamante.
///
/// ** Y el contenido lleva CUENTA. La ruta se corta en su primer cero porque en
/// una ruta un cero no puede aparecer; en un fichero **si puede**, y cortarlo
/// ahi seria guardar la mitad de un fichero sin que nada fallara.
///
/// El `0` no dice por que: el motivo va a CABINA (F11), que es donde caben las
/// frases. Los cuatro que se ven en la practica son no caber en 96 bytes, que el
/// nombre este repetido, que la carpeta este llena y que la escritura este
/// cerrada.
pub fn crear_fichero(nombre: &[u8], datos: &[u8]) -> u64 {
    if datos.len() as u64 > ES_CREAR_MAX {
        return 0;
    }
    // El nombre primero, por el renglon de siempre.
    let mut i = 0;
    while i < nombre.len() {
        let mut w = [0u8; 8];
        let n = (nombre.len() - i).min(8);
        w[..n].copy_from_slice(&nombre[i..i + n]);
        invoke(CURRENT_TASK, OP_RUTA, u64::from_le_bytes(w), 0, 0);
        i += 8;
    }
    // Y el contenido por el suyo, limpiando antes: un intento a medias de hace
    // un rato no puede envenenar este.
    invoke(CURRENT_TASK, OP_ES_CREAR, ES_CREAR_LIMPIAR, 0, 0);
    let mut i = 0;
    while i < datos.len() {
        let mut w = [0u8; 8];
        let n = (datos.len() - i).min(8);
        w[..n].copy_from_slice(&datos[i..i + n]);
        invoke(
            CURRENT_TASK,
            OP_ES_CREAR,
            ES_CREAR_DATOS | ((n as u64) << 8),
            u64::from_le_bytes(w),
            0,
        );
        i += 8;
    }
    invoke(CURRENT_TASK, OP_ES_CREAR, ES_CREAR_HACER, 0, 0).value
}
