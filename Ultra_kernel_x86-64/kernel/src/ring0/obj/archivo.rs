//! `KIND_ARCHIVO` — **leer y escribir lo que hay dentro**, como capability.
//!
//! Hermano de [`crate::ring0::obj::directorio`]. Aquel deja PREGUNTAR qué hay en el
//! disco; éste deja abrir uno de esos nombres y mover sus bytes.
//!
//! Hasta ahora el kernel sabía leer archivos (`fs::load`, con el que se carga
//! el compositor) y escribirlos (`fs::create`, con el que CABINA deja su caja
//! negra), y **Ring 3 no tenía con qué pedírselo**. El eslabón que faltaba no
//! era el sistema de ficheros: era la puerta.
//!
//! ## Dos modos, no dos objetos
//!
//! El modo se fija AL ABRIR y no cambia. Un handle de lectura no escribe
//! aunque se le pida — y no por una comprobación de permisos que alguien deba
//! acordarse de escribir, sino porque en ese modo **no hay a dónde escribir**.
//! Es la misma idea que hace inmutable el volumen de arranque: no existe la
//! función.
//!
//! ## Por qué hay un buffer, y qué límite impone
//!
//! La superficie congelada no acepta punteros: los bytes cruzan de 7 en 7
//! dentro de un registro. Y `bmo_fat32` escribe un archivo ENTERO de una vez,
//! no por trozos. Entre esas dos cosas hace falta un sitio donde juntar lo que
//! llega suelto, y ese sitio es un buffer del kernel.
//!
//! Consecuencia, dicha en alto porque es una limitación real y no un detalle:
//! **un archivo abierto desde Ring 3 no puede pasar de [`BUFFER`] bytes.** Al
//! leer se rechaza con `ERROR_DEMASIADO_GRANDE` en vez de entregar medio
//! archivo —medio fichero de movimientos es peor que ninguno—, y al escribir
//! se descarta lo que no cabe y `cerrar` lo dice.
//!
//! Lo que quitaría ese límite es un escritor por sectores en `bmo_fat32`, que
//! es otra pieza y va después. Prometer aquí que no hay límite sería mentir en
//! el sitio donde más caro sale.
//!
//! ## Escribir es un acto de dos pasos
//!
//! Lo escrito NO está en el disco hasta `ARCH_OP_CERRAR`. Un proceso que muere
//! a medias no deja un archivo a medias: no deja nada. Para un fichero de
//! movimientos eso es lo correcto — un extracto truncado se parece demasiado a
//! uno completo.

use crate::ring0::obj::cap;

/// Cuántos archivos pueden estar abiertos a la vez, en todo el sistema.
pub const MAX_ABIERTOS: usize = 4;

/// Lo más grande que Ring 3 puede leer o escribir de una vez. Ver la nota de
/// cabecera: es el buffer, no un capricho.
pub const BUFFER: usize = 4096;

pub const SIN_DUENO: u32 = u32::MAX;

/// No quedan ranuras de archivo abierto.
pub const ERROR_SIN_HUECO: u32 = 27;
/// La ruta no existe, o no es un archivo.
pub const ERROR_NO_ESTA: u32 = 28;
/// El archivo no cabe en el buffer. Se dice en vez de entregar un trozo.
pub const ERROR_DEMASIADO_GRANDE: u32 = 29;
/// El nombre no cabe en 8.3 (ocho de nombre, tres de extensión).
pub const ERROR_NOMBRE: u32 = 30;
/// No hay volumen de datos montado con escritor.
pub const ERROR_SOLO_LECTURA: u32 = 31;
/// La CARPETA de la ruta no existe. Distinto de que falte el archivo: manda a
/// mirar otra cosa, y un mensaje que no los separa manda a buscar donde no es.
pub const ERROR_CARPETA: u32 = 32;
/// La ruta no nombra un archivo — acaba en barra, o es un directorio.
pub const ERROR_ES_CARPETA: u32 = 33;

