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
//! ** Y ese dia NO fue el del panel de arbol (2026-08-18). Parecia que si: un
//! arbol a la izquierda y una rejilla a la derecha suenan a dos recorridos. No
//! lo son -- son **el mismo recorrido mirado a dos profundidades**, y lo que
//! faltaba no era otro cursor sino que este no se olvidara de por donde ha
//! pasado. Ver `nivel_hijos` y compania, aqui abajo.
//!
//! El aviso sigue en pie tal cual para el caso de verdad: dos ventanas en
//! sitios DISTINTOS.
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

// == ** EL ARBOL: los niveles por los que YA se ha pasado ===================
//
// Un panel de arbol --el de la izquierda de cualquier explorador-- ensena a la
// vez los hijos de la raiz, los del nivel siguiente y los del siguiente, con la
// rama por la que has bajado marcada. Con las funciones de arriba no se puede:
// todas contestan del nivel donde ESTA el cursor.
//
// * Y la respuesta NO fue un segundo cursor. El aviso de la cabecera de este
// fichero decia que dos clientes pedirian un handle por cliente, y sigue siendo
// verdad para dos VENTANAS mirando sitios distintos. El arbol y la rejilla no
// son eso: son el mismo recorrido a dos profundidades. Lo que se arreglo es que
// el cursor no se olvide de por donde ha pasado -- `fsys/estratos/nivel.rs`.
//
// Ninguna de las cuatro toca el disco.

/// Cuantos hijos tiene el nivel `nivel`. `0` si no se ha llegado a el.
pub fn nivel_hijos(nivel: u64) -> u64 {
    pregunta(0x0C, nivel)
}

/// El tipo del hijo `i` del nivel `nivel`: [`ARCHIVO`], [`DIRECTORIO`] o
/// [`NOTHING`].
pub fn nivel_hijo_tipo(nivel: u64, i: u64) -> u64 {
    pregunta(0x0D, (nivel << 32) | (i & 0xFFFF_FFFF))
}

/// Por que hijo se bajo desde el nivel `nivel`. [`NINGUNO`] si por ninguno.
pub fn nivel_elegido(nivel: u64) -> u64 {
    pregunta(0x0E, nivel)
}

/// Lo que contesta [`nivel_elegido`] cuando de ese nivel no se bajo.
///
/// ** No es cero, y no puede serlo: **cero es el primer hijo**. Un arbol que
/// tomara el cero como "ninguno" pintaria siempre la primera rama abierta.
pub const NINGUNO: u64 = u64::MAX;

/// **Relee el arbol y deja el cursor donde estaba.**
///
/// Se manda DESPUES de cualquier gesto que escriba. Sin esto el cursor sigue
/// ensenando el estrato de antes: borrarias un fichero y ahi seguiria.
///
/// Es la unica del cursor que toca el disco, y por eso se pide a mano en vez de
/// hacerse sola en cada repintado.
pub fn recargar() -> bool {
    pregunta(0x0F, 0) != 0
}

