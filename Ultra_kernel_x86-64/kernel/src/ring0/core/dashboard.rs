//! **THE ROLLING LOG** -- the band of text the kernel writes on screen.
//!
//! [carril]  VERDE     la banda de texto; equivocarse pinta feo
//!
//! === Why this is a file of its own ===
//!
//! It was living inside `phase.rs`, a file named after the boot sequence, and
//! it has nothing to do with booting: it is a **ring buffer of screen rows**
//! that anything in Ring 0 can write to, at any point in the machine's life.
//!
//! The three entry points differ only in colour, and the one rule underneath
//! them is the only thing worth remembering: **the log owns the panel MINUS the
//! band at the bottom, which belongs to CABINA**. Without that split the two
//! wrote into the same rows and erased each other -- the log ate the record and
//! the record ate the log.
//!
//! [!] The messages stay in Spanish. The boundary in this house is the quote: a
//! string is what BMO-X *prints*, and the console renderer is Latin-1 on
//! purpose.

use super::splash;

// Rolling index into the dashboard log. Each `dash_log` call
// advances this and wraps at the end of the log BAND.
pub(crate) static mut DASH_LOG_ROW: usize = 0;

/// Filas del log rodante: el panel entero MENOS la banda inferior que ocupa
/// CABINA. Sin este reparto los dos escribian en las mismas filas y se
/// borraban mutuamente (el log se comia la bitacora y viceversa).
pub(crate) fn log_rows() -> usize {
    let total = splash::dash_rows();
    if total == 0 { return 1; }
    total.saturating_sub(crate::ring0::cabina::band_rows(total)).max(1)
}

// Mirror the serial output to a line in the dashboard's log
// area, so the user can see what the kernel is doing without a
// serial terminal attached.
pub(crate) fn dash_log(msg: &str) {
    dashboard_log(msg);
}

/// Append one line to the rolling on-screen kernel log. Public so other
/// subsystems (e.g. the Ring 3 bootstrap console in `uconsole`) can surface
/// output in the same panel instead of maintaining a competing row cursor.
/// Framebuffer-only; a no-op on a headless (serial) boot.
pub fn dashboard_log(msg: &str) {
    dashboard_log_impl(msg, None);
}

/// Igual, pero eligiendo el color a mano.
///
/// El color por prefijo sirve para el log del kernel, donde el emisor SE
/// reconoce. Un informe del shell no tiene emisor: tiene estructura -- titulos,
/// etiquetas y valores -- y quien la conoce es quien lo escribe.
pub fn dashboard_log_color(msg: &str, color: u32) {
    dashboard_log_impl(msg, Some(color));
}