/// Saca hasta 7 bytes: `(n << 56) | bytes_LE`. `n == 0` = se acabó.
///
/// Siete y no ocho porque el octavo lleva la cuenta. Es el mismo trato que la
/// consola y que `DIR_OP_NOMBRE`: un contador honesto vale más que un byte
/// apretado, y aquí además hace que **el NUL viaje** — un archivo no es texto
/// y cortar en el primer cero corrompería cualquier binario.
pub const ARCH_OP_LEER: u64 = 0x01;

/// Mete hasta 7 bytes: `arg0 = (n << 56) | bytes_LE`, el mismo formato que
/// `LEER` pero al revés. Devuelve cuántos se aceptaron.
pub const ARCH_OP_ESCRIBIR: u64 = 0x02;

/// Bytes del archivo (los que quedan por leer, o los escritos hasta ahora).
pub const ARCH_OP_TAMANO: u64 = 0x03;

/// Cierra. En un archivo de ESCRITURA es donde el contenido llega al disco:
/// devuelve `1` si se guardó, `0` si no. En uno de lectura devuelve `1`.
pub const ARCH_OP_CERRAR: u64 = 0x04;

static mut BUF: [[u8; BUFFER]; MAX_ABIERTOS] = [[0; BUFFER]; MAX_ABIERTOS];
/// Bytes válidos: lo leído del disco, o lo acumulado para escribir.
static mut LARGO: [usize; MAX_ABIERTOS] = [0; MAX_ABIERTOS];
/// Por dónde va la lectura.
static mut CURSOR: [usize; MAX_ABIERTOS] = [0; MAX_ABIERTOS];
/// Nombre 8.3 y directorio destino, para el momento de guardar.
static mut NOMBRE: [[u8; 11]; MAX_ABIERTOS] = [[b' '; 11]; MAX_ABIERTOS];
static mut DIRECTORIO: [u32; MAX_ABIERTOS] = [0; MAX_ABIERTOS];
static mut ESCRIBE: [bool; MAX_ABIERTOS] = [false; MAX_ABIERTOS];
/// Se pidió escribir más de lo que cabe. `cerrar` lo confiesa en vez de
/// guardar un archivo corto que parece entero.
static mut DESBORDO: [bool; MAX_ABIERTOS] = [false; MAX_ABIERTOS];
static mut DUENO: [u32; MAX_ABIERTOS] = [SIN_DUENO; MAX_ABIERTOS];

fn hueco() -> Option<usize> {
    unsafe { (0..MAX_ABIERTOS).find(|&i| DUENO[i] == SIN_DUENO) }
}

/// Abre un archivo del volumen de datos para LEER y entrega su handle a `pid`.
///
/// El archivo entero se trae al buffer aquí, no según se pide. Así una lectura
/// no puede fallar a mitad por un error de disco: o el archivo está entero en
/// memoria antes de que Ring 3 vea el primer byte, o `abrir` falla y no hay
/// handle.
pub fn abrir(pid: u32, ruta: &str) -> Result<u64, u32> {
    let i = match hueco() {
        Some(i) => i,
        None => return Err(ERROR_SIN_HUECO),
    };
    let leidos = unsafe {
        let dst = &mut (*core::ptr::addr_of_mut!(BUF))[i];
        // Cada motivo manda a hacer algo distinto, y por eso no se aplanan
        // todos a "no esta": quien escribe `lee apps/` tiene que enterarse de
        // que eso es una carpeta, no ponerse a buscar un archivo que nunca
        // existio.
        use crate::ring0::fsys::fs::LoadError;
        match crate::ring0::fsys::fs::load(ruta, dst) {
            Ok(n) => n,
            Err(LoadError::TooBig) => return Err(ERROR_DEMASIADO_GRANDE),
            Err(LoadError::BadPath) => return Err(ERROR_ES_CARPETA),
            Err(LoadError::NameTooLong) => return Err(ERROR_NOMBRE),
            Err(LoadError::DirNotFound) => return Err(ERROR_CARPETA),
            Err(_) => return Err(ERROR_NO_ESTA),
        }
    };
    unsafe {
        LARGO[i] = leidos;
        CURSOR[i] = 0;
        ESCRIBE[i] = false;
        DESBORDO[i] = false;
        DUENO[i] = pid;
        match cap::grant(pid, cap::KIND_ARCHIVO, cap::RIGHT_READ, i as u64) {
            Some(h) => {
                crate::ring0::cabina::info("arch", "archivo abierto para leer", leidos as u64);
                Ok(h)
            }
            None => {
                DUENO[i] = SIN_DUENO;
                Err(cap::ERROR_PERMISSION_DENIED)
            }
        }
    }
}

/// Abre un archivo del volumen de datos para ESCRIBIR.
///
/// El directorio se resuelve AHORA y el nombre se valida AHORA, aunque no se
/// escriba nada hasta cerrar. Descubrir al final que la carpeta no existía
/// significaría haber dejado a un programa acumulando bytes para nada.
pub fn crear(pid: u32, ruta: &str) -> Result<u64, u32> {
    if !crate::ring0::fsys::fs::data_mounted() {
        return Err(ERROR_SOLO_LECTURA);
    }
    // Partir la ruta en carpeta + nombre por la ÚLTIMA barra.
    let limpia = {
        let mut p = ruta.trim();
        if p.len() >= 2 && p.as_bytes()[1] == b':' { p = &p[2..]; }
        while p.starts_with('/') || p.starts_with('\\') { p = &p[1..]; }
        p
    };
    let corte = limpia.rfind(['/', '\\']);
    let (carpeta, nombre_txt) = match corte {
        Some(k) => (&limpia[..k], &limpia[k + 1..]),
        None => ("", limpia),
    };
    if nombre_txt.is_empty() {
        return Err(ERROR_NOMBRE);
    }
    let nombre = match crate::ring0::fsys::fs::nombre_8_3_pub(nombre_txt) {
        Some(n) => n,
        None => return Err(ERROR_NOMBRE),
    };
    let dir = match crate::ring0::fsys::fs::dir_datos(carpeta) {
        Some(c) => c,
        // La carpeta, no el archivo. `escribe datos/x.txt` cuando no hay
        // `datos/` tiene que decir que falta la CARPETA: el archivo es
        // justamente lo que se venia a crear.
        None => return Err(ERROR_CARPETA),
    };

    let i = match hueco() {
        Some(i) => i,
        None => return Err(ERROR_SIN_HUECO),
    };
    unsafe {
        LARGO[i] = 0;
        CURSOR[i] = 0;
        NOMBRE[i] = nombre;
        DIRECTORIO[i] = dir;
        ESCRIBE[i] = true;
        DESBORDO[i] = false;
        DUENO[i] = pid;
        // Se conceden los dos derechos: `invoke` resuelve con RIGHT_READ, así
        // que sin él ni siquiera llegaría el `ESCRIBIR`. Lo que impide leer un
        // archivo de escritura no es el derecho, es el modo — ver `operacion`.
        match cap::grant(pid, cap::KIND_ARCHIVO, cap::RIGHT_READ | cap::RIGHT_WRITE, i as u64) {
            Some(h) => {
                crate::ring0::cabina::info("arch", "archivo abierto para escribir", pid as u64);
                Ok(h)
            }
            None => {
                DUENO[i] = SIN_DUENO;
                Err(cap::ERROR_PERMISSION_DENIED)
            }
        }
    }
}

fn leer(i: usize) -> u64 {
    unsafe {
        let mut w = [0u8; 8];
        let mut n = 0usize;
        while n < 7 && CURSOR[i] < LARGO[i] {
            w[n] = BUF[i][CURSOR[i]];
            CURSOR[i] += 1;
            n += 1;
        }
        ((n as u64) << 56) | u64::from_le_bytes(w)
    }
}

