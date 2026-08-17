//! `KIND_DIRECTORIO` -- **preguntar que hay**, como capability.
//!
//! Hasta ahora Ring 3 podia LANZAR un programa pero no MIRAR el disco: habia
//! que saberse la ruta de memoria y teclearla entera. Sin esto no hay `ls`, no
//! hay autocompletado y no hay iconos de carpeta -- no por falta de dibujo,
//! sino porque no existia la pregunta.
//!
//! ## Esto NO es como abrir un archivo por su nombre
//!
//! Y ahi esta la diferencia con un sistema de ficheros clasico, que merece
//! estar escrita porque es el modelo entero:
//!
//! - En Unix, **una ruta es un NAME**. Cualquiera puede escribir `/etc/passwd`
//!   y el kernel decide despues si le deja. El nombre siempre es nombrable.
//! - Aqui una ruta abierta es un **HANDLE que a alguien le concedieron**. Lo
//!   que no te han dado no existe para tu proceso: no es que te lo nieguen, es
//!   que no tienes con que preguntar.
//!
//! Por eso `open` es una operacion sobre `CURRENT_TASK` --lo que uno pide por
//! ser quien es-- y el listado es una operacion sobre el handle resultante. El
//! dia que haya varios usuarios, quien puede abrir que se decide en `open` y
//! el resto del sistema no se entera.
//!
//! ## Sin cursor en el driver
//!
//! El handle guarda `(cluster, indice)` y el driver contesta "dame la entrada
//! n". Es O(n) por llamada, o sea O(n^2) por listado -- irrelevante con
//! directorios de decenas de entries, y a cambio **dos listados a la vez no se
//! pisan** y una entrada que desaparece no deja un cursor apuntando al vacio.
//!
//! ## Los nombres salen en 8.3 crudo
//!
//! `COBOL   BEX` con sus espacios, tal cual esta en el disco. Convertirlo a
//! `COBOL.BEX` es presentacion, y la presentacion es de Ring 3 -- la misma linea
//! que deja el cursor del raton fuera del kernel.

use crate::ring0::obj::cap;

/// Cuantos directorios pueden estar abiertos a la vez.
pub const MAX_ABIERTOS: usize = 8;

pub const NO_OWNER: u32 = u32::MAX;

/// No quedan ranuras de directorio abierto.
pub const ERROR_NO_FREE_SLOT: u32 = 25;
/// La ruta no existe, o no es un directorio.
pub const ERROR_NOT_THERE: u32 = 26;

/// Avanza a la siguiente entrada y devuelve lo que se sabe de ella:
/// `(hay << 63) | (es_dir << 62) | tamano`. `hay == 0` = se acabo el
/// directorio.
///
/// El NAME no viaja aqui: son 11 bytes y no caben con los demas campos.
/// Se pide aparte con `DIR_OP_NOMBRE`, que es la misma decision que ya se tomo
/// en la consola -- un contador honesto vale mas que un byte apretado.
pub const DIR_OP_SIGUIENTE: u64 = 0x01;
/// Los 11 bytes del nombre 8.3 de la entrada ACTUAL, de 7 en 7.
/// `arg0` = desplazamiento (0 o 7). Devuelve `(n << 56) | bytes_LE`.
pub const DIR_OP_NOMBRE: u64 = 0x02;
/// **Cerrar. Devuelve la ranura.**
///
/// === Por que faltaba, y lo que costo ===
///
/// No existia, asi que la UNICA forma de liberar una ranura era
/// [`process_died`] -- o sea, que el proceso se muriera. Y el cliente de esto
/// es **el compositor, que no muere nunca**: es el escritorio.
///
/// Resultado: cada `ls` se quedaba una ranura para siempre, y al noveno la
/// tabla estaba llena. A partir de ahi `ls` contestaba **"no puedo abrir esa
/// carpeta"** -- un mensaje falso, porque la carpeta estaba perfectamente ahi.
/// Lo que no habia era sitio para abrirla.
///
/// `KIND_ARCHIVO` si tenia su `ARCH_OP_CERRAR` desde el principio. Esta es la
/// misma clase de recurso y se quedo sin el; la asimetria no se ve leyendo
/// ninguno de los dos archivos por separado.
///
/// Es el **patron 17** de nuevo --una tabla de recursos vivos que solo se libera
/// con un evento que no ocurre-- y ya se pago hoy en `KIND_MEMORIA`, donde el
/// contador se indexaba por un pid que solo sube. La pregunta que lo caza en
/// los dos casos es la misma: **quien devuelve esto, y ocurre alguna vez?**
pub const DIR_OP_CERRAR: u64 = 0x03;