pub(crate) fn dashboard_log_impl(msg: &str, color: Option<u32>) {
    // * GUARDAR VA PRIMERO, antes de cualquier `return`.
    //
    // Y el orden es el punto entero de que esto exista. Debajo hay dos salidas
    // tempranas --sin framebuffer, sin filas-- que son razones para no PINTAR, no
    // para no RECORDAR. Guardar despues de ellas dejaria sin log justo los dos
    // casos en los que un log hace mas falta: el arranque antes de que haya
    // pantalla, y la maquina que ya cedio la pantalla a Ring 3.
    //
    // Ese segundo caso es el de todos los dias desde que el escritorio es el
    // arranque, y sin esto el relato de como arranco la maquina no existiria en
    // ninguna parte.
    //
    // ** [!] Y AQUI HABIA UNA FRASE QUE AFIRMABA UN INVARIANTE QUE NADIE
    // IMPLEMENTA. (corregida el 2026-09-02)
    //
    // Decia *"el panel del kernel no se pinta"* cuando Ring 3 tiene la
    // pantalla. **Nada lo comprueba.** Las dos salidas tempranas de abajo son
    // `has_fb()` y `rows == 0`, y `log_rows()` nunca da cero: su primera linea
    // es `if total == 0 { return 1; }`. Ni una mira `fb::owner()`.
    //
    // Lo que de verdad pasa es otra cosa, y conviene saberla porque se apoya en
    // una casualidad: **todos los que llaman aqui son deliberados o de
    // arranque** -- `phase` (arranca), `purga` y `emergencia` (Ctrl+Alt+Esc),
    // `cockpit` (F11) y el shell (una orden tuya). Ninguno se dispara solo,
    // asi que en la practica no se pinta encima de Ring 3.
    //
    // > Un invariante que no esta escrito no es un invariante: es una suerte
    // > que dura hasta que deja de durar.
    //
    // Es la MISMA frase que `scheduler::reap` ya tiene escrita sobre su propio
    // hueco. El dia que un subsistema de fondo llame aqui, pintara encima de
    // una app a pantalla completa y nadie lo habra decidido. Cuando eso pase,
    // la puerta es la de `splash_dashboard_log_color` --que ya existe para la
    // intro-- mas una variante `forzado` para las cuatro salidas que TIENEN que
    // pintar pase lo que pase.
    //
    // [!] Se deja MEDIDO y no arreglado a proposito: hoy no hay ni un llamante
    // de fondo, asi que la guarda no cambiaria una sola pantalla y costaria
    // diecinueve sitios. Lo que faltaba era que la frase dijera la verdad.
    //
    // ** Y va con la HORA delante. `[  1234ms] usb: ...`
    //
    // El arranque no estaba cronometrado en ninguna parte: se sabia que tarda
    // "unos diez segundos" y **no se sabia en que**. Optimizar sin ese numero
    // es mover cosas a ver si suena distinto -- y lo primero que hay que
    // descartar es que esos segundos sean del firmware de la placa y no de
    // BMO, en cuyo caso no hay nada que arreglar aqui.
    //
    // No cuesta un cronometro nuevo: `timer::ticks()` ya corre y cada evento de
    // la CABINA **ya guardaba su marca** -- solo que no se ensenaba. Con esto,
    // una sola foto de F11 dice donde se van los segundos, linea por linea.
    crate::ring0::core::klog::guardar_con_hora(crate::ring0::plat::timer::ticks(), msg);

    if !crate::info::has_fb() { return; }
    let rows = log_rows();
    if rows == 0 { return; }

    // -- Lineas repetidas: se cuentan, no se apilan --------------------------
    //
    // El censo de puertos del AHCI escupe una linea por puerto y la mayoria
    // son identicas: catorce `p0x0 ssts=0x0` seguidas se comian medio panel y
    // barrian el arranque entero fuera de la pantalla. Y el panel es la unica
    // ventana que hay -- aqui no se puede hacer scroll hacia atras.
    //
    // Una repeticion NO es informacion nueva; el numero de veces SI. Asi que
    // la fila se queda donde esta y se le anade el contador. Catorce lineas
    // pasan a ser una que dice `x14`, y las trece filas que ganamos son trece
    // hechos distintos que antes no cabian.
    const KEEP: usize = 96;
    static mut LAST_LINE: [u8; KEEP] = [0u8; KEEP];
    static mut LAST_LEN: usize = 0;
    static mut LAST_ROW: usize = usize::MAX;
    static mut REPEATS: u32 = 0;

    let b = msg.as_bytes();
    let n = b.len().min(KEEP);

    unsafe {
        let last = &mut *core::ptr::addr_of_mut!(LAST_LINE);
        if LAST_LEN == n && LAST_ROW < rows && last[..n] == b[..n] {
            REPEATS += 1;
            // Repintar la MISMA fila con la cuenta al final. El prefijo no
            // cambia, asi que el color de la linea sigue siendo el suyo.
            let mut buf = [0u8; KEEP + 12];
            let mut o = 0usize;
            for i in 0..n { buf[o] = b[i]; o += 1; }
            for &c in b"  x".iter() { if o < buf.len() { buf[o] = c; o += 1; } }
            let mut v = REPEATS + 1;
            let mut tmp = [0u8; 10];
            let mut i = 0usize;
            while v > 0 { tmp[i] = b'0' + (v % 10) as u8; v /= 10; i += 1; }
            while i > 0 { i -= 1; if o < buf.len() { buf[o] = tmp[i]; o += 1; } }
            if let Ok(s) = core::str::from_utf8(&buf[..o]) {
                match color {
                    Some(c) => splash::splash_dashboard_log_color(LAST_ROW, s, c),
                    None => splash::splash_dashboard_log(LAST_ROW, s),
                }
            }
            return;
        }
        last[..n].copy_from_slice(&b[..n]);
        LAST_LEN = n;
        REPEATS = 0;
    }

    let row = unsafe { DASH_LOG_ROW } % rows;
    unsafe { DASH_LOG_ROW = (row + 1) % rows; LAST_ROW = row; }
    match color {
        Some(c) => splash::splash_dashboard_log_color(row, msg, c),
        None => splash::splash_dashboard_log(row, msg),
    }
}

// -- Constructor de lineas del shell -----------------------------------------
//
// Cada comando del shell traia sus propias closures `txt`/`dec` copiadas.
// Esto es una sola, con lo que hace falta para alinear columnas y para decir
// un tamano en la unidad que se entiende.
