//! `KIND_DIRECTORIO` — **preguntar qué hay**, como capability.
//!
//! Hasta ahora Ring 3 podía LANZAR un programa pero no MIRAR el disco: había
//! que saberse la ruta de memoria y teclearla entera. Sin esto no hay `ls`, no
//! hay autocompletado y no hay iconos de carpeta — no por falta de dibujo,
//! sino porque no existía la pregunta.
//!
//! ## Esto NO es como abrir un archivo por su nombre
//!
//! Y ahí está la diferencia con un sistema de ficheros clásico, que merece
//! estar escrita porque es el modelo entero:
//!
//! - En Unix, **una ruta es un NOMBRE**. Cualquiera puede escribir `/etc/passwd`
//!   y el kernel decide después si le deja. El nombre siempre es nombrable.
//! - Aquí una ruta abierta es un **HANDLE que a alguien le concedieron**. Lo
//!   que no te han dado no existe para tu proceso: no es que te lo nieguen, es
//!   que no tienes con qué preguntar.
//!
//! Por eso `abrir` es una operación sobre `CURRENT_TASK` —lo que uno pide por
//! ser quien es— y el listado es una operación sobre el handle resultante. El
//! día que haya varios usuarios, quién puede abrir qué se decide en `abrir` y
//! el resto del sistema no se entera.
//!
//! ## Sin cursor en el driver
//!
//! El handle guarda `(cluster, índice)` y el driver contesta "dame la entrada
//! n". Es O(n) por llamada, o sea O(n²) por listado — irrelevante con
//! directorios de decenas de entradas, y a cambio **dos listados a la vez no se
//! pisan** y una entrada que desaparece no deja un cursor apuntando al vacío.
//!
//! ## Los nombres salen en 8.3 crudo
//!
//! `COBOL   BEX` con sus espacios, tal cual está en el disco. Convertirlo a
//! `COBOL.BEX` es presentación, y la presentación es de Ring 3 — la misma línea
//! que deja el cursor del ratón fuera del kernel.

use crate::ring0::obj::cap;

/// Cuántos directorios pueden estar abiertos a la vez.
pub const MAX_ABIERTOS: usize = 8;

pub const SIN_DUENO: u32 = u32::MAX;

/// No quedan ranuras de directorio abierto.
pub const ERROR_SIN_HUECO: u32 = 25;
/// La ruta no existe, o no es un directorio.
pub const ERROR_NO_ESTA: u32 = 26;

/// Avanza a la siguiente entrada y devuelve lo que se sabe de ella:
/// `(hay << 63) | (es_dir << 62) | tamaño`. `hay == 0` = se acabó el
/// directorio.
///
/// El NOMBRE no viaja aquí: son 11 bytes y no caben con los demás campos.
/// Se pide aparte con `DIR_OP_NOMBRE`, que es la misma decisión que ya se tomó
/// en la consola — un contador honesto vale más que un byte apretado.
pub const DIR_OP_SIGUIENTE: u64 = 0x01;
/// Los 11 bytes del nombre 8.3 de la entrada ACTUAL, de 7 en 7.
/// `arg0` = desplazamiento (0 o 7). Devuelve `(n << 56) | bytes_LE`.
pub const DIR_OP_NOMBRE: u64 = 0x02;

static mut CLUSTER: [u32; MAX_ABIERTOS] = [0; MAX_ABIERTOS];
static mut INDICE: [usize; MAX_ABIERTOS] = [0; MAX_ABIERTOS];
static mut NOMBRE: [[u8; 11]; MAX_ABIERTOS] = [[b' '; 11]; MAX_ABIERTOS];
static mut DUENO: [u32; MAX_ABIERTOS] = [SIN_DUENO; MAX_ABIERTOS];

/// Abre un directorio del volumen de datos y entrega su handle a `pid`.
/// Ruta vacía = la raíz.
pub fn abrir(pid: u32, ruta: &str) -> Result<u64, u32> {
    let cluster = match crate::ring0::fsys::fs::dir_datos(ruta) {
        Some(c) => c,
        None => return Err(ERROR_NO_ESTA),
    };
    unsafe {
        let libre = (0..MAX_ABIERTOS).find(|&i| DUENO[i] == SIN_DUENO);
        let i = match libre {
            Some(i) => i,
            None => return Err(ERROR_SIN_HUECO),
        };
        CLUSTER[i] = cluster;
        // ★ Empieza en usize::MAX para que el PRIMER `SIGUIENTE` caiga en la
        // entrada 0. Si empezara en 0, la primera llamada devolvería la
        // segunda entrada y la primera no la vería nadie — el clásico error
        // de un cursor que ya apunta a algo antes de que le pidan avanzar.
        INDICE[i] = usize::MAX;
        NOMBRE[i] = [b' '; 11];
        DUENO[i] = pid;
        match cap::grant(pid, cap::KIND_DIRECTORIO, cap::RIGHT_READ, i as u64) {
            Some(h) => {
                crate::ring0::cabina::info("dir", "directorio abierto para Ring 3", pid as u64);
                Ok(h)
            }
            None => {
                DUENO[i] = SIN_DUENO;
                Err(cap::ERROR_PERMISSION_DENIED)
            }
        }
    }
}

fn siguiente(i: usize) -> u64 {
    unsafe {
        let n = INDICE[i].wrapping_add(1);
        match crate::ring0::fsys::fs::entrada_datos(CLUSTER[i], n) {
            Some((nombre, es_dir, tam)) => {
                INDICE[i] = n;
                NOMBRE[i] = nombre;
                (1u64 << 63) | ((es_dir as u64) << 62) | tam as u64
            }
            None => 0,
        }
    }
}

fn nombre(i: usize, desde: usize) -> u64 {
    unsafe {
        let n = &NOMBRE[i];
        let mut w = [0u8; 8];
        let mut k = 0usize;
        while k < 7 && desde + k < n.len() {
            w[k] = n[desde + k];
            k += 1;
        }
        ((k as u64) << 56) | u64::from_le_bytes(w)
    }
}

pub fn operacion(idx: u64, op: u64, arg0: u64) -> Option<u64> {
    let i = idx as usize;
    if i >= MAX_ABIERTOS {
        return None;
    }
    match op {
        DIR_OP_SIGUIENTE => Some(siguiente(i)),
        DIR_OP_NOMBRE => Some(nombre(i, arg0 as usize)),
        _ => None,
    }
}

/// Lo llama `cap::revoke_all`: los directorios que tuviera abiertos se cierran.
pub fn proceso_muerto(pid: u32) {
    unsafe {
        for i in 0..MAX_ABIERTOS {
            if DUENO[i] == pid {
                DUENO[i] = SIN_DUENO;
                CLUSTER[i] = 0;
                INDICE[i] = usize::MAX;
            }
        }
    }
}
