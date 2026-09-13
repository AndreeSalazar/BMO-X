//! **LO QUE VA DETRAS DE LA RUTA** -- los argumentos de un programa.
//!
//! [carril]  VERDE     unas ranuras de bytes por pid
//! [consumo] NADA      corre cuando alguien lanza o pregunta
//!
//! ## Que problema resuelve (2026-09-13)
//!
//! `run inti/musica.ibx datos/tema.mus` no tenia a donde ir: la linea entera
//! viajaba como RUTA, el kernel intentaba abrir un fichero llamado
//! `musica.ibx datos/tema.mus`, y un programa no tenia forma de saber con que lo
//! habian llamado. Cada app que quisiera abrir "un fichero" tenia que llevar el
//! nombre escrito dentro -- que es como `bico.inti` convierte SIEMPRE
//! `datos/foto.bmp` y ningun otro.
//!
//! ## La forma, y por que es esta
//!
//! ```text
//!    OP_RUTA  x N      "inti/musica.ibx datos/tema.mus"   (lo de siempre)
//!    OP_EJECUTAR       parte en el PRIMER espacio:
//!                         ruta       -> lo que se abre y se carga
//!                         el resto   -> aqui, apuntado al pid del hijo
//!    OP_ARGUMENTOS(i)  el hijo lee su trozo i, 8 bytes; 0 = se acabo
//! ```
//!
//! De ocho en ocho porque la superficie congelada no acepta punteros -- el mismo
//! idioma que `OP_RUTA` y `ES_TEXTO`. Y con una ventaja que no es casualidad:
//! **un trozo de argumento es un trozo de ruta**. Un programa que recibe un nombre
//! de fichero se lo pasa a `OP_RUTA` palabra a palabra, sin copiar nada.
//!
//! ## ** Lo que NO hace, a proposito
//!
//! No trocea en `argv`, no entiende comillas, no expande nada. Es UN texto: el
//! que lo escribio sabe como leerlo, y un analizador de argumentos en Ring 0 es
//! codigo con privilegio que no necesita tenerlo.
//!
//! Un texto que no cabe **no se recorta**: `EJECUTAR` se niega antes de lanzar.
//! Medio nombre de fichero abre otro fichero, que es la misma razon por la que
//! `package::recordar` se rinde con una ruta larga.

use crate::ring0::cabina;

/// Cuantos procesos pueden tener argumentos a la vez. El mismo numero que
/// `family::MAX_VIVOS`, y por el mismo motivo.
const MAX_VIVOS: usize = 16;

/// Lo mas largo que puede ser el texto. Dos rutas de 8.3 con carpeta y sitio.
pub const MAX_BYTES: usize = 128;

static mut PIDS: [u32; MAX_VIVOS] = [0; MAX_VIVOS];
static mut LARGOS: [u8; MAX_VIVOS] = [0; MAX_VIVOS];
static mut TEXTOS: [[u8; MAX_BYTES]; MAX_VIVOS] = [[0; MAX_BYTES]; MAX_VIVOS];

/// Parte la linea de `OP_RUTA` en `(ruta, argumentos)` por el primer espacio.
///
/// Los espacios de los dos extremos no son de nadie y se quitan. Una linea sin
/// espacio es una ruta sola, que es lo que era toda linea hasta hoy.
pub fn partir(linea: &str) -> (&str, &str) {
    let l = linea.trim();
    match l.find(' ') {
        Some(i) => (&l[..i], l[i + 1..].trim()),
        None => (l, ""),
    }
}

/// **Lanza la linea de `OP_RUTA`**: parte, carga la ruta, y apunta el resto al
/// hijo. Lo llama el brazo de `EJECUTAR`.
///
/// Vive aqui y no en `syscall/mod.rs` porque aquel fichero esta por encima de
/// la linea de L6a y solo puede encoger: el brazo se queda en una llamada.
///
/// ** Un texto que no cabe se niega ANTES de lanzar: el motivo va a CABINA y
/// quien llama recibe el fallo del lanzamiento. Crear el proceso y darle medio
/// texto seria abrirle otro fichero.
pub fn lanzar(linea: &str, autoridad: u64) -> crate::ring0::task::launch::Informe {
    use crate::ring0::task::launch::{self, Fallo, Informe};
    let (ruta, texto) = partir(linea);
    if texto.len() > MAX_BYTES {
        cabina::warn("argumentos", "los argumentos pasan de 128 bytes; no se lanza", texto.len() as u64);
        let f = Fallo::NoSeEncuentra("los argumentos pasan de 128 bytes");
        return Informe { origen: "", bytes: 0, firma: None, pid: None, res: Err(f) };
    }
    // Se COPIA antes de lanzar: `linea` apunta al renglon de ruta del kernel,
    // y lo que pase dentro de `launch::ruta` no tiene por que respetarlo.
    let mut copia = [0u8; MAX_BYTES];
    copia[..texto.len()].copy_from_slice(texto.as_bytes());
    let informe = launch::ruta(ruta, autoridad);
    if let (Ok(_), Some(hijo)) = (&informe.res, informe.pid) {
        recordar(hijo, &copia[..texto.len()]);
    }
    informe
}

/// Apunta los argumentos de `pid`. Un texto vacio no ocupa ranura.
pub fn recordar(pid: u32, texto: &[u8]) {
    if pid == 0 || texto.is_empty() || texto.len() > MAX_BYTES {
        return;
    }
    unsafe {
        let libre = (0..MAX_VIVOS).find(|&i| PIDS[i] == 0 || PIDS[i] == pid);
        let Some(i) = libre else {
            cabina::warn("argumentos", "sin ranura para los argumentos de", pid as u64);
            return;
        };
        PIDS[i] = pid;
        LARGOS[i] = texto.len() as u8;
        TEXTOS[i] = [0; MAX_BYTES];
        TEXTOS[i][..texto.len()].copy_from_slice(texto);
    }
}

/// El trozo `i` (8 bytes, little-endian) de los argumentos de `pid`.
///
/// `0` cuando se acaba o cuando no hay: el texto no puede llevar un cero dentro
/// --viene de `OP_RUTA`, que corta en el primero--, asi que un cero es siempre
/// "no hay mas" y nunca un dato.
pub fn trozo(pid: u32, i: u64) -> u64 {
    unsafe {
        for r in 0..MAX_VIVOS {
            if PIDS[r] != pid || pid == 0 {
                continue;
            }
            let largo = LARGOS[r] as u64;
            if i >= (largo + 7) / 8 {
                return 0;
            }
            let desde = (i * 8) as usize;
            let hasta = (desde + 8).min(largo as usize);
            let mut w = [0u8; 8];
            w[..hasta - desde].copy_from_slice(&TEXTOS[r][desde..hasta]);
            return u64::from_le_bytes(w);
        }
        0
    }
}

/// Suelta la ranura. Lo llama `cap::revoke_all`, al lado de `family`.
///
/// Sin esto un pid reutilizado naceria con los argumentos del muerto: abriria
/// el fichero que le pidieron a OTRO programa.
pub fn process_died(pid: u32) {
    unsafe {
        for r in 0..MAX_VIVOS {
            if PIDS[r] == pid {
                PIDS[r] = 0;
                LARGOS[r] = 0;
            }
        }
    }
}