/// El nombre del hijo `i` del nivel `nivel`, en `dst`.
pub fn nivel_hijo_nombre(nivel: u64, i: u64, dst: &mut [u8]) -> usize {
    texto_de((2u64 << 32) | ((nivel & 0xFFFF) << 16) | (i & 0xFFFF), dst)
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

// == ** LOS CUATRO GESTOS ==================================================
//
// Lo unico de este modulo que ESCRIBE. Todo lo de arriba contesta preguntas
// --a la raiz, los hijos, los nombres-- y esto cambia el volumen.
//
// * UNA operacion y cuatro subordenes, porque debajo hay UNA maquina y cuatro
// verbos (`fsys::estratos::escribir::Gesto`). La forma de la puerta es la forma
// del codigo que sirve.

pub const ES_GESTO_LIMPIAR: u64 = 0x00;
pub const ES_GESTO_DATOS: u64 = 0x01;
pub const ES_GESTO_FICHERO: u64 = 0x02;
pub const ES_GESTO_CARPETA: u64 = 0x03;
pub const ES_GESTO_QUITAR: u64 = 0x04;
pub const ES_GESTO_RENOMBRAR: u64 = 0x05;
pub const ES_GESTO_COPIA: u64 = 0x06;
/// Lo que cabe DENTRO del nodo, sin gastar un bloque de datos.
pub const ES_GESTO_MAX: u64 = 96;

/// Manda la ruta por el renglon de [`OP_RUTA`], de ocho en ocho.
///
/// El mismo que ya usan `ejecutar` y los dos de archivo: la superficie congelada
/// no acepta punteros y no hay `copy_from_user`, asi que pasar una direccion de
/// Ring 3 obligaria al kernel a traducirla contra el espacio del llamante.
fn mandar_ruta(ruta: &[u8]) {
    let mut i = 0;
    while i < ruta.len() {
        let mut w = [0u8; 8];
        let n = (ruta.len() - i).min(8);
        w[..n].copy_from_slice(&ruta[i..i + n]);
        invoke(CURRENT_TASK, OP_RUTA, u64::from_le_bytes(w), 0, 0);
        i += 8;
    }
}

/// Manda el contenido por su renglon, limpiando antes.
///
/// ** Y lleva CUENTA. La ruta se corta en su primer cero porque en una ruta un
/// cero no puede aparecer; en un contenido **si puede**, y cortarlo ahi seria
/// guardar la mitad de un fichero sin que nada fallara.
///
/// Se limpia siempre, incluso para mandar nada: un intento a medias de hace un
/// rato no puede envenenar este.
fn mandar_datos(datos: &[u8]) {
    invoke(CURRENT_TASK, OP_ES_GESTO, ES_GESTO_LIMPIAR, 0, 0);
    let mut i = 0;
    while i < datos.len() {
        let mut w = [0u8; 8];
        let n = (datos.len() - i).min(8);
        w[..n].copy_from_slice(&datos[i..i + n]);
        invoke(
            CURRENT_TASK,
            OP_ES_GESTO,
            ES_GESTO_DATOS | ((n as u64) << 8),
            u64::from_le_bytes(w),
            0,
        );
        i += 8;
    }
}

/// **Crea `ruta` con `datos` dentro.** Devuelve la generacion nueva, o `0`.
///
/// ** `ruta` es el destino ENTERO --`datos/notas/x.txt`-- y el kernel le corta
/// el ultimo tramo. Antes era solo un nombre y por eso solo se podia crear en la
/// raiz; una ruta ya contiene su nombre, asi que no hacia falta un segundo
/// canal, hacia falta leerla entera.
///
/// El `0` no dice por que: el motivo va a CABINA (F11), que es donde caben las
/// frases. Los que se ven en la practica son no caber en 96 bytes, el nombre
/// repetido, la carpeta llena, la ruta que no existe y la escritura cerrada.
pub fn crear_fichero(ruta: &[u8], datos: &[u8]) -> u64 {
    if datos.len() as u64 > ES_GESTO_MAX {
        return 0;
    }
    mandar_ruta(ruta);
    mandar_datos(datos);
    invoke(CURRENT_TASK, OP_ES_GESTO, ES_GESTO_FICHERO, 0, 0).value
}

/// **Crea la carpeta `ruta`**, vacia. Devuelve la generacion nueva, o `0`.
pub fn crear_carpeta(ruta: &[u8]) -> u64 {
    mandar_ruta(ruta);
    mandar_datos(&[]);
    invoke(CURRENT_TASK, OP_ES_GESTO, ES_GESTO_CARPETA, 0, 0).value
}

/// **Quita `ruta`.** Devuelve la generacion nueva, o `0`.
///
/// ** No destruye nada: publica un arbol sin esa entrada. El nodo, su contenido
/// y el estrato de ayer siguen donde estaban -- **borrar aqui es dejar de
/// nombrar**. Lo que se suelta de verdad es cosa del recolector.
pub fn quitar(ruta: &[u8]) -> u64 {
    mandar_ruta(ruta);
    mandar_datos(&[]);
    invoke(CURRENT_TASK, OP_ES_GESTO, ES_GESTO_QUITAR, 0, 0).value
}

/// **Trae `origen` de FAT32 y lo guarda en `destino`.**
///
/// ** El contenido NO pasa por aqui. Viajan los dos NOMBRES y el kernel lee la
/// fuente el mismo -- por eso esta es la unica forma de meter en ESTRATOS algo
/// mas grande que los 96 bytes del renglon.
///
/// Y por eso tampoco hay un tope de tamano en esta funcion: el que hay es el
/// del volumen, y lo dice el nivel de ocupacion.
pub fn copiar(destino: &[u8], origen: &[u8]) -> u64 {
    mandar_ruta(destino);
    mandar_datos(origen);
    invoke(CURRENT_TASK, OP_ES_GESTO, ES_GESTO_COPIA, 0, 0).value
}

/// **Le cambia el nombre a `ruta`.** Devuelve la generacion nueva, o `0`.
///
/// `nuevo` es solo el nombre, no una ruta: renombrar no mueve de carpeta.
///
/// * El nodo NO se toca -- la entrada nueva apunta al mismo bloque, asi que el
/// contenido, los atributos y la `:firma` siguen siendo los de antes.
/// **Renombrar un fichero firmado no le invalida la firma**, cosa que hacerlo
/// por el camino largo --quitar y crear-- si haria.
pub fn renombrar(ruta: &[u8], nuevo: &[u8]) -> u64 {
    mandar_ruta(ruta);
    // El nombre nuevo va por el renglon del contenido: es el unico verbo que
    // necesita dos nombres, y ese renglon lleva cuenta explicita de bytes.
    mandar_datos(nuevo);
    invoke(CURRENT_TASK, OP_ES_GESTO, ES_GESTO_RENOMBRAR, 0, 0).value
}