static mut CLUSTER: [u32; MAX_ABIERTOS] = [0; MAX_ABIERTOS];
static mut INDICE: [usize; MAX_ABIERTOS] = [0; MAX_ABIERTOS];
static mut NAME: [[u8; 11]; MAX_ABIERTOS] = [[b' '; 11]; MAX_ABIERTOS];
static mut OWNER: [u32; MAX_ABIERTOS] = [NO_OWNER; MAX_ABIERTOS];

/// Abre un directorio del volumen de datos y entrega su handle a `pid`.
/// Ruta vacia = la raiz.
pub fn open(pid: u32, ruta: &str) -> Result<u64, u32> {
    let cluster = match crate::ring0::fsys::fs::dir_datos(ruta) {
        Some(c) => c,
        None => return Err(ERROR_NOT_THERE),
    };
    unsafe {
        let libre = (0..MAX_ABIERTOS).find(|&i| OWNER[i] == NO_OWNER);
        let i = match libre {
            Some(i) => i,
            None => return Err(ERROR_NO_FREE_SLOT),
        };
        CLUSTER[i] = cluster;
        // * Empieza en usize::MAX para que el PRIMER `SIGUIENTE` caiga en la
        // entrada 0. Si empezara en 0, la primera llamada devolveria la
        // segunda entrada y la primera no la veria nadie -- el clasico error
        // de un cursor que ya apunta a algo antes de que le pidan avanzar.
        INDICE[i] = usize::MAX;
        NAME[i] = [b' '; 11];
        OWNER[i] = pid;
        match cap::grant(pid, cap::KIND_DIRECTORIO, cap::RIGHT_READ, i as u64) {
            Some(h) => {
                crate::ring0::cabina::info("dir", "directorio abierto para Ring 3", pid as u64);
                Ok(h)
            }
            None => {
                OWNER[i] = NO_OWNER;
                Err(cap::ERROR_PERMISSION_DENIED)
            }
        }
    }
}

fn next(i: usize) -> u64 {
    unsafe {
        let n = INDICE[i].wrapping_add(1);
        match crate::ring0::fsys::fs::entrada_datos(CLUSTER[i], n) {
            Some((name, es_dir, tam)) => {
                INDICE[i] = n;
                NAME[i] = name;
                (1u64 << 63) | ((es_dir as u64) << 62) | tam as u64
            }
            None => 0,
        }
    }
}

fn name(i: usize, desde: usize) -> u64 {
    unsafe {
        let n = &NAME[i];
        let mut w = [0u8; 8];
        let mut k = 0usize;
        while k < 7 && desde + k < n.len() {
            w[k] = n[desde + k];
            k += 1;
        }
        ((k as u64) << 56) | u64::from_le_bytes(w)
    }
}

pub fn operation(idx: u64, op: u64, arg0: u64) -> Option<u64> {
    let i = idx as usize;
    if i >= MAX_ABIERTOS {
        return None;
    }
    match op {
        DIR_OP_SIGUIENTE => Some(next(i)),
        DIR_OP_NOMBRE => Some(name(i, arg0 as usize)),
        DIR_OP_CERRAR => {
            unsafe {
                OWNER[i] = NO_OWNER;
                CLUSTER[i] = 0;
                INDICE[i] = usize::MAX;
            }
            Some(1)
        }
        _ => None,
    }
}

/// Lo llama `cap::revoke_all`: los directorios que tuviera abiertos se cierran.
/// Cuantas ranuras siguen siendo de `pid`. **Despues de `process_died` tiene
/// que ser CERO**, y quien lo comprueba es la autopsia: el escalon 1 de
/// `docs/plan/PLAN_AUTOCURACION.md`.
///
/// Existe porque `process_died` hace su trabajo y **nadie miraba si funciono**.
/// Una fuga de ranuras no da error: da un sistema que un dia no puede abrir un
/// directorio mas, sin nada que lo relacione con el proceso que murio hace una
/// hora.
pub fn pending_of(pid: u32) -> u32 {
    let mut n = 0;
    unsafe {
        for i in 0..MAX_ABIERTOS {
            if OWNER[i] == pid {
                n += 1;
            }
        }
    }
    n
}

pub fn process_died(pid: u32) {
    unsafe {
        for i in 0..MAX_ABIERTOS {
            if OWNER[i] == pid {
                OWNER[i] = NO_OWNER;
                CLUSTER[i] = 0;
                INDICE[i] = usize::MAX;
            }
        }
    }
}
