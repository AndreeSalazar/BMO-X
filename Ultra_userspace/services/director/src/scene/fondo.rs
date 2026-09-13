//! **EL FONDO CON IMAGEN** -- `fondo_imagen = sys/fondo.qoi` (2026-09-13).
//!
//! [consumo] NADA      lee y descifra UNA vez, al arrancar; despues contestar
//!                     un color son dos tablas y un acceso (L6h)
//!
//! Eddi eligio la imagen 4 como escritorio, y lo primero que la separa del de
//! BMO-X es que el fondo es una FOTO y no un degradado. `bmo-imagen` ya descifra
//! BICO, BMP y QOI para el visor; esto le da un segundo lector.
//!
//! ## *** Dos preguntan, y los dos por el MISMO camino
//!
//! Pintar el fondo y RESTAURARLO --al mover el cursor, al cerrar una ventana--
//! preguntan `scene::background_at`. Si esta pieza solo supiera pintar, cada
//! ventana cerrada dejaria un hueco con el degradado viejo dentro de la foto:
//! es la misma leccion que el redondeo de `inside_rounded`.
//!
//! ## El encuadre: CUBRIR, como cualquier escritorio
//!
//! La imagen se escala hasta tapar la pantalla entera y se recorta lo que
//! sobra, centrado. Estirar deformaria; encajar dejaria franjas. El escalado es
//! el vecino mas cercano y se calcula UNA vez por columna y por fila:
//!
//! ```text
//!    COL[x] = columna de la imagen que cae en la x de la pantalla
//!    FIL[y] = fila    de la imagen que cae en la y de la pantalla
//!    color(x, y) = PIXELES[FIL[y] * ancho + COL[x]]      sin dividir
//! ```
//!
//! Sin fichero, o con uno roto, se queda el degradado y el motivo sale en la
//! consola de Ejecutar: un fondo que no carga no es razon para no arrancar.

use bmo_userland as bmo;
use core::ptr::{addr_of, addr_of_mut};

use super::estilo;

/// Lo mas grande que se lee como fondo: igual que el visor.
const TOPE: u64 = 4 * 1024 * 1024;
/// La pantalla mas ancha o alta que se tabula. 4K entra.
const LADO_PANTALLA: usize = 4096;

static mut BUFER: Option<bmo::Memoria> = None;
static mut ANCHO: u32 = 0;
static mut LISTO: bool = false;
static mut MOTIVO: Option<&'static str> = None;
static mut COL: [u16; LADO_PANTALLA] = [0; LADO_PANTALLA];
static mut FIL: [u16; LADO_PANTALLA] = [0; LADO_PANTALLA];

/// Por que no hay foto de fondo, si se pidio una y no salio.
pub(crate) fn motivo() -> Option<&'static str> {
    unsafe { MOTIVO }
}

fn fallar(m: &'static str) {
    unsafe {
        MOTIVO = Some(m);
        LISTO = false;
    }
}

/// **Lee `fondo_imagen` del estilo y la prepara para la pantalla `p`.**
pub(crate) fn cargar(p: &bmo::Pantalla) {
    unsafe {
        LISTO = false;
        MOTIVO = None;
    }
    let ruta = estilo::estilo().fondo_imagen;
    if ruta.vacia() {
        return;
    }
    if p.ancho as usize > LADO_PANTALLA || p.alto as usize > LADO_PANTALLA {
        return fallar("fondo_imagen: la pantalla es mas grande de lo que se tabula");
    }
    let Ok(a) = bmo::Archivo::leer_de(ruta.bytes()) else {
        return fallar("fondo_imagen: no encuentro ese fichero");
    };
    let mide = a.tamano();
    if mide > TOPE {
        return fallar("fondo_imagen: pasa de 4 MiB");
    }
    // ** Se pide lo que MIDE, no el tope: el fichero se suelta al acabar y los
    // pixeles son ancho x alto. Un atardecer de 480x270 son 518 KiB, no 8 MiB.
    let Some(fichero) = bmo::Memoria::request(mide.max(1)) else {
        return fallar("fondo_imagen: sin memoria para leer el fichero");
    };
    let n = a.leer_en(&fichero, 0, mide) as usize;
    // SAFETY: `n` bytes que el kernel acaba de escribir en un bloque de este proceso.
    let bytes = unsafe { core::slice::from_raw_parts(fichero.base() as *const u8, n) };
    let medidas = match bmo_imagen::medir(bytes) {
        Ok(m) => m,
        Err(e) => return fallar(e.motivo()),
    };
    let lleva = (medidas.ancho as u64 * medidas.alto as u64 * 4).max(4);
    unsafe { *addr_of_mut!(BUFER) = bmo::Memoria::request(lleva) };
    let Some(bufer) = (unsafe { (*addr_of!(BUFER)).as_ref() }) else {
        return fallar("fondo_imagen: sin memoria para los pixeles");
    };
    // SAFETY: el bloque mide `lleva` bytes y su base es de pagina (alineada a 4).
    let px = unsafe { core::slice::from_raw_parts_mut(bufer.base() as *mut u32, (lleva / 4) as usize) };
    let m = match bmo_imagen::decodificar(bytes, px) {
        Ok(m) => m,
        Err(e) => return fallar(e.motivo()),
    };
    // CUBRIR: la escala es la MENOR de las dos proporciones imagen/pantalla
    // (en milesimas de pixel de imagen por pixel de pantalla), y lo que sobra
    // se reparte a los dos lados.
    let (w, h) = (m.ancho as u64, m.alto as u64);
    let (pw, ph) = (p.ancho as u64, p.alto as u64);
    let s = (w * 1024 / pw).min(h * 1024 / ph).max(1);
    let ox = (w * 1024).saturating_sub(pw * s) / 2;
    let oy = (h * 1024).saturating_sub(ph * s) / 2;
    unsafe {
        let col = &mut *addr_of_mut!(COL);
        for x in 0..pw {
            col[x as usize] = ((ox + x * s) / 1024).min(w - 1) as u16;
        }
        let fil = &mut *addr_of_mut!(FIL);
        for y in 0..ph {
            fil[y as usize] = ((oy + y * s) / 1024).min(h - 1) as u16;
        }
        ANCHO = m.ancho;
        LISTO = true;
    }
}

/// **El color de la foto en `(x, y)`**, o `None` si no hay foto.
#[inline]
pub(crate) fn color(x: u32, y: u32) -> Option<u32> {
    if !unsafe { LISTO } || x as usize >= LADO_PANTALLA || y as usize >= LADO_PANTALLA {
        return None;
    }
    let bufer = unsafe { (*addr_of!(BUFER)).as_ref()? };
    let i = unsafe { (*addr_of!(FIL))[y as usize] as usize * ANCHO as usize + (*addr_of!(COL))[x as usize] as usize };
    // SAFETY: FIL < alto y COL < ancho de la imagen descifrada, que cabe en PIXELES.
    Some(unsafe { *(bufer.base() as *const u32).add(i) } & 0x00FF_FFFF)
}

/// **Pinta la foto entera.** `false` si no hay foto: entonces pinta el degradado
/// quien llama.
pub(crate) fn pintar(p: &bmo::Pantalla) -> bool {
    if !unsafe { LISTO } {
        return false;
    }
    // Se marca UNA vez: marcar por pixel es el 68 a 1 de `verde.rs`.
    p.marcar(0, 0, p.ancho, p.alto);
    for y in 0..p.alto {
        for x in 0..p.ancho {
            if let Some(c) = color(x, y) {
                p.punto_ya_marcado(x, y, c);
            }
        }
    }
    true
}