fn escribir(i: usize, palabra: u64) -> u64 {
    let n = ((palabra >> 56) & 0xFF) as usize;
    let n = n.min(7);
    let bytes = palabra.to_le_bytes();
    unsafe {
        let mut puestos = 0usize;
        for k in 0..n {
            if LARGO[i] >= BUFFER {
                DESBORDO[i] = true;
                break;
            }
            BUF[i][LARGO[i]] = bytes[k];
            LARGO[i] += 1;
            puestos += 1;
        }
        puestos as u64
    }
}

/// Cierra la ranura y, si era de escritura, guarda. `1` = todo bien.
fn cerrar(i: usize) -> u64 {
    unsafe {
        let ok = if ESCRIBE[i] {
            if DESBORDO[i] {
                // No se guarda NADA. Un archivo recortado en silencio se
                // parece demasiado a uno entero, y el que lo lea manana no
                // tiene forma de saberlo.
                crate::ring0::cabina::warn("arch", "no cabia: no se guarda nada", LARGO[i] as u64);
                false
            } else {
                // El slice se construye desde el puntero crudo, sin autoref
                // implicito sobre el `static mut`: la fila primero, el recorte
                // despues. Encadenarlo en una expresion crea una referencia a
                // la desreferencia del puntero, que es justo lo que el lint
                // prohibe — y con razon, porque esconde de donde sale.
                let fila = &(*core::ptr::addr_of!(BUF))[i];
                let datos = &fila[..LARGO[i]];
                match crate::ring0::fsys::fs::crear_en(DIRECTORIO[i], &NOMBRE[i], datos) {
                    Ok(()) => {
                        crate::ring0::cabina::info("arch", "archivo guardado", LARGO[i] as u64);
                        true
                    }
                    Err(_) => {
                        crate::ring0::cabina::warn("arch", "no se pudo guardar", LARGO[i] as u64);
                        false
                    }
                }
            }
        } else {
            true
        };
        soltar(i);
        ok as u64
    }
}

fn soltar(i: usize) {
    unsafe {
        DUENO[i] = SIN_DUENO;
        LARGO[i] = 0;
        CURSOR[i] = 0;
        ESCRIBE[i] = false;
        DESBORDO[i] = false;
        DIRECTORIO[i] = 0;
    }
}

pub fn operacion(idx: u64, op: u64, arg0: u64) -> Option<u64> {
    let i = idx as usize;
    if i >= MAX_ABIERTOS {
        return None;
    }
    let escribe = unsafe { ESCRIBE[i] };
    match op {
        // El modo manda. Pedirle bytes a un archivo de escritura no es un
        // error de permisos: es una pregunta que ese objeto no responde.
        ARCH_OP_LEER if !escribe => Some(leer(i)),
        ARCH_OP_ESCRIBIR if escribe => Some(escribir(i, arg0)),
        ARCH_OP_TAMANO => Some(unsafe {
            if escribe { LARGO[i] as u64 } else { (LARGO[i] - CURSOR[i]) as u64 }
        }),
        ARCH_OP_CERRAR => Some(cerrar(i)),
        _ => None,
    }
}

/// Lo llama `cap::revoke_all`. Un proceso que muere con un archivo de
/// escritura a medias **no deja nada**: lo acumulado se tira. Guardarlo seria
/// inventar un archivo que su autor nunca dio por terminado.
pub fn proceso_muerto(pid: u32) {
    unsafe {
        for i in 0..MAX_ABIERTOS {
            if DUENO[i] == pid {
                if ESCRIBE[i] && LARGO[i] > 0 {
                    crate::ring0::cabina::warn("arch", "murio sin cerrar: se descarta", LARGO[i] as u64);
                }
                soltar(i);
            }
        }
    }
}
