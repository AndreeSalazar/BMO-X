//! **Las ordenes que PINTAN.** `cabina`, `fb` y `splash`.
//!
//! # Por que no estan con las de hardware, si el framebuffer es hardware
//!
//! Porque la pregunta es otra. `hardware` contesta *"que hay y como esta"*;
//! esto **cambia lo que se ve**. Un `fb` que reclama la pantalla o un `splash`
//! que la repinta no son diagnosticos: son actos.
//!
//! Y `cabina` esta aqui y no en hardware por lo mismo -- vuelca el anillo de
//! eventos a la pantalla y al disco, o sea que su trabajo es MOSTRAR, no medir.
//! Lo que mide es CABINA misma, que vive en `ring0/cabina.rs`.
//!
//! # El orden dentro de la carpeta
//!
//! Van las terceras: despues de mirar (`hardware`) y de leer datos
//! (`ficheros`), y antes de lo que no se deshace (`peligro`). Pintar se puede
//! repetir; borrar un fichero no.

use super::super::phase::s_log;
use super::super::splash;

/// `cabina` -- vuelca la bitacora de vuelo a disco.
///
/// Es el punto donde todo lo demas cobra sentido: hasta ahora CABINA lo veia
/// todo y lo olvidaba al apagar. Un registrador que solo existe mientras vuela
/// el avion no sirve para investigar la caida.
pub(crate) fn shell_cabina() {
    use crate::ring0::fsys::fs;
    if !fs::data_mounted() {
        s_log("[cabina] no hay volumen de datos donde escribir");
        s_log("[cabina] escribe 'disk' para ver que dijo el gate de identidad");
        return;
    }
    let n = crate::ring0::cabina::dump_to_disk();
    if n == 0 {
        s_log("[cabina] no se volco (el motivo esta en la bitacora)");
        return;
    }
    let mut b = [0u8; 80];
    let mut o = 0usize;
    for &c in b"[cabina] CABINA.LOG escrito: ".iter() { if o < b.len() { b[o] = c; o += 1; } }
    {
        let mut tmp = [0u8; 20];
        let mut i = 0;
        let mut v = n as u64;
        if v == 0 { tmp[0] = b'0'; i = 1; }
        while v > 0 { tmp[i] = b'0' + (v % 10) as u8; v /= 10; i += 1; }
        while i > 0 { i -= 1; if o < b.len() { b[o] = tmp[i]; o += 1; } }
    }
    for &c in b" bytes".iter() { if o < b.len() { b[o] = c; o += 1; } }
    if let Ok(s) = core::str::from_utf8(&b[..o]) { s_log(s); }
}

pub(crate) fn shell_fb() {
    if !crate::info::has_fb() {
        s_log("[fb] no framebuffer (headless boot)");
        return;
    }
    crate::ring0::dev::console::serial_write("[fb] base=0x");
    crate::ring0::dev::console::serial_write_u64(unsafe { crate::info::FB_ADDR }, 16);
    crate::ring0::dev::console::serial_write(" ");
    crate::ring0::dev::console::serial_write_u64(unsafe { crate::info::FB_WIDTH } as u64, 10);
    crate::ring0::dev::console::serial_write("x");
    crate::ring0::dev::console::serial_write_u64(unsafe { crate::info::FB_HEIGHT } as u64, 10);
    crate::ring0::dev::console::serial_write("x32 stride=");
    crate::ring0::dev::console::serial_write_u64(unsafe { crate::info::FB_STRIDE } as u64, 10);
    crate::ring0::dev::console::serial_write("\n");
}

pub(crate) fn shell_splash() {
    if !crate::info::has_fb() {
        s_log("[splash] no framebuffer");
        return;
    }
    splash::splash_init();
    splash::splash_progress(50, "Shell re-triggered splash");
    // Return to the persistent dashboard instead of clearing to black.
    splash::splash_dashboard_init();
    s_log("[splash] done");
}