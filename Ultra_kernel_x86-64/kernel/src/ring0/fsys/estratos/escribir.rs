//! **CREAR UN FICHERO EN ESTRATOS.** La mitad que toca el disco.
//!
//! [eje]     CORRECCION -- lo pide una persona y escribe en el almacen
//! [exige]   la seccion 5 del diseno (el paso que falta para 1.0), L7 (el
//!           formato no se decide aqui)
//!
//! # Que hace, y por que es corto
//!
//! Junta cuatro cosas que ya existian y nunca se habian llamado juntas:
//!
//! ```text
//!   bmo_estratos::escritura   QUE bytes    nodo, entradas, directorio
//!   Transaccion               CUANDO       reservar, barrera, commit
//!   dir + walk                lo de HOY    la raiz actual y sus entradas
//!   disk                      el aparato   write_block y FLUSH CACHE
//! ```
//!
//! Aqui no se decide el formato ni el orden: los dos vienen dados. Lo unico
//! propio es **el reparto de los cuatro bloques**, y esta escrito abajo.
//!
//! # ** POR QUE CUATRO BLOQUES PARA UN FICHERO DE 16 BYTES
//!
//! ```text
//!   base+0   el nodo del FICHERO      con su contenido dentro (residente)
//!   base+1   el bloque de ENTRADAS    las de antes + la nueva
//!   base+2   el nodo del DIRECTORIO   que apunta al bloque de entradas
//!   base+3   el ESTRATO               que apunta al directorio
//! ```
//!
//! Los tres ultimos no son del fichero: son **la version nueva del arbol**. En
//! un sistema que sobreescribe, anadir una entrada toca un bloque; aqui no se
//! toca ninguno, se copian los que cambian. Eso es el copy-on-write, y es lo que
//! hace que el arbol de ayer siga entero y alcanzable -- que es la razon de que
//! este sistema de ficheros exista.
//!
//! [!] Son cuatro y no dos porque **un objeto no comparte bloque**. El
//! formateador del anfitrion si los empaqueta, y hace bien: tiene el volumen
//! entero delante. Aqui gastar 16 KiB donde caben 1,4 es correcto y es simple, y
//! la contabilidad lo dice en voz alta -- el log_head sube de cuatro en cuatro.
//!
//! # El orden, que es el unico que no pierde datos
//!
//! ```text
//!   1  escribir los CUATRO bloques      todavia no los alcanza nadie
//!   2  FLUSH CACHE                      hasta aqui, el volumen es el de antes
//!   3  commit: el superbloque ALTERNO   el punto de no retorno, UN sector
//!   4  FLUSH CACHE otra vez             o el commit se queda en la cache
//! ```
//!
//! ** Si se corta la corriente en 1 o 2, el volumen monta exactamente igual que
//! antes: los bloques nuevos estan ahi y **no hay nada que los alcance**. Si se
//! corta en 3, se estropea la copia del superbloque que NO manda. No hay ninguna
//! ventana en la que se pierda algo.

use bmo_estratos as es;
use bmo_estratos::escritura::{entradas_con, nodo_de_directorio, nodo_de_fichero};
use bmo_estratos::objects::{Attr, BlockPtr, ATTR_ENTRADAS, BLOQUE, NODO_LEN};

use super::{
    copia_en_uso, dir, identidad_ok, superbloque, walk, write_block, write_superblock, WriteError,
};
use crate::ring0::dev::disk;

/// Bloques que cuesta un fichero: el suyo, las entradas, el directorio, el estrato.
const BLOQUES_POR_FICHERO: u64 = 4;

/// Las entradas que YA tiene la raiz. Estatico y no en la pila: son 4 KiB, y la
/// pila del kernel son 64 para todo.
static mut PREVIAS: [u8; BLOQUE] = [0u8; BLOQUE];
/// El bloque de entradas NUEVO. Tiene que ser otro buffer: se lee del de arriba
/// mientras se escribe en este.
static mut ENTRADAS: [u8; BLOQUE] = [0u8; BLOQUE];
/// Un bloque de paso para escribir cada objeto pequeno con su relleno.
static mut BLOQUE_TMP: [u8; BLOQUE] = [0u8; BLOQUE];

/// **Crea `nombre` con `datos` dentro.** Devuelve la generacion nueva.
///
/// El contenido va DENTRO del nodo mientras quepa (96 bytes). Mas grande pide un
/// arbol de bloques con sus niveles, y eso es otra funcion -- se rechaza en vez
/// de partirlo a escondidas, para que el que llama sepa lo que cuesta.
pub fn crear_fichero(nombre: &str, datos: &[u8]) -> Result<u64, WriteError> {
    let sb = superbloque().ok_or(WriteError::SinVolumen)?;

    // -- Lo de HOY, antes de abrir nada. Si esto falla, no se ha tocado nada.
    //
    // ** Un volumen sin estrato --recien formateado-- no es un error: es un
    // arbol vacio, y la entrada nueva sera la primera. Distinguirlo de "no se
    // pudo leer la raiz" importa, porque uno se arregla escribiendo y el otro
    // se agrava escribiendo encima.
    let previas = unsafe { &mut *core::ptr::addr_of_mut!(PREVIAS) };
    let n_previas = match dir::raiz() {
        None => 0,
        Some((_, raiz)) => match raiz.attr(ATTR_ENTRADAS) {
            None => 0,
            Some(a) => leer_entradas(a, previas)?,
        },
    };

    let nodo_f = nodo_de_fichero(datos).map_err(|_| WriteError::NoCabe)?;

    let mut t = es::escritura::Transaccion::open(&sb, copia_en_uso(), identidad_ok())
        .map_err(WriteError::Rechazada)?;
    let base = t
        .reserve(BLOQUES_POR_FICHERO)
        .map_err(WriteError::Rechazada)?;

    // -- Los punteros se calculan ANTES de escribir, y por eso esto es simple:
    // `reserve` ya dijo QUE bloques son, y el hash sale del contenido.
    let p_fichero = BlockPtr::nuevo(base, 0, &nodo_f);

    let entradas = unsafe { &mut *core::ptr::addr_of_mut!(ENTRADAS) };
    let n_ent = entradas_con(&previas[..n_previas], nombre, p_fichero, entradas)
        .map_err(|_| WriteError::NoCabe)?;
    let p_entradas = BlockPtr::nuevo(base + 1, 0, &entradas[..n_ent]);

    let nodo_d = nodo_de_directorio(p_entradas, n_ent as u64).map_err(|_| WriteError::NoCabe)?;
    let p_dir = BlockPtr::nuevo(base + 2, 0, &nodo_d);

    // El estrato nuevo apunta al de antes: recorrer esa cadena hacia atras es
    // recorrer la historia, y por eso el padre es un PUNTERO y no un hash.
    let estrato = es::Estrato::new(
        p_dir,
        sb.estrato,
        0,
        es::Autor::Proceso(crate::ring0::task::scheduler::current_pid()),
        "fichero nuevo",
    );
    let e_bytes = estrato.encode();
    let p_estrato = BlockPtr::nuevo(base + 3, 0, &e_bytes);

    // -- 1. LOS CUATRO BLOQUES. Todavia no los alcanza nadie.
    poner(base, &nodo_f)?;
    poner(base + 1, &entradas[..n_ent])?;
    poner(base + 2, &nodo_d)?;
    poner(base + 3, &e_bytes)?;

    // -- 2. LA BARRERA. Hasta aqui el volumen sigue siendo el de antes.
    if !disk::flush() {
        t.abandonar();
        return Err(WriteError::SinBarrera);
    }
    t.barrera_hecha().map_err(WriteError::Rechazada)?;

    // -- 3. EL COMMIT: un sector, en la copia que no manda.
    let (destino, nuevo) = t.commit(p_estrato).map_err(WriteError::Rechazada)?;
    if !write_superblock(destino, &nuevo.encode()) {
        crate::ring0::cabina::fault("estratos", "no se pudo escribir el superbloque", destino);
        return Err(WriteError::NoEscribio);
    }

    // -- 4. Y VACIAR OTRA VEZ, o el commit se queda en la cache del disco.
    if !disk::flush() {
        crate::ring0::cabina::warn("estratos", "el commit no se pudo vaciar al plato", destino);
        return Err(WriteError::SinBarrera);
    }

    super::fijar_superbloque(nuevo);
    crate::ring0::cabina::info("estratos", "fichero nuevo, generacion", nuevo.generation);
    Ok(nuevo.generation)
}

/// Las entradas de la raiz, en `dst`. `0` si el atributo esta vacio.
fn leer_entradas(a: &Attr, dst: &mut [u8; BLOQUE]) -> Result<usize, WriteError> {
    match walk::flujo(a, dst) {
        Some(n) => Ok(n),
        // No poder leer lo que YA hay es lo peor que puede pasar aqui: escribir
        // sin ello dejaria un directorio con una sola entrada y el resto
        // huerfano. Se para antes de abrir la transaccion.
        None => Err(WriteError::NoSeLeeLaRaiz),
    }
}

/// Escribe un trozo en su bloque, con el relleno a cero.
///
/// ** El relleno va a CERO y no se deja lo que hubiera: el `BlockPtr` solo
/// verifica los `len` bytes del objeto, asi que la basura de detras no rompe
/// nada -- pero un bloque recien reservado con datos viejos dentro es justo lo
/// que no se quiere encontrar el dia que alguien mire el disco a mano.
fn poner(bloque: u64, datos: &[u8]) -> Result<(), WriteError> {
    if datos.len() > BLOQUE {
        return Err(WriteError::NoCabe);
    }
    let buf = unsafe { &mut *core::ptr::addr_of_mut!(BLOQUE_TMP) };
    *buf = [0u8; BLOQUE];
    buf[..datos.len()].copy_from_slice(datos);
    if write_block(bloque, buf) {
        Ok(())
    } else {
        crate::ring0::cabina::fault("estratos", "el disco no acepto un bloque", bloque);
        Err(WriteError::NoEscribio)
    }
}

/// Lo que va a costar, para poder DECIRLO antes de escribir.
pub fn coste() -> u64 {
    BLOQUES_POR_FICHERO
}

/// Cabe el contenido dentro del nodo? Lo pregunta el que propone, antes de nada.
pub fn cabe(datos: &[u8]) -> bool {
    datos.len() <= es::objects::RESIDENTE_MAX
}

const _: () = assert!(NODO_LEN <= BLOQUE, "un nodo tiene que caber en su bloque");
