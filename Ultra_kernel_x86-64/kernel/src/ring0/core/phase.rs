//! Ring 0 boot phases - orchestrator for the kernel entry path.
//!
//! In Ultra_kernel_x86-64's minimal Ring 0 base we keep only what's necessary:
//! the splash animation, the framebuffer init, and a serial shell.
//! All GDT/IDT/CPU/MM/IRQ/SMP subsystems live in the faggin stages
//! (s2_gdt, s3_idt, s4_cpuid, s5_control, s9_paging) and are already
//! configured by the time the kernel runs.
//!
//! Phases:
//!   0. fb    - framebuffer init from BootContext
//!   1. ui    - splash animation (if FB available)
//!
//! After phases: serial shell takes over so the user has a way to
//! interact even without a display.

use boot_context::BootContext;
use super::splash;

fn s_log(msg: &str) {
    crate::ring0::dev::console::serial_write(msg);
    crate::ring0::dev::console::serial_write("\n");
    // Mirror to the on-screen log panel (if framebuffer present).
    dashboard_log(msg);
}

fn phase0_fb(ctx: &BootContext) {
    s_log("[phase0] === Framebuffer Init ===");
    let fmt = match ctx.fb_pixel_format {
        0 => crate::ring0::dev::framebuffer::PixelFormat::Bgr,
        1 => crate::ring0::dev::framebuffer::PixelFormat::Rgb,
        _ => crate::ring0::dev::framebuffer::PixelFormat::Unknown,
    };
    crate::ring0::dev::framebuffer::init_gop(
        ctx.fb_addr,
        ctx.fb_width,
        ctx.fb_height,
        ctx.fb_stride,
        fmt,
    );
    s_log("[phase0] done");
}

fn phase1_ui(_ctx: &BootContext) {
    s_log("[phase1] === UI (dashboard) ===");
    if crate::info::has_fb() {
        // Land on the persistent dashboard after the cinematic intro.
        splash::splash_dashboard_init();
    } else {
        s_log("[splash] no framebuffer, skipping");
    }
    s_log("[phase1] done");
}

// ---------------------------------------------------------------------------
// Serial shell (with optional framebuffer echo)
// ---------------------------------------------------------------------------

// Rolling index into the dashboard log. Each `dash_log` call
// advances this and wraps at the end of the log BAND.
static mut DASH_LOG_ROW: usize = 0;

/// Filas del log rodante: el panel entero MENOS la banda inferior que ocupa
/// CABINA. Sin este reparto los dos escribian en las mismas filas y se
/// borraban mutuamente (el log se comia la bitacora y viceversa).
fn log_rows() -> usize {
    let total = splash::dash_rows();
    if total == 0 { return 1; }
    total.saturating_sub(crate::ring0::cabina::band_rows(total)).max(1)
}

// Mirror the serial output to a line in the dashboard's log
// area, so the user can see what the kernel is doing without a
// serial terminal attached.
fn dash_log(msg: &str) {
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

fn dashboard_log_impl(msg: &str, color: Option<u32>) {
    // * GUARDAR VA PRIMERO, antes de cualquier `return`.
    //
    // Y el orden es el punto entero de que esto exista. Debajo hay dos salidas
    // tempranas --sin framebuffer, sin filas-- que son razones para no PINTAR, no
    // para no RECORDAR. Guardar despues de ellas dejaria sin log justo los dos
    // casos en los que un log hace mas falta: el arranque antes de que haya
    // pantalla, y la maquina que ya cedio la pantalla a Ring 3.
    //
    // Ese segundo caso es el de todos los dias desde que el escritorio es el
    // arranque: el panel del kernel no se pinta, asi que sin esto el relato de
    // como arranco la maquina no existia en ninguna parte.
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

/// Colores de los informes del shell. Etiqueta apagada, valor claro, titulo
/// ambar: la misma jerarquia en todos los comandos, para que `info` y `disk`
/// se lean como partes del mismo sistema y no como dos programas distintos.
const SH_TITLE: u32 = 0xFFF6C445; // ambar
const SH_LABEL: u32 = 0xFF00F0FF; // cian
const SH_VALUE: u32 = 0xFFE6EDF7; // texto

struct L { b: [u8; 96], o: usize }

impl L {
    fn new() -> Self { Self { b: [0u8; 96], o: 0 } }
    fn txt(&mut self, s: &str) {
        for &c in s.as_bytes() { if self.o < self.b.len() { self.b[self.o] = c; self.o += 1; } }
    }
    fn dec(&mut self, mut v: u64) {
        if v == 0 { self.txt("0"); return; }
        let mut tmp = [0u8; 20];
        let mut i = 0;
        while v > 0 { tmp[i] = b'0' + (v % 10) as u8; v /= 10; i += 1; }
        while i > 0 { i -= 1; if self.o < self.b.len() { self.b[self.o] = tmp[i]; self.o += 1; } }
    }
    fn hex(&mut self, v: u64, digits: usize) {
        const H: &[u8; 16] = b"0123456789ABCDEF";
        for i in (0..digits).rev() {
            if self.o < self.b.len() { self.b[self.o] = H[((v >> (i * 4)) & 0xF) as usize]; self.o += 1; }
        }
    }
    /// Rellena con espacios hasta la columna `col`. Es lo que mantiene las
    /// etiquetas alineadas sin contar caracteres a mano en cada linea.
    fn col(&mut self, col: usize) {
        while self.o < col && self.o < self.b.len() { self.b[self.o] = b' '; self.o += 1; }
    }
    /// Un tamano en la unidad que se entiende, con dos decimales.
    ///
    /// Sin coma flotante: la parte fraccionaria se saca multiplicando el resto
    /// por 100 antes de dividir. En Ring 0 no hay `f64` que valga -- y aunque
    /// lo hubiera, el estado SSE de la tarea no se preserva todavia.
    ///
    /// Unidades binarias (1 KiB = 1024 B) porque es lo que cuenta el
    /// asignador: sus marcos son de 4096 bytes, no de 4000.
    fn size(&mut self, bytes: u64) {
        const K: u64 = 1024;
        const M: u64 = K * 1024;
        const G: u64 = M * 1024;
        let (unit, div) = if bytes >= G { ("GiB", G) }
                          else if bytes >= M { ("MiB", M) }
                          else if bytes >= K { ("KiB", K) }
                          else { ("B", 1) };
        self.dec(bytes / div);
        if div > 1 {
            let frac = (bytes % div) * 100 / div;
            self.txt(".");
            if frac < 10 { self.txt("0"); }
            self.dec(frac);
        }
        self.txt(" ");
        self.txt(unit);
    }
    /// Porcentaje entero, sin flotantes.
    fn pct(&mut self, part: u64, whole: u64) {
        if whole == 0 { self.txt("0%"); return; }
        self.dec(part * 100 / whole);
        self.txt("%");
    }
    fn as_str(&self) -> &str { core::str::from_utf8(&self.b[..self.o]).unwrap_or("") }
}

/// Una fila de informe: etiqueta a la izquierda, valor alineado en la columna 10.
fn row(label: &str, build: impl FnOnce(&mut L)) {
    let mut l = L::new();
    l.txt(" ");
    l.txt(label);
    // Un espacio SIEMPRE, y luego la columna. Rellenar solo hasta la columna 10
    // dejaba pegados los valores de las etiquetas de 9 letras: en pantalla
    // salia "particion3" y "generacion1", que se leen como una sola palabra.
    l.txt(" ");
    l.col(12);
    build(&mut l);
    // La etiqueta va en cian y el valor en blanco, pero el panel pinta una
    // linea de un solo color: se elige el del VALOR, que es lo que se lee.
    dashboard_log_color(l.as_str(), SH_VALUE);
    crate::ring0::dev::console::serial_write(l.as_str());
    crate::ring0::dev::console::serial_write("\n");
}

// Mirror the current in-progress shell line to the framebuffer's prompt area,
// con CURSOR PARPADEANTE. Antes se repintaba en CADA iteracion del loop del
// shell (limpiar+dibujar sin cambio) -> ese era el ghosting ocasional del
// prompt. Ahora solo repinta cuando: cambia la linea, parpadea el cursor, o
// hubo un clear. Pantalla estable + cursor vivo.
fn dash_prompt(line: &str, cursor: usize) {
    if !crate::info::has_fb() { return; }
    let ticks = crate::ring0::plat::timer::ticks();
    let blink = ((ticks >> 6) & 1) == 0; // visible ~mitad del tiempo
    let n = line.len();
    static mut LAST_N: usize = usize::MAX;
    static mut LAST_CUR: usize = usize::MAX;
    static mut LAST_BLINK: bool = false;
    static mut LAST_GEN: u32 = u32::MAX;
    static mut LAST_LOCKS: u8 = 0xFF;
    // El estado de los bloqueos entra en la firma: al pulsar Bloq Mayus la
    // linea no cambia, pero el indicador si -- y hay que repintarlo.
    let (caps, num) = crate::ring0::dev::keyboard::lock_state();
    let locks = (caps as u8) | ((num as u8) << 1);
    unsafe {
        let gen = SCREEN_GEN;
        if LAST_N == n && LAST_CUR == cursor && LAST_BLINK == blink
            && LAST_GEN == gen && LAST_LOCKS == locks { return; }
        LAST_N = n; LAST_CUR = cursor; LAST_BLINK = blink; LAST_GEN = gen; LAST_LOCKS = locks;
    }
    splash::splash_dashboard_prompt(line, cursor, blink);
    // Indicadores a la derecha de la barra: distribucion activa y bloqueos.
    // Los LEDs fisicos de este teclado no responden; la pantalla nunca miente.
    splash::splash_status_right(crate::ring0::dev::keyboard::layout_name(), caps, num);
}

fn shell_prompt() {
    crate::ring0::dev::console::serial_write("> ");
    dash_prompt("", 0);
}

/// Generacion de pantalla: se incrementa en cada limpieza. Los paneles de fila
/// FIJA (heartbeat, usb) la comparan para FORZAR un repintado tras un clear,
/// aunque sus valores no hayan cambiado -- si no, la deteccion de cambios los
/// dejaria en blanco para siempre despues de limpiar (bug real observado).
static mut SCREEN_GEN: u32 = 0;

/// Generacion de pantalla actual -- CABINA la usa para repintar su cockpit tras
/// un clear (mismo mecanismo anti-ghosting que los paneles fijos).
pub(crate) fn screen_gen() -> u32 { unsafe { SCREEN_GEN } }

/// Limpia la pantalla y re-dibuja el dashboard vacio (comando `cls` y
/// auto-limpieza al terminar un proceso). Reinicia el cursor rodante del log
/// para que el panel arranque de cero, como una terminal recien abierta.
pub(crate) fn clear_screen() {
    if !crate::info::has_fb() { return; }
    splash::splash_clear();
    splash::splash_dashboard_init();
    unsafe {
        DASH_LOG_ROW = 0;
        SCREEN_GEN = SCREEN_GEN.wrapping_add(1); // fuerza repintado de paneles fijos
    }
}

// Los paneles de fila fija `dash_heartbeat` (r3hb, fila 10) y `dash_usb_status`
// (usb, fila 12) VIVIAN aqui. CABINA los absorbio: su banda inferior muestra la
// misma telemetria (ticks/switches/estado de Ring 3/USB detallado) en una vista
// coherente y con color semantico, y ademas con la bitacora de eventos que
// aquellos no tenian. Ya nadie los llamaba -- codigo muerto pintando filas que
// el log rodante volvia a borrar. Se eliminan: CABINA es el unico observador.

/// Ultimo total de tareas visto por el shell, para detectar cuando un proceso
/// TERMINO (el total baja) y limpiar la pantalla automaticamente.
static mut LAST_TASK_TOTAL: usize = 0;

// -- Historial de comandos ---------------------------------------------------
//
// Un anillo de las ultimas lineas ejecutadas, recorrible con las flechas
// arriba/abajo. Sin esto, repetir un comando es volver a teclearlo entero.

const HIST_MAX: usize = 16;
const HIST_LINE: usize = 64;
static mut HIST: [[u8; HIST_LINE]; HIST_MAX] = [[0; HIST_LINE]; HIST_MAX];
static mut HIST_LEN: [usize; HIST_MAX] = [0; HIST_MAX];
static mut HIST_COUNT: usize = 0; // cuantas lineas hay (tope HIST_MAX)
static mut HIST_HEAD: usize = 0;  // donde se escribe la siguiente

/// Guarda una linea en el historial. No repite la inmediatamente anterior:
/// pulsar Enter tres veces sobre el mismo comando no deberia llenar el
/// historial de copias.
fn hist_push(line: &[u8]) {
    if line.is_empty() || line.len() > HIST_LINE { return; }
    unsafe {
        if HIST_COUNT > 0 {
            let last = (HIST_HEAD + HIST_MAX - 1) % HIST_MAX;
            if HIST_LEN[last] == line.len() && HIST[last][..line.len()] == *line { return; }
        }
        HIST[HIST_HEAD][..line.len()].copy_from_slice(line);
        HIST_LEN[HIST_HEAD] = line.len();
        HIST_HEAD = (HIST_HEAD + 1) % HIST_MAX;
        if HIST_COUNT < HIST_MAX { HIST_COUNT += 1; }
    }
}

/// Entrada `back` posiciones hacia atras (1 = la ultima ejecutada).
fn hist_get(back: usize) -> Option<&'static [u8]> {
    unsafe {
        if back == 0 || back > HIST_COUNT { return None; }
        let idx = (HIST_HEAD + HIST_MAX - back) % HIST_MAX;
        Some(&HIST[idx][..HIST_LEN[idx]])
    }
}

/// Lee una linea del teclado con edicion completa: cursor, historial y los
/// atajos de Ctrl de toda la vida.
///
/// Devuelve `(largo, cancelada)`; cancelada = el usuario pulso Ctrl+C.
fn shell_read_line(buf: &mut [u8]) -> usize {
    use crate::ring0::dev::keyboard as kb;
    let mut n = 0;     // largo de la linea
    let mut cur = 0;   // posicion del cursor dentro de la linea
    let mut hist_at = 0; // 0 = linea nueva; >0 = navegando el historial

    // Inserta un byte en la posicion del cursor, desplazando lo que haya.
    fn insert(buf: &mut [u8], n: &mut usize, cur: &mut usize, c: u8) {
        if *n >= buf.len() { return; }
        let mut i = *n;
        while i > *cur { buf[i] = buf[i - 1]; i -= 1; }
        buf[*cur] = c;
        *cur += 1;
        *n += 1;
    }
    // Borra el byte ANTERIOR al cursor (retroceso).
    fn erase(buf: &mut [u8], n: &mut usize, cur: &mut usize) {
        if *cur == 0 { return; }
        let mut i = *cur;
        while i < *n { buf[i - 1] = buf[i]; i += 1; }
        *cur -= 1;
        *n -= 1;
    }

    loop {
        // Auto-limpieza: si un proceso termino (el total de tareas bajo) y NO
        // estas escribiendo (linea vacia), limpia la pantalla -- como una
        // terminal que se refresca al acabar el programa. Nunca borra a media
        // escritura (solo con n==0).
        let (total, _) = crate::ring0::task::scheduler::counts();
        unsafe {
            if n == 0 && total < LAST_TASK_TOTAL {
                clear_screen();
                dash_log("== proceso terminado : pantalla limpia ==");
            }
            LAST_TASK_TOTAL = total;
        }
        dash_prompt(core::str::from_utf8(&buf[..n]).unwrap_or(""), cur);
        // CABINA -- cockpit omnisciente en la banda inferior.
        crate::ring0::cabina::render_hud();

        // Entrada: serial (COM1), teclado USB o PS/2, lo que tenga un byte.
        //
        // * El SERIAL nunca se cede. Es el cable del que depura, y sigue
        // hablando aunque Ring 3 sea dueno de la pantalla y del teclado -- que
        // es justo cuando mas falta hace.
        let mut byte = crate::ring0::dev::console::serial_read_byte();
        // * El teclado FISICO si. Si un proceso reclamo `KIND_INPUT`, las
        // teclas son suyas y este shell no las toca: los dos drenan la MISMA
        // cola, asi que leer aqui no seria "leer tambien", seria robarle letras
        // sueltas a la caja. Cedido es cedido, tambien para el que la cedio.
        if byte.is_none() && !crate::ring0::obj::input::yielded() {
            byte = crate::ring0::dev::usb::poll_ascii();
            if byte.is_none() {
                // PS/2 i8042 (mudo post-EBS en esta placa). Se conserva por si
                // algun dia reviviera (adaptador PS/2, otra placa).
                if let Some((_raw, ascii)) = kb::poll_event() {
                    byte = ascii;
                }
            }
        }
        let c = match byte { Some(c) => c, None => continue };

        match c {
            b'\r' | b'\n' => {
                crate::ring0::dev::console::serial_write("\n");
                hist_push(&buf[..n]);
                return n;
            }
            0x7f | 0x08 => { // Retroceso
                erase(buf, &mut n, &mut cur);
            }
            kb::KEY_DELETE => { // Suprimir: borra HACIA ADELANTE
                if cur < n {
                    let mut i = cur + 1;
                    while i < n { buf[i - 1] = buf[i]; i += 1; }
                    n -= 1;
                }
            }
            kb::KEY_LEFT  => { if cur > 0 { cur -= 1; } }
            kb::KEY_RIGHT => { if cur < n { cur += 1; } }
            kb::KEY_HOME  => { cur = 0; }
            kb::KEY_END   => { cur = n; }
            kb::KEY_UP => {
                // Hacia atras en el historial.
                if let Some(h) = hist_get(hist_at + 1) {
                    hist_at += 1;
                    n = h.len().min(buf.len());
                    buf[..n].copy_from_slice(&h[..n]);
                    cur = n;
                }
            }
            kb::KEY_DOWN => {
                if hist_at > 1 {
                    hist_at -= 1;
                    if let Some(h) = hist_get(hist_at) {
                        n = h.len().min(buf.len());
                        buf[..n].copy_from_slice(&h[..n]);
                        cur = n;
                    }
                } else {
                    // Se acabo el historial: vuelta a la linea en blanco.
                    hist_at = 0;
                    n = 0;
                    cur = 0;
                }
            }
            0x01 => { cur = 0; }              // Ctrl+A: al principio
            0x05 => { cur = n; }              // Ctrl+E: al final
            0x03 => {                          // Ctrl+C: cancelar la linea
                crate::ring0::dev::console::serial_write("^C\n");
                return 0;
            }
            0x0C => { clear_screen(); }        // Ctrl+L: limpiar pantalla
            0x15 => { n = 0; cur = 0; }        // Ctrl+U: borrar la linea entera
            0x0B => { n = cur; }               // Ctrl+K: borrar hasta el final
            0x17 => {                          // Ctrl+W: borrar la palabra
                while cur > 0 && buf[cur - 1] == b' ' { erase(buf, &mut n, &mut cur); }
                while cur > 0 && buf[cur - 1] != b' ' { erase(buf, &mut n, &mut cur); }
            }
            // Imprimible = ASCII visible O byte Latin-1 alto (n, a, , ...).
            // El teclado espanol entrega un byte por caracter y el font sabe
            // dibujarlos: dejarlos pasar es todo lo que hace falta.
            c if c >= 0x20 && c != 0x7f && !kb::is_nav(c) => {
                insert(buf, &mut n, &mut cur, c);
                crate::ring0::dev::console::serial_write_byte(c);
            }
            _ => {}
        }
    }
}

fn shell_help() {
    // Por categorias y con las columnas alineadas por el mismo constructor que
    // usan `info` y `disk`: antes cada comando alineaba a ojo con espacios
    // contados a mano, y bastaba una palabra mas larga para torcer la columna.
    dashboard_log_color("== BMO-X shell ==", SH_TITLE);
    row("sistema", |l| l.txt("info  cpu  mem  tasks  disk  ls  estratos  cabina  hist"));
    row("edicion", |l| l.txt("flechas  Inicio/Fin  Supr  ^A ^E ^U ^K ^W ^C ^L"));
    row("video", |l| l.txt("fb  splash  cls"));
    row("ring3", |l| l.txt("run <ruta>  bex  ktest"));
    row("poder", |l| l.txt("reboot  halt  panic"));
    row("ayuda", |l| l.txt("help"));
}

/// `disk` -- que disco tiene BMO delante y que hay en el.
///
/// La tabla de particiones es como el kernel RECONOCE su disco: no se fia del
/// orden en que el PCI enumere ni de que el firmware repita el mismo orden dos
/// veces. El disco propio es el que lleva estas particiones y no otras.
fn shell_disk() {
    use crate::ring0::dev::disk;
    if !disk::is_ready() {
        s_log("[disk] sin disco SATA listo (mira la bitacora de CABINA)");
        return;
    }
    fn txt(b: &mut [u8; 80], o: &mut usize, t: &str) {
        for &c in t.as_bytes() { if *o < b.len() { b[*o] = c; *o += 1; } }
    }
    fn dec(b: &mut [u8; 80], o: &mut usize, mut v: u64, width: usize) {
        let mut tmp = [0u8; 20];
        let mut i = 0;
        if v == 0 { tmp[0] = b'0'; i = 1; }
        while v > 0 { tmp[i] = b'0' + (v % 10) as u8; v /= 10; i += 1; }
        for _ in i..width { if *o < b.len() { b[*o] = b' '; *o += 1; } }
        while i > 0 { i -= 1; if *o < b.len() { b[*o] = tmp[i]; *o += 1; } }
    }

    // Quien es el disco, segun el mismo. Con tres discos en la maquina y el
    // sistema del dueno en uno de ellos, esta linea es la que autoriza (o no)
    // a escribir algun dia.
    {
        let mut b = [0u8; 80];
        let mut o = 0usize;
        txt(&mut b, &mut o, "[disk] ");
        txt(&mut b, &mut o, disk::model());
        txt(&mut b, &mut o, "  ");
        dec(&mut b, &mut o, disk::total_sectors() >> 21, 1);
        txt(&mut b, &mut o, " GiB");
        if let Ok(s) = core::str::from_utf8(&b[..o]) { s_log(s); }
    }
    {
        let mut b = [0u8; 80];
        let mut o = 0usize;
        txt(&mut b, &mut o, "[disk] AHCI mmio=0x");
        {
            const H: &[u8; 16] = b"0123456789ABCDEF";
            for i in (0..8).rev() {
                if o < b.len() { b[o] = H[((disk::mmio() >> (i * 4)) & 0xF) as usize]; o += 1; }
            }
        }
        txt(&mut b, &mut o, " puerto=");
        dec(&mut b, &mut o, disk::port() as u64, 1);
        txt(&mut b, &mut o, "  sectores=");
        dec(&mut b, &mut o, disk::last_lba(), 1);
        if let Ok(s) = core::str::from_utf8(&b[..o]) { s_log(s); }
    }

    let parts = disk::partitions();
    if parts.is_empty() {
        s_log("[disk] sin tabla de particiones legible");
        return;
    }
    s_log(" #   primer LBA      GiB  tipo      nombre");
    for p in parts {
        let mut b = [0u8; 80];
        let mut o = 0usize;
        txt(&mut b, &mut o, " ");
        dec(&mut b, &mut o, p.index as u64, 2);
        dec(&mut b, &mut o, p.first_lba, 12);
        // Sectores de 512 B -> GiB: >>21 es dividir entre 2 Mi sectores.
        dec(&mut b, &mut o, p.sectors() >> 21, 9);
        txt(&mut b, &mut o, "  ");
        let tipo = if p.is_esp() { "ESP/boot " }
                   else if p.is_basic_data() { "datos    " }
                   else { "otro     " };
        txt(&mut b, &mut o, tipo);
        txt(&mut b, &mut o, p.name_str());
        if let Ok(s) = core::str::from_utf8(&b[..o]) { s_log(s); }
    }
    // El veredicto del gate, en palabras. Es la linea que decide si este disco
    // se puede escribir, asi que se pinta siempre -- diga que si o que no.
    {
        let mut b = [0u8; 80];
        let mut o = 0usize;
        txt(&mut b, &mut o, "[disk] ");
        txt(&mut b, &mut o, disk::gate_reason());
        if let Ok(s) = core::str::from_utf8(&b[..o]) { s_log(s); }
    }
    if disk::write_armed() {
        let mut b = [0u8; 80];
        let mut o = 0usize;
        txt(&mut b, &mut o, "[disk] serie=");
        txt(&mut b, &mut o, disk::serial());
        if let Some(p) = disk::data_partition() {
            txt(&mut b, &mut o, "  ventana=particion ");
            dec(&mut b, &mut o, p.index as u64, 1);
        }
        if let Ok(s) = core::str::from_utf8(&b[..o]) { s_log(s); }
    }
}

/// `cabina` -- vuelca la bitacora de vuelo a disco.
///
/// Es el punto donde todo lo demas cobra sentido: hasta ahora CABINA lo veia
/// todo y lo olvidaba al apagar. Un registrador que solo existe mientras vuela
/// el avion no sirve para investigar la caida.
fn shell_cabina() {
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

/// `ls` -- recorre `EFI\BOOT\BOOTX64.EFI` de la particion de arranque y lo lee.
///
/// Lo que esto demuestra: que el camino entero --AHCI, GPT, FAT32, directorios,
/// cadena de clusteres-- funciona de punta a punta contra un archivo real.
///
/// Lo que NO demuestra: que ese archivo sea el nuestro. La version anterior
/// remataba con "es un ejecutable UEFI: SOY YO" a partir de la firma `MZ`, que
/// la lleva CUALQUIER ejecutable de Windows. En este disco la particion de
/// arranque es la ESP de 0,6 GB que comparte con el sistema del dueno, asi que
/// bien puede ser su cargador. Se dice lo que se sabe.
fn shell_ls() {
    use crate::ring0::fsys::fs;
    if !fs::is_mounted() {
        s_log("[fs] no hay volumen montado (mira la bitacora de CABINA)");
        return;
    }

    dashboard_log_color("== volumen de arranque ==", SH_TITLE);
    row("formato", |l| { l.txt(fs::fs_name()); l.txt("  LBA "); l.dec(fs::mounted_lba()); l.txt("  solo lectura"); });

    // Los nombres van en 8.3 crudo: 8 de nombre + 3 de extension, con
    // espacios de relleno. Feo, pero es como FAT los guarda en disco.
    let efi = match fs::find_dir(b"EFI        ") {
        Some(c) => c,
        None => { s_log("[fs] no encuentro el directorio EFI"); return; }
    };
    let boot = match fs::find_dir_in(b"BOOT       ", efi) {
        Some(c) => c,
        None => { s_log("[fs] no encuentro EFI\\BOOT"); return; }
    };
    // * Y buscar el archivo DENTRO de `boot`, no en la raiz. El primer
    // intento encontro los dos directorios y luego pregunto por el archivo en
    // la raiz de todas formas: tenia el cluster correcto en la mano y lo tiro.
    let (cluster, size) = match fs::find_in(b"BOOTX64 EFI", boot) {
        Some(v) => v,
        None => { s_log("[fs] BOOTX64.EFI no esta en EFI\\BOOT"); return; }
    };
    row("archivo", |l| { l.txt("EFI\\BOOT\\BOOTX64.EFI"); });
    row("tamano", |l| { l.size(size as u64); l.txt("   "); l.dec(size as u64); l.txt(" B   cluster "); l.dec(cluster as u64); });

    // Leer los primeros bytes. Un archivo que se encuentra pero no se lee no
    // demuestra nada.
    let mut head = [0u8; 64];
    let n = fs::read(cluster, 64.min(size), &mut head);
    if n >= 2 && head[0] == b'M' && head[1] == b'Z' {
        row("firma", |l| { l.txt("MZ  ejecutable PE   "); l.dec(n as u64); l.txt(" bytes leidos"); });
        row("cadena", |l| { l.txt("AHCI -> GPT -> FAT32 -> directorios -> clusteres  OK"); });
        crate::ring0::cabina::info("fs", "cadena de lectura verificada contra un PE real", size as u64);
    } else {
        row("firma", |l| { l.txt("?? no es un PE: revisar la cadena de clusteres"); });
        crate::ring0::cabina::warn("fs", "el archivo se encontro pero no se leyo bien", n as u64);
    }
}

/// `cpu` -- que estado extendido tiene este procesador y si el perfil acierta.
///
/// La pregunta que responde: **hay registros que el cambio de contexto esta
/// perdiendo hoy?** Todos los numeros salen de `CPUID` hoja 0xD; el perfil solo
/// sirve para avisar si el silicio no es el que esperaba.
fn shell_cpu() {
    use crate::ring0::cpu_vendor::xsave;
    let inf = xsave::informe();
    let p = crate::ring0::cpu_vendor::profile::active();

    dashboard_log_color("== CPU : estado extendido ==", SH_TITLE);
    row("perfil", |l| { l.txt(p.name); l.txt("  "); l.txt(p.microarch); });
    if !inf.xsave {
        row("xsave", |l| l.txt("el procesador no lo implementa"));
        return;
    }
    row("xsave", |l| {
        l.txt("si   guardado: ");
        if inf.xsavec { l.txt("XSAVEC "); }
        if inf.xsaveopt { l.txt("XSAVEOPT "); }
        if inf.xsaves { l.txt("XSAVES"); }
    });
    row("soporta", |l| { l.txt("0x"); l.hex(inf.soportado, 4); l.txt("   todo: "); l.dec(inf.area_maxima as u64); l.txt(" B"); });
    row("xcr0", |l| {
        if inf.osxsave {
            l.txt("0x"); l.hex(inf.xcr0, 4);
            // El area que IMPORTA: la de los componentes habilitados ahora, no
            // la maxima teorica del CPU.
            l.txt("   habilitado: "); l.dec(inf.area_actual as u64); l.txt(" B");
        } else {
            l.txt("CR4.OSXSAVE apagado — el estado extendido no esta habilitado");
        }
    });

    // Cada componente con su tamano y su sitio, tal como los declara el CPU.
    for c in inf.comps() {
        let mut l = L::new();
        l.txt("   bit ");
        l.dec(c.bit as u64);
        l.col(12);
        l.txt(c.name());
        l.col(32);
        l.dec(c.tam as u64); l.txt(" B en +"); l.dec(c.offset as u64);
        dashboard_log_color(l.as_str(), SH_VALUE);
        crate::ring0::dev::console::serial_write(l.as_str());
        crate::ring0::dev::console::serial_write("\n");
    }

    // El veredicto contra el perfil, y el aviso que justifica todo esto.
    row("perfil dice", |l| { l.txt("0x"); l.hex(p.xsave_componentes, 4); l.txt("   area "); l.dec(p.xsave_area as u64); l.txt(" B"); });
    // XCR0 aparte: lo habilitado no es lo soportado, y este lo pone el firmware,
    // no el kernel -- es el unico de los tres que puede moverse por debajo.
    row("perfil xcr0", |l| { l.txt("0x"); l.hex(p.xsave_xcr0, 4); l.txt("   habilitado que se espera"); });
    // El MISMO veredicto que dio el arranque. Antes esta linea tenia su propia
    // comparacion y contestaba distinto a la misma pregunta.
    let coincide = xsave::coincide(&inf);
    row("veredicto", |l| l.txt(if coincide { "el silicio coincide con el perfil" }
                                else { "DIFIERE — manda el silicio, el perfil esta desfasado" }));
    // Lo que hace el cambio de contexto HOY. Esta linea decia "AVX aun no es
    // seguro" mucho despues de que dejara de ser cierto: un informe que se
    // queda contando una etapa anterior es peor que no tener informe, porque
    // se le cree.
    row("contexto", |l| {
        l.txt("XSAVE  reserva "); l.dec(crate::ring0::plat::trap::XSAVE_AREA as u64);
        l.txt(" B  usa "); l.dec(inf.area_actual as u64); l.txt(" B");
    });
    if inf.hay_estado_sin_guardar() {
        row("preserva", |l| {
            l.txt("mas alla de x87/SSE: 0x");
            l.hex(inf.soportado & !0b11, 4);
            l.txt("  (AVX seguro en Ring 3)");
        });
    }
}

/// `estratos` -- el estado del volumen propio y su raiz.
///
/// Es la primera vez que BMO-X lee un sistema de ficheros **suyo**: FAT32 es
/// un formato prestado que habia que entender; ESTRATOS lo escribio el.
fn shell_estratos() {
    use crate::ring0::fsys::estratos as est;
    if !est::is_mounted() {
        s_log("[estratos] ninguna particion tiene un volumen ESTRATOS");
        s_log("[estratos] se formatea desde el anfitrion con estratos-fmt");
        return;
    }
    let sb = match est::superbloque() { Some(s) => s, None => return };

    dashboard_log_color("== ESTRATOS ==", SH_TITLE);
    row("particion", |l| { l.dec(est::particion() as u64); l.txt("   LBA "); l.dec(est::base_lba()); });
    row("generacion", |l| { l.dec(sb.generation); l.txt("   bloques "); l.dec(sb.total_blocks); });
    row("log", |l| { l.txt("cabeza en el bloque "); l.dec(sb.log_head); });

    // -- El espacio, que es lo que decide si se puede escribir --
    //
    // Un FS que no sobreescribe se llena AUNQUE nadie cree un archivo: cada
    // version se queda. Por eso esto no es un adorno del panel -- es la
    // condicion previa al paso 5 del diseno, y el aviso que impide que el
    // volumen se llene por sorpresa (section 9).
    if let Some(oc) = est::ocupacion() {
        row("espacio", |l| {
            l.size(oc.bytes_usados());
            l.txt(" usados de ");
            l.size(oc.bytes_usados() + oc.bytes_libres());
            l.txt("   (");
            l.dec(oc.por_ciento() as u64);
            l.txt("%)");
        });
        row("libre", |l| {
            l.size(oc.bytes_libres());
            l.txt("   ");
            l.txt(oc.nivel().name());
        });
        // Lo que de verdad contesta "cuando hara falta el recolector?": no un
        // porcentaje, sino cuantas VERSIONES mas caben. Con 414 GiB la
        // respuesta son millones, y por eso el GC es "algun dia".
        row("caben", |l| {
            l.dec(oc.caben_de(20 * 1024));
            l.txt(" objetos mas de 20 KiB (un .bex de C)");
        });
        if !oc.nivel().admite_escritura() {
            crate::ring0::cabina::fault(
                "estratos",
                "volumen al 95%: SOLO LECTURA hasta que se libere sitio",
                oc.por_ciento() as u64,
            );
        }
    }
    // El gate del diseno: si el volumen no nacio aqui, se dice EN ALTO. Hoy
    // solo se lee, pero el dia que se escriba esta linea es la que decide.
    row("identidad", |l| {
        l.txt(if est::identidad_ok() { "es de ESTE disco" } else { "NO nacio en este disco (clonado?)" });
    });

    if let Some(e) = est::estrato() {
        row("estrato", |l| { l.txt("\""); l.txt(e.motivo_str()); l.txt("\""); });
    }

    let (_, raiz) = match est::raiz() {
        Some(v) => v,
        None => { s_log("[estratos] el volumen no tiene raiz (recien formateado?)"); return; }
    };
    let (n, truncado) = match est::entradas(&raiz) {
        Some(v) => v,
        None => { s_log("[estratos] no se pudo leer la raiz"); return; }
    };
    for i in 0..n {
        if let Some(e) = est::entrada(i) {
            let hijo = est::nodo(&e.nodo);
            let dir = matches!(hijo.map(|h| h.tipo), Some(bmo_estratos::Tipo::Directorio));
            let mut l = L::new();
            l.txt("  ");
            l.txt(e.nombre_str());
            if dir { l.txt("/"); }
            dashboard_log_color(l.as_str(), SH_VALUE);
            crate::ring0::dev::console::serial_write(l.as_str());
            crate::ring0::dev::console::serial_write("\n");
        }
    }
    if truncado {
        s_log("[estratos] ...la raiz tiene mas entradas de las que caben en el listado");
    }
}

/// `run <ruta>` -- carga un `.bex` del disco y lo ejecuta.
///
/// Es el punto donde el trabajo del disco cobra sentido. Hasta ahora los
/// programas Ring 3 vivian DENTRO del kernel (`include_bytes!`): cambiar un
/// "hola mundo" obligaba a recompilar el sistema operativo entero y
/// reflashear. Ahora se copia el `.bex` a la particion desde el anfitrion y se
/// escribe `run c/holac.bex`.
///
/// El buffer es estatico y no local: un `.bex` son varios KiB y la pila del
/// kernel son 64 KiB para todo.
fn shell_run(arg: &[u8]) {
    use crate::ring0::fsys::estratos as est;
    use crate::ring0::task::lanzar;

    let path = match core::str::from_utf8(arg) {
        Ok(s) => s.trim(),
        Err(_) => { s_log("[run] la ruta tiene bytes que no son texto"); return; }
    };

    // Buscar el archivo, comprobar la firma y admitirlo ya NO se hace aqui: lo
    // hace `lanzar::ruta`, que es EL MISMO camino que usa la caja de Ring 3.
    // Tener dos versiones del gate de firma era tener dos versiones que se
    // separan en cuanto alguien toque una. Al shell le queda lo suyo, que es
    // contarlo en filas.
    let inf = lanzar::ruta(path);

    if inf.res == Err(lanzar::Fallo::RutaVacia) {
        s_log("[run] uso: run c/holac.bex   (o A:/c/holac.bex)");
        return;
    }
    if let Err(lanzar::Fallo::NoSeEncuentra(e)) = inf.res {
        // El motivo exacto: "no esta" y "no cabe en 8.3" mandan a hacer cosas
        // distintas, y un "no se pudo" no manda a ninguna.
        let mut l = L::new();
        l.txt("[run] ");
        l.txt(e);
        l.txt(": ");
        l.txt(path);
        dashboard_log_color(l.as_str(), SH_TITLE);
        crate::ring0::dev::console::serial_write(l.as_str());
        crate::ring0::dev::console::serial_write("\n");
        return;
    }

    dashboard_log_color("== run ==", SH_TITLE);
    row("archivo", |l| { l.txt(path); });

    // Origen, tamano y firma solo si se llego a LEER el archivo. Con
    // `SinHueco` u `Ocupado` no se abrio nada, y pintar entonces "FAT32 no
    // puede llevar firma" seria contestar una pregunta que no se hizo -- el
    // informe hablaria de un archivo que nadie miro.
    if inf.bytes > 0 {
        row("origen", |l| { l.txt(inf.origen); });
        row("leido", |l| { l.size(inf.bytes as u64); });
        match inf.firma {
            Some(est::Firma::Cuadra) => row("firma", |l| l.txt("cuadra con el contenido")),
            Some(est::Firma::NoCuadra) => row("firma", |l| l.txt("NO CUADRA: el archivo no es el que se guardo")),
            Some(est::Firma::Ausente) => row("firma", |l| l.txt("el nodo no lleva :firma")),
            // Honestidad sobre la asimetria: FAT32 no tiene atributos con
            // nombre, asi que un binario de ahi no PUEDE traer su firma
            // pegada. No es que no se compruebe por pereza: es que no hay
            // donde guardarla.
            None => row("firma", |l| l.txt("FAT32 no puede llevar firma (sin atributos)")),
        }
    }

    match inf.res {
        Ok(tid) => {
            row("admitido", |l| { l.txt("tid "); l.dec(tid as u64); l.txt("   corre en el siguiente tick"); });
        }
        Err(f @ (lanzar::Fallo::FirmaMala | lanzar::Fallo::SinFirma)) => {
            row("gate", |l| { l.txt("RECHAZADO — "); l.txt(f.motivo()); });
        }
        Err(f) => {
            row("rechazado", |l| { l.txt(f.motivo()); });
        }
    }
}

/// `hist` -- la lista de comandos ejecutados, numerada. Lo mismo que recorren
/// las flechas arriba/abajo, pero de un vistazo.
fn shell_hist() {
    let count = unsafe { HIST_COUNT };
    if count == 0 {
        s_log("[hist] todavia no has ejecutado nada");
        return;
    }
    s_log("== historial (flechas arriba/abajo para recuperarlos) ==");
    // Del mas antiguo al mas reciente, que es como se lee una lista.
    for back in (1..=count).rev() {
        if let Some(line) = hist_get(back) {
            let mut b = [0u8; 80];
            let mut o = 0;
            let idx = count - back + 1;
            if idx >= 10 { b[o] = b'0' + (idx / 10) as u8; o += 1; }
            b[o] = b'0' + (idx % 10) as u8; o += 1;
            for &c in b"  ".iter() { if o < b.len() { b[o] = c; o += 1; } }
            for &c in line { if o < b.len() { b[o] = c; o += 1; } }
            if let Ok(s) = core::str::from_utf8(&b[..o]) { s_log(s); }
        }
    }
}

/// `layout` -- muestra o cambia la distribucion del teclado EN CALIENTE.
/// El scancode dice que tecla se pulso, no que letra es; si lo que sale no
/// coincide con lo impreso en tus teclas, prueba otra aqui mismo.
fn shell_layout(arg: &[u8]) {
    use crate::ring0::dev::keyboard::{self, Layout};
    let chosen = match arg {
        b"" => None,
        b"us" => Some(Layout::Us),
        b"latam" | b"es-latam" | b"la" => Some(Layout::EsLatam),
        b"es" | b"espana" | b"es-espana" => Some(Layout::EsSpain),
        _ => {
            s_log("[layout] opciones: us | latam | es");
            return;
        }
    };
    if let Some(l) = chosen {
        keyboard::set_layout(l);
        crate::ring0::cabina::info("kbd", keyboard::layout_name(), 0);
    }
    let mut b = [0u8; 64];
    let mut o = 0;
    for &c in b"[layout] activo: ".iter() { if o < b.len() { b[o] = c; o += 1; } }
    for &c in keyboard::layout_name().as_bytes() { if o < b.len() { b[o] = c; o += 1; } }
    for &c in b"  (us | latam | es)".iter() { if o < b.len() { b[o] = c; o += 1; } }
    if let Ok(s) = core::str::from_utf8(&b[..o]) { s_log(s); }
}

/// `info` -- el informe completo de la maquina.
///
/// Antes escribia TODO al puerto serie y al panel no llegaba nada: en una
/// maquina sin cable serie el comando parecia no hacer nada. Ahora cada linea
/// va a los dos sitios.
fn shell_info(ctx: &BootContext) {
    use crate::ring0::mm::phys;
    const PAGE: u64 = 4096;

    dashboard_log_color("== BMO-X : informe del sistema ==", SH_TITLE);

    // -- CPU --
    let p = crate::ring0::cpu_vendor::profile::active();
    row("cpu", |l| { l.txt(p.vendor); l.txt(" "); l.txt(p.name); });
    row("uarch", |l| { l.txt(p.microarch); l.txt("  familia "); l.txt(p.family_model); });
    row("tsc", |l| {
        // Hz -> GHz con dos decimales, sin flotantes.
        l.dec(ctx.tsc_freq / 1_000_000_000);
        l.txt(".");
        let frac = (ctx.tsc_freq % 1_000_000_000) / 10_000_000;
        if frac < 10 { l.txt("0"); }
        l.dec(frac);
        l.txt(" GHz");
    });

    // -- MEMORIA: lo que hay, lo que se esta comiendo y en que --
    //
    // `used` es lo que el asignador de marcos NO tiene disponible: la imagen
    // del kernel, su bitmap, las pilas, las tablas de paginas, los buffers de
    // DMA y las regiones que el firmware declaro inutilizables. No se desglosa
    // mas porque el asignador no lo sabe, y un desglose inventado seria peor
    // que ninguno.
    let (total_frames, free_frames) = phys::stats();
    let total_b = total_frames * PAGE;
    let free_b = free_frames * PAGE;
    let used_b = total_b.saturating_sub(free_b);

    dashboard_log_color("== memoria ==", SH_TITLE);
    row("total", |l| { l.size(total_b); l.txt("   "); l.dec(total_frames); l.txt(" marcos de 4 KiB"); });
    row("usada", |l| { l.size(used_b); l.txt("   "); l.pct(used_b, total_b); l.txt("   "); l.dec(total_frames - free_frames); l.txt(" marcos"); });
    row("libre", |l| { l.size(free_b); l.txt("   "); l.pct(free_b, total_b); l.txt("   "); l.dec(free_frames); l.txt(" marcos"); });

    // El tamano REAL del kernel en RAM: desde donde lo linkea el script hasta
    // el final de su .bss (que incluye la pila de 64 KiB). Es un dato medido,
    // no el tamano del archivo.
    extern "C" { static __bss_end: u8; }
    let kernel_end = unsafe { &__bss_end as *const u8 as u64 };
    row("kernel", |l| { l.size(kernel_end.saturating_sub(0x400000)); l.txt("   en 0x400000"); });

    if crate::info::has_fb() {
        let (fw, fh, fs) = unsafe { (crate::info::FB_WIDTH as u64, crate::info::FB_HEIGHT as u64, crate::info::FB_STRIDE as u64) };
        row("video", |l| { l.size(fs * fh * 4); l.txt("   "); l.dec(fw); l.txt("x"); l.dec(fh); l.txt("x32  fb 0x"); l.hex(unsafe { crate::info::FB_ADDR }, 8); });
    }

    // -- ALMACENAMIENTO --
    {
        use crate::ring0::dev::disk;
        dashboard_log_color("== almacenamiento ==", SH_TITLE);
        if disk::is_ready() {
            row("disco", |l| { l.txt(disk::model()); l.txt("  puerto "); l.dec(disk::port() as u64); });
            row("serie", |l| { l.txt(disk::serial()); });
            row("tamano", |l| { l.size(disk::total_sectors() * 512); l.txt("   "); l.dec(disk::total_sectors()); l.txt(" sectores"); });
            row("escrit.", |l| { l.txt(if disk::write_armed() { "ARMADA" } else { "cerrada" }); });
        } else {
            row("disco", |l| { l.txt("sin disco listo"); });
        }
        let fs = crate::ring0::fsys::fs::fs_name();
        row("arranque", |l| { l.txt(fs); l.txt("  LBA "); l.dec(crate::ring0::fsys::fs::mounted_lba()); l.txt("  solo lectura"); });
        if crate::ring0::fsys::fs::data_mounted() {
            row("datos", |l| { l.txt("LBA "); l.dec(crate::ring0::fsys::fs::data_lba()); l.txt("  LECTURA/ESCRITURA"); });
        } else {
            row("datos", |l| { l.txt("sin montar"); });
        }
    }

    // -- PROCESOS Y ARRANQUE --
    dashboard_log_color("== sistema ==", SH_TITLE);
    let (tasks, runnable) = crate::ring0::task::scheduler::counts();
    row("tareas", |l| { l.dec(tasks as u64); l.txt(" totales   "); l.dec(runnable as u64); l.txt(" ejecutables"); });
    row("ticks", |l| { l.txt("0x"); l.hex(crate::ring0::plat::timer::ticks(), 8); });
    row("boot", |l| { l.txt("BootContext v"); l.dec(ctx.version as u64); l.txt("   "); l.dec(ctx.memory_map_count as u64); l.txt(" entradas de mapa"); });
    row("pml4", |l| { l.txt("0x"); l.hex(ctx.pml4, 8); l.txt("   rsdp 0x"); l.hex(ctx.rsdp, 8); });
}

fn shell_fb() {
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

fn shell_tasks() {
    let (total, runnable) = crate::ring0::task::scheduler::counts();
    crate::ring0::dev::console::serial_write("[tasks] total=");
    crate::ring0::dev::console::serial_write_u64(total as u64, 10);
    crate::ring0::dev::console::serial_write(" runnable=");
    crate::ring0::dev::console::serial_write_u64(runnable as u64, 10);
    crate::ring0::dev::console::serial_write(" current_tid=");
    crate::ring0::dev::console::serial_write_u64(
        crate::ring0::task::scheduler::current_tid() as u64,
        10,
    );
    crate::ring0::dev::console::serial_write(" ticks=");
    crate::ring0::dev::console::serial_write_u64(crate::ring0::plat::timer::ticks(), 10);
    crate::ring0::dev::console::serial_write("\n");
}

fn shell_splash() {
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

/// `bex` -- la tabla de programas que este kernel ha ejecutado.
///
/// El log cuenta la historia segun pasa y se la lleva el desplazamiento; esto
/// es la FOTO, consultable en cualquier momento: que se admitio, de que
/// tamano, donde entra, con que pid, como acabo y cuanto llego a escribir.
fn shell_bex() {
    let progs = crate::ring0::task::proc::programs();
    if progs.is_empty() {
        s_log("[bex] ningun programa admitido todavia");
        return;
    }
    s_log("== programas BEX (BEF1 x86-64) ==");
    s_log(" tag     imagen  secc  entry       pid tid  estado     lineas");
    // Formateo con columnas de ancho fijo: quietas se leen de un vistazo.
    fn txt(b: &mut [u8; 80], o: &mut usize, t: &str) {
        for &c in t.as_bytes() { if *o < b.len() { b[*o] = c; *o += 1; } }
    }
    fn pad(b: &mut [u8; 80], o: &mut usize, t: &str, width: usize) {
        let n = t.len().min(width);
        txt(b, o, &t[..n]);
        for _ in n..width { if *o < b.len() { b[*o] = b' '; *o += 1; } }
    }
    fn dec(b: &mut [u8; 80], o: &mut usize, mut v: u64, width: usize) {
        let mut tmp = [0u8; 20];
        let mut i = 0;
        if v == 0 { tmp[0] = b'0'; i = 1; }
        while v > 0 { tmp[i] = b'0' + (v % 10) as u8; v /= 10; i += 1; }
        for _ in i..width { if *o < b.len() { b[*o] = b' '; *o += 1; } }
        while i > 0 { i -= 1; if *o < b.len() { b[*o] = tmp[i]; *o += 1; } }
    }
    fn hex(b: &mut [u8; 80], o: &mut usize, v: u64, digits: usize) {
        const H: &[u8; 16] = b"0123456789ABCDEF";
        for i in (0..digits).rev() {
            if *o < b.len() { b[*o] = H[((v >> (i * 4)) & 0xF) as usize]; *o += 1; }
        }
    }

    for p in progs {
        let mut b = [0u8; 80];
        let mut o = 0usize;
        txt(&mut b, &mut o, " ");
        pad(&mut b, &mut o, p.tag, 7);
        dec(&mut b, &mut o, p.image_bytes as u64, 6);
        txt(&mut b, &mut o, "B ");
        dec(&mut b, &mut o, p.sections as u64, 4);
        txt(&mut b, &mut o, "  0x");
        hex(&mut b, &mut o, p.entry_va, 8);
        dec(&mut b, &mut o, p.pid as u64, 4);
        dec(&mut b, &mut o, p.tid as u64, 4);
        txt(&mut b, &mut o, "  ");
        // El estado sale del scheduler AHORA, no de lo que anotamos al
        // admitirlo: la tabla dice la verdad del momento en que se mira.
        let estado = if !p.admitted { "RECHAZADO" } else {
            match crate::ring0::task::scheduler::tid_state(p.tid) {
                0x01 => "listo    ",
                0x02 => "corriendo",
                0x03 => "bloqueado",
                0x04 => "saliendo ",
                _    => "terminado",
            }
        };
        txt(&mut b, &mut o, estado);
        dec(&mut b, &mut o, crate::ring0::uconsole::lines_of(p.pid) as u64, 7);
        if let Ok(s) = core::str::from_utf8(&b[..o]) { s_log(s); }
    }
}

/// F1 demo task: runs preempted by the timer, parks on a WAIT deadline,
/// wakes, and exits through the reaper. Watch the interleaving with the
/// shell prompt on serial -- that interleaving IS the context switch.
extern "C" fn ktest_main(arg: u64) -> ! {
    use crate::ring0::dev::console::{serial_write, serial_write_u64};
    serial_write("[ktest] start tid=");
    serial_write_u64(crate::ring0::task::scheduler::current_tid() as u64, 10);
    serial_write(" arg=");
    serial_write_u64(arg, 10);
    serial_write("\n");
    for i in 0..3u64 {
        serial_write("[ktest] window ");
        serial_write_u64(i, 10);
        serial_write("\n");
        // Busy window ~250 ms so the timer preempts us several times and
        // the shell task runs in between (look for the '>' echoes).
        let start = crate::ring0::task::scheduler::rdtsc();
        let span = crate::ring0::task::scheduler::tsc_freq() / 4;
        while crate::ring0::task::scheduler::rdtsc().wrapping_sub(start) < span {
            core::hint::spin_loop();
        }
    }
    serial_write("[ktest] park 2000 ms (WAIT deadline)\n");
    let deadline = crate::ring0::task::scheduler::rdtsc()
        + crate::ring0::task::scheduler::ns_to_tsc(2_000_000_000);
    crate::ring0::task::scheduler::park_until(deadline);
    serial_write("[ktest] woke; exit via reaper\n");
    crate::ring0::task::scheduler::exit_and_park();
}

fn shell_ktest() {
    match crate::ring0::task::scheduler::spawn_kernel(ktest_main as usize as u64, 0xB0, 1) {
        Some(tid) => {
            crate::ring0::dev::console::serial_write("[ktest] spawned tid=");
            crate::ring0::dev::console::serial_write_u64(tid as u64, 10);
            crate::ring0::dev::console::serial_write("\n");
        }
        None => s_log("[ktest] spawn failed (no frames or task slots)"),
    }
}

/// **Despierta los otros nucleos y cuenta cuantos contestan.**
///
/// [!] Es una orden a mano y no un paso del arranque **a proposito**. El
/// trampolin corre en modo real, antes de que exista nada, y no lo ha ejecutado
/// ningun CPU todavia. Si esta mal, lo que se cuelga es este comando y no la
/// maquina al encenderla; la salida es un reinicio a boton. Ver
/// `plat/smp.rs` y `docs/SMP_MAESTRO.md`.
fn shell_smp() {
    dashboard_log_color("== smp ==", SH_TITLE);

    // Por el PERFIL, no por el nombre del fabricante: ver `profile.rs`.
    if let Some(t) = (crate::ring0::cpu_vendor::profile::active().nucleos)() {
        row("silicio", |l| {
            l.dec(t.nucleos as u64);
            l.txt(" nucleos   ");
            l.dec(t.hilos as u64);
            l.txt(" hilos   ");
            l.dec(t.ccx as u64);
            l.txt(" CCX");
        });
    }

    row("   ", |l| l.txt("despertando... (si se queda aqui, reinicia a boton)"));

    // * Una linea POR NUCLEO y antes de mandarle nada. Es lo unico de este
    // comando que puede colgarse, y asi el cuelgue deja dicho en cual fue.
    // El censo se pinta ANTES de llamar a nadie: si algo cuelga, ya se sabe a
    // quien se iba a llamar y con que lista.
    if let Some(c) = crate::ring0::plat::madt::censo() {
        row("firmware", |l| {
            l.dec(c.ids().len() as u64);
            l.txt(" nucleos en la MADT");
            if c.apagados() > 0 {
                l.txt("   (+");
                l.dec(c.apagados() as u64);
                l.txt(" apagados)");
            }
        });
    } else {
        row("firmware", |l| l.txt("sin MADT: se supondran los APIC IDs"));
    }

    // El shell de Ring 0 despierta a TODOS: aqui no hay linea que escribir un
    // argumento, y quien llega a este shell es porque el escritorio no arranco.
    let (alive, esperados) = crate::ring0::plat::smp::despertar(u32::MAX, |id| {
        row("  ->", |l| {
            l.txt("APIC ");
            l.dec(id as u64);
        });
    });
    let (_, mascara) = crate::ring0::plat::smp::alive();

    // El BSP cuenta: es un nucleo que esta corriendo esto mismo.
    row("en pie", |l| {
        l.dec(alive as u64 + 1);
        l.txt(" / ");
        l.dec(esperados as u64 + 1);
        l.txt(" hilos");
    });
    // * Cuales, y no solo cuantos: "faltan dos" no dice a cual mirar.
    row("mascara", |l| {
        l.txt("APIC IDs que contestaron: ");
        l.hex(mascara as u64, 8);
    });
    if alive < esperados {
        row("   ", |l| l.txt("los que faltan no arrancaron o el trampolin no llego"));
    }
}

fn shell_mem() {
    let (total, free) = crate::ring0::mm::phys::stats();
    const PAGE: u64 = 4096;
    let total_b = total * PAGE;
    let free_b = free * PAGE;
    let used_b = total_b.saturating_sub(free_b);

    // Antes esto pintaba en el panel la linea "[mem] stats printed on serial",
    // que es la definicion de un comando inutil: te dice que la informacion
    // existe en un sitio donde no estas mirando.
    dashboard_log_color("== memoria ==", SH_TITLE);
    row("total", |l| { l.size(total_b); l.txt("   "); l.dec(total); l.txt(" marcos"); });
    row("usada", |l| { l.size(used_b); l.txt("   "); l.pct(used_b, total_b); });
    row("libre", |l| { l.size(free_b); l.txt("   "); l.pct(free_b, total_b); });

    if crate::ring0::mm::vmm::self_test() {
        s_log("[mem] vmm selftest OK (alloc/map/translate/unmap/destroy)");
    } else {
        s_log("[mem] vmm selftest FAILED");
    }
}

fn shell_panic() -> ! {
    s_log("[shell] triggering test panic...");
    panic!("intentional panic from serial shell");
}

fn shell_reboot() -> ! {
    // El pulso del 8042 a secas no reiniciaba nada en esta placa --su i8042
    // solo entrega ruido-- asi que el comando se quedaba colgado en el `hlt`
    // de despues. `reinicio::ahora` prueba 0xCF9, luego el 8042, y si nada
    // funciona provoca un triple fault, que no depende de ningun chipset.
    s_log("[shell] reboot");
    crate::ring0::plat::reinicio::ahora();
}

fn shell_halt() -> ! {
    s_log("[shell] halting");
    loop { unsafe { core::arch::asm!("sti; hlt"); } }
}

/// Donde vive el escritorio en el volumen de datos.
///
/// Es un CONTRATO, no una constante de conveniencia: quien quiera otro
/// escritorio deja su `.bex` ahi y arranca. Eso es exactamente lo que NO se
/// podia hacer mientras el compositor viajaba dentro del kernel.
///
/// * `gui` y no `compositor` porque **el nombre tiene que caber en 8.3**. El
/// driver FAT32 de `fs.rs` no lee nombres largos y se NIEGA a recortar: un
/// nombre recortado en silencio abre otro archivo, y en un cargador de
/// programas eso significa ejecutar otro binario. `compositor` son diez
/// caracteres; no cabia, y el fallo habria salido como `NameTooLong` despues
/// de copiarlo -- o sea, despues de creer que ya estaba.
///
/// `gui` ademas es el nombre que ya usa el crate (`bmo-service-gui`) y la
/// etiqueta con la que habla en CABINA. Un nombre, no tres.
const RUTA_COMPOSITOR: &str = "sys/gui.bex";

/// Arranca el escritorio desde el disco. Va DESPUES de montar el volumen de
/// datos -- antes no habria de donde leerlo.
///
/// * Si arranca, el panel del kernel deja de verse: al reclamar la pantalla el
/// kernel deja de dibujar, y el compositor no termina nunca (si terminara,
/// `revoke_all` devolveria la pantalla y el panel se repintaria encima). Los
/// logs siguen enteros por serie y en CABINA, y un fault de kernel recupera la
/// pantalla para contarlo.
///
/// * Si NO arranca, no pasa nada malo: queda el panel y el shell de Ring 0, que
/// es un sistema perfectamente usable. Por eso esto no se planta ni reintenta --
/// pero lo DICE, y dice que hacer. Un escritorio que no sale sin explicar por
/// que manda a alguien a leer codigo.
/// El tid del escritorio, para poder preguntar despues si sigue vivo.
/// `0` = no se admitio.
static mut ESCRITORIO_TID: u32 = 0;
/// * Y su PID, que **no es el mismo numero**. `vive()` pregunta por tid (el
/// hilo) y `uconsole` guarda las lineas por pid (el proceso). Confundirlos aqui
/// haria que el informe de defuncion leyera las ultimas palabras de otro -- o de
/// nadie, que es peor, porque pareceria que se murio callado.
static mut ESCRITORIO_PID: u32 = 0;

/// Se admitio el escritorio y ya no esta?
///
/// Admitir no es arrancar: el compositor puede morirse a los pocos ticks --y se
/// ha muerto-- dejando la maquina en el panel del kernel. Hasta ahora eso no lo
/// decia nadie y habia que deducirlo de que la ventana no salia.
fn escritorio_murio() -> bool {
    let tid = unsafe { ESCRITORIO_TID };
    tid != 0 && !crate::ring0::task::scheduler::vive(tid)
}

/// Cuantas veces se ha intentado levantar el escritorio.
static mut ESCRITORIO_INTENTOS: u32 = 0;
/// Tope de relanzamientos automaticos.
///
/// Dos y no infinitos: un compositor que muere por una condicion de carrera del
/// arranque se levanta al segundo intento, y uno que muere por un bug lo hara
/// las veces que se le pida. Reintentar sin tope convertiria un fallo visible
/// en una maquina que parpadea para siempre -- y encima borrando su propio log
/// en cada vuelta. Es la misma leccion que los puertos USB.
const ESCRITORIO_MAX_INTENTOS: u32 = 2;

fn arrancar_escritorio() {
    use crate::ring0::task::lanzar;

    unsafe { ESCRITORIO_INTENTOS += 1 };
    let inf = lanzar::ruta(RUTA_COMPOSITOR);
    match inf.res {
        Ok(tid) => {
            // * Arrancar y no decir su pid es un caso RARO, y por eso mismo
            // valia la pena distinguirlo: `unwrap_or(0)` lo dejaba en 0, que es
            // un pid con dueno. A partir de ahi, todo lo que se decida "para el
            // escritorio" mirando `ESCRITORIO_PID` apunta a otro.
            let pid = match inf.pid {
                Some(p) => p,
                None => {
                    crate::ring0::cabina::warn(
                        "gui", "el escritorio arranco y NO dijo su pid (queda 0)", tid as u64);
                    0
                }
            };
            unsafe {
                ESCRITORIO_TID = tid;
                ESCRITORIO_PID = pid;
            }
            row("escritorio", |l| {
                l.txt(RUTA_COMPOSITOR);
                l.txt("   tid ");
                l.dec(tid as u64);
                l.txt("  pid ");
                l.dec(pid as u64);
            });
            crate::ring0::cabina::info("gui", "escritorio admitido desde disco", tid as u64);
        }
        Err(f) => {
            row("escritorio", |l| { l.txt("NO ARRANCA: "); l.txt(f.motivo()); });
            row("   copia", |l| { l.txt(RUTA_COMPOSITOR); l.txt(" al volumen de datos"); });
            row("   o bien", |l| { l.txt("run "); l.txt(RUTA_COMPOSITOR); l.txt("   desde este shell"); });
            crate::ring0::cabina::warn("gui", f.motivo(), 0);
        }
    }
}

/// **El informe de defuncion del escritorio.**
///
/// * Antes esto decia *"mira el panico en el log de arriba"*. Y el panico SI
/// estaba: el manejador del compositor imprime archivo y linea exactos. Solo que
/// el log del kernel sigue corriendo, asi que para cuando se mira la pantalla
/// esa linea ya subio y salio -- tres arranques seguidos con la respuesta
/// delante y sin poder leerla.
///
/// Ahora se reimprimen **sus ultimas palabras**, guardadas por `uconsole`
/// mientras aun vivia. Un registrador de vuelo que borra la caja negra al
/// aterrizar no es un registrador de vuelo.
fn informe_de_defuncion() {
    let tid = unsafe { ESCRITORIO_TID };
    // * Por PID, no por tid. Ver la nota de `ESCRITORIO_PID`.
    let pid = unsafe { ESCRITORIO_PID };
    row("escritorio", |l| {
        l.txt("MURIO tras arrancar, tid ");
        l.dec(tid as u64);
        l.txt(" — esto es lo ULTIMO que dijo:");
    });
    if crate::ring0::uconsole::hubo_palabras(pid) {
        crate::ring0::uconsole::ultimas_palabras(pid, |linea| {
            row("   |", |l| { l.txt(linea); });
        });
    } else {
        // Que no dijera nada TAMBIEN es un dato: significa que se murio antes
        // de llegar a su primer mensaje, o que ni siquiera entro a CPL3.
        row("   |", |l| { l.txt("(nada: murio antes de decir una sola linea)"); });
    }
    row("   relanzar", |l| { l.txt("run "); l.txt(RUTA_COMPOSITOR); });
    crate::ring0::cabina::warn("gui", "el escritorio murio tras arrancar", tid as u64);
}

/// Espera a que los programas de ejemplo de Ring 3 terminen.
///
/// Con tope de tiempo: uno que se cuelgue no puede impedir que arranque el
/// escritorio. Y con `hlt` en el bucle -- girar en vacio aqui seria quitarle al
/// planificador el CPU que necesita justo para que esos programas avancen.
fn esperar_a_los_demos() {
    use crate::ring0::plat::timer;
    let limite = timer::ticks() + 400; // ~400 ms si el tick es de 1 ms
    loop {
        let (_total, listos) = crate::ring0::task::scheduler::counts();
        // 1 = solo queda la tarea del kernel. Los demos han acabado.
        if listos <= 1 {
            break;
        }
        if timer::ticks() > limite {
            crate::ring0::cabina::warn(
                "ring3", "los demos no acabaron a tiempo: se sigue igual", listos as u64);
            break;
        }
        unsafe { core::arch::asm!("hlt") };
    }
}

fn run_shell(ctx: &BootContext) -> ! {
    // Normalize the i8042 (translation -> Set 1, re-enable scanning) so the
    // physical keyboard reaches shell_read_line. No-op if the controller is
    // dead/absent (bounded timeouts inside). El stack USB real (xHCI+HID)
    // ya desperto en el Acto I de main().
    crate::ring0::dev::keyboard::init();
    // * Si el escritorio se admitio y ya no esta, DECIRLO aqui y decir que
    // hacer. Estar en este shell despues de haber lanzado el compositor no es
    // lo normal: significa que se murio, y quien lo mira solo ve un shell.
    if escritorio_murio() {
        informe_de_defuncion();
        // * Y se vuelve a intentar UNA vez. La entrada a Ring 3 es el estado
        // normal de esta maquina: quedarse en el shell de Ring 0 porque el
        // primer intento se cruzo con algo del arranque es conformarse.
        // Con tope, y diciendolo -- un relanzamiento silencioso convierte un
        // fallo en un misterio.
        if unsafe { ESCRITORIO_INTENTOS } < ESCRITORIO_MAX_INTENTOS {
            row("   reintento", |l| {
                l.txt("levantando el escritorio otra vez (");
                l.dec(unsafe { ESCRITORIO_INTENTOS } as u64 + 1);
                l.txt(" de ");
                l.dec(ESCRITORIO_MAX_INTENTOS as u64);
                l.txt(")");
            });
            crate::ring0::cabina::info("gui", "relanzando el escritorio", unsafe {
                ESCRITORIO_INTENTOS
            } as u64);
            arrancar_escritorio();
        } else {
            row("   basta", |l| {
                l.txt("no se relanza mas: dos veces es un bug, no una carrera");
            });
        }
    }
    dash_log("== BMO-X operativo : escribe help ==");
    // Serial-only banner: keep the rolling dashboard rows untouched so the
    // fixed-row Ring 3 diagnostics painted just before timer::enable survive.
    crate::ring0::dev::console::serial_write("\n=== BMO-X Ring 0 shell (type 'help') ===\n");
    shell_prompt();

    let mut buf = [0u8; 64];
    loop {
        let n = shell_read_line(&mut buf);
        if n == 0 { shell_prompt(); continue; }

        let cmd = &buf[..n];

        if cmd == b"help" {
            shell_help();
        } else if cmd == b"ls" {
            shell_ls();
        } else if cmd == b"disk" {
            shell_disk();
        } else if cmd == b"cabina" {
            shell_cabina();
        } else if cmd == b"estratos" {
            shell_estratos();
        } else if cmd == b"cpu" {
            shell_cpu();
        } else if cmd == b"hist" || cmd == b"history" {
            shell_hist();
        } else if cmd == b"layout" {
            shell_layout(b"");
        } else if cmd.len() > 7 && &cmd[..7] == b"layout " {
            shell_layout(&cmd[7..]);
        } else if cmd.len() > 4 && &cmd[..4] == b"run " {
            shell_run(&cmd[4..]);
        } else if cmd == b"run" {
            shell_run(b"");
        } else if cmd == b"cls" || cmd == b"clear" {
            clear_screen();
        } else if cmd == b"info" {
            shell_info(ctx);
        } else if cmd == b"smp" {
            shell_smp();
        } else if cmd == b"tasks" {
            shell_tasks();
        } else if cmd == b"mem" {
            shell_mem();
        } else if cmd == b"ktest" {
            shell_ktest();
        } else if cmd == b"fb" {
            shell_fb();
        } else if cmd == b"splash" {
            shell_splash();
        } else if cmd == b"bex" {
            shell_bex();
        } else if cmd == b"panic" {
            shell_panic();
        } else if cmd == b"reboot" {
            shell_reboot();
        } else if cmd == b"halt" {
            shell_halt();
        } else {
            s_log("unknown command (try 'help')");
        }
        shell_prompt();
    }
}

/// Public entry: called from `entry::kernel_main_real` after the
/// naked `_start` BSS zero.
pub fn main(ctx: &mut BootContext) {
    // Boot bisected cleanly on real hardware; the visual progress markers
    // are retired. `kbar!` is now a no-op so the call sites can stay as
    // documentation of the init order without painting over the UI. (Their
    // spirit lives on in the planned per-module status/version registry.)
    macro_rules! kbar { ($y:expr, $c:expr) => {{ let _ = ($y, $c); }}; }

    kbar!(90, 0xFF00_FF00u32); // green @90: past the magenta paint, before s_log
    s_log("[ring0] validating BootContext");
    // Guardar el `rsdp` -- solo un numero, no se lee ninguna tabla aqui. Lo
    // necesita el censo de la MADT, que lo pide mas tarde y desde sitios que no
    // tienen el `BootContext` delante (el manejador de syscall, por ejemplo).
    crate::ring0::plat::madt::recordar(ctx.rsdp);
    if !ctx.is_valid() {
        // Make an invalid BootContext VISIBLE (red @90) instead of a silent
        // halt -- otherwise a magic mismatch looks identical to a hang.
        kbar!(90, 0xFFFF_0000u32);
        s_log("[ring0] FATAL: BootContext magic mismatch");
        loop { unsafe { core::arch::asm!("hlt"); } }
    }
    kbar!(110, 0xFFFF_FFFFu32); // white @110: BootContext valid, before percpu

    crate::ring0::dev::console::serial_write("[ring0] BootContext OK, version=");
    crate::ring0::dev::console::serial_write_u64(ctx.version as u64, 10);
    crate::ring0::dev::console::serial_write("\n");
    // Primera entrada de la bitacora, ANTES de que exista framebuffer: CABINA
    // graba desde el minuto cero y lo muestra cuando haya pantalla. Si el
    // kernel muere entre aqui y el shell, el anillo ya tiene lo que paso.
    crate::ring0::cabina::info("ring0", "BootContext valido, kernel arrancando", ctx.version as u64);

    // Kernel-init checkpoints live in the empty band starting at row 140
    // (well below the boot bars that end near row 120), so any new bar is
    // unmistakably kernel progress -- not a repeat of an s1/s2 color.
    // * ANTES que nada que pueda atrapar. Los stubs de trap guardan el estado
    // extendido con XSAVE en un area de tamano FIJO, y el tamano que este CPU
    // necesita solo lo sabe el. Si no cabe, hay que enterarse AHORA y no
    // cuando el primer tick del timer desborde una pila de tarea.
    crate::ring0::cpu_vendor::xsave::init();
    crate::ring0::task::percpu::init_bsp();
    kbar!(140, 0xFF00_FF00u32); // green: percpu OK
    crate::ring0::task::scheduler::init(ctx.tsc_freq);
    kbar!(152, 0xFFFF_FFFFu32); // white: scheduler OK
    crate::ring0::mm::phys::init(ctx);
    kbar!(164, 0xFFFF_0000u32); // red: phys::init OK
    crate::ring0::mm::vmm::init();
    kbar!(176, 0xFF00_FFFFu32); // aqua: vmm::init OK
    let (frames_total, frames_free) = crate::ring0::mm::phys::stats();
    crate::ring0::dev::console::serial_write("[ring0] mm ready: frames free=");
    crate::ring0::dev::console::serial_write_u64(frames_free, 10);
    crate::ring0::dev::console::serial_write("/");
    crate::ring0::dev::console::serial_write_u64(frames_total, 10);
    crate::ring0::dev::console::serial_write("\n");
    crate::ring0::cabina::info("mem", "physmap + asignador de frames listos", frames_free);
    crate::ring0::obj::channel::init(ctx);
    crate::ring0::svc::register_all();
    crate::ring0::syscall::init();
    // Arm the on-screen fault reporter before anything can enter Ring 3, so a
    // CPL3 crash paints its vector/RIP/CR2 instead of the silent serial-halt
    // the boot stage installs.
    crate::ring0::plat::faults::init(ctx);
    let timer_ready = crate::ring0::plat::timer::init(ctx);
    if timer_ready {
        s_log("[ring0] scheduler + BMO Channel + SYSCALL + LAPIC tick ready (BSP)");
        crate::ring0::cabina::info("ring0", "scheduler + canal + syscalls + LAPIC armados", 0);
    } else {
        s_log("[ring0] WARNING: LAPIC tick unavailable; scheduler remains cooperative");
        crate::ring0::cabina::warn("ring0", "sin tick LAPIC: scheduler solo cooperativo", 0);
    }
    kbar!(188, 0xFFFF_FF00u32); // yellow: channel/svc/syscall/timer OK

    // Populate the active BMO CPU profile (today: Ryzen 5 5600X).
    // Identity, SMT/CCX topology, cache hierarchy, TSC calibration and
    // errata/speculation mitigations all live behind the profile --
    // changing CPU or vendor is a profile swap, never a kernel edit.
    let cpu_profile = crate::ring0::cpu_vendor::active();
    (cpu_profile.init)();
    kbar!(200, 0xFFFF_8800u32); // orange: CPU profile + errata (MSR) OK
    crate::ring0::dev::console::serial_write("[cpu] profile: ");
    crate::ring0::dev::console::serial_write(cpu_profile.vendor);
    crate::ring0::dev::console::serial_write(" ");
    crate::ring0::dev::console::serial_write(cpu_profile.name);
    crate::ring0::dev::console::serial_write("\n");
    crate::ring0::cabina::info("cpu", cpu_profile.name, 0);

    // BEX is the only native executable contract admitted by this kernel.
    // The parser is allocation-free so it is safe before the process allocator.
    crate::ring0::task::bex::announce();
    kbar!(212, 0xFF00_FFFFu32); // aqua: bex announce OK, before proc::spawn_init

    // F2: if the boot chain reserved a Ring 3 payload, admit it as the
    // init process. With no payload this is a no-op and the boot flow is
    // exactly the Ring 0 shell as before.
    crate::ring0::task::proc::init(ctx);
    let ring3_tid = crate::ring0::task::proc::spawn_init(ctx);
    if let Some(tid) = ring3_tid {
        crate::ring0::dev::console::serial_write("[ring0] Ring 3 init task ready, tid=");
        crate::ring0::dev::console::serial_write_u64(tid as u64, 10);
        crate::ring0::dev::console::serial_write("\n");
        crate::ring0::cabina::info("ring3", "proceso init admitido", tid as u64);
    } else {
        crate::ring0::cabina::warn("ring3", crate::ring0::task::proc::init_status(), 0);
    }
    kbar!(224, 0xFFFF_FFFFu32); // white: proc init + Ring 3 spawn OK, before splash

    // CPU identity detection (CPUID leaf 0, 1, 0x80000002-04)
    let cpu = crate::ring0::cpu::detect_cpu();
    let cpu_line = match cpu.vendor {
        crate::ring0::cpu::CpuVendor::Amd => "AMD",
        crate::ring0::cpu::CpuVendor::Intel => "Intel",
        crate::ring0::cpu::CpuVendor::Unknown => "Unknown",
    };
    let brand = cpu.brand.as_str();
    // Use a stack buffer to build the log line, then emit to both
    // serial and the framebuffer dashboard.
    let mut line = [0u8; 96];
    let prefix = b"[cpu] ";
    let mid1   = b" | ";
    let mid2   = b" | cores=";
    let mut off = 0;
    for &b in prefix { line[off] = b; off += 1; }
    for &b in brand.as_bytes() { if off < line.len() { line[off] = b; off += 1; } }
    for &b in mid1 { if off < line.len() { line[off] = b; off += 1; } }
    for &b in cpu_line.as_bytes() { if off < line.len() { line[off] = b; off += 1; } }
    for &b in mid2 { if off < line.len() { line[off] = b; off += 1; } }
    if off < line.len() { line[off] = b'0' + (cpu.logical_cores as u8 / 10); off += 1; }
    if off < line.len() { line[off] = b'0' + (cpu.logical_cores as u8 % 10); off += 1; }
    if let Ok(s) = core::str::from_utf8(&line[..off]) {
        s_log(s);
    }

    // Populate FB globals from the context, then bring up the fb driver.
    crate::info::init_from(ctx);
    phase0_fb(ctx);

    // -- Intro cinematica (logo -> preparando -> RING 0 -> RING 3) ----------
    // Escenas centradas con fundido y transicion, al estilo de un arranque
    // moderno. Al terminar aterrizamos en el dashboard, donde el trabajo
    // REAL de cada etapa fluye como log (igual que Windows: la animacion
    // juega, luego apareces en el escritorio).
    if crate::info::has_fb() {
        splash::boot_intro();
    } else {
        s_log("[splash] no framebuffer, skipping splash");
    }

    // Aterrizar en el dashboard persistente.
    phase1_ui(ctx);

    // -- Acto I: RING 0 despierta el hardware (log real) -----------------
    // Los encabezados "==" se pintan en cyan (dash_line_color).
    dash_log("== RING 0 : despertando hardware ==");
    s_log("[ring0] cpu Zen 3 perfilado + GDT/IDT propias");
    {
        let (total, free) = crate::ring0::mm::phys::stats();
        let mut b = [0u8; 48];
        let mut o = 0;
        for &c in b"[ring0] mem ".iter() { if o < b.len() { b[o] = c; o += 1; } }
        let gib = (total * 4096) >> 30;
        let mut tmp = [0u8; 4];
        let mut t = 0;
        let mut v = gib.max(1);
        while v > 0 && t < 4 { tmp[t] = b'0' + (v % 10) as u8; v /= 10; t += 1; }
        while t > 0 { t -= 1; if o < b.len() { b[o] = tmp[t]; o += 1; } }
        for &c in b" GiB physmap listos".iter() { if o < b.len() { b[o] = c; o += 1; } }
        let _ = free;
        if let Ok(s) = core::str::from_utf8(&b[..o]) { s_log(s); }
    }
    s_log("[ring0] scheduler preemptivo + capabilities armados");
    // CABINA abre los ojos y censa el almacenamiento (scan PCI). Va AQUI, en el
    // acto donde el kernel despierta hardware -- antes vivia dentro del render y
    // clavaba ~65k lecturas de config PCI en el primer frame del cockpit.
    crate::ring0::cabina::boot_probe();
    // USB en su lugar narrativo: el kernel despierta teclado y mouse AQUI.
    crate::ring0::dev::usb::init(ctx);
    // Y el disco: el HBA SATA (no el NVMe -- ahi vive el sistema del dueno) y
    // su tabla de particiones. Ver dev/disk.rs.
    crate::ring0::dev::disk::init();
    crate::ring0::dev::disk::scan_partitions();
    // Y el sistema de ficheros: de sectores a ARCHIVOS. Monta la particion de
    // arranque, que es donde vive el BOOTX64.EFI con el que arrancamos.
    crate::ring0::fsys::fs::mount();
    // El gate: el disco tiene que decir QUIEN ES antes de que se le pueda
    // escribir. Va DESPUES de leer la GPT porque una de las pruebas es que la
    // tabla cuadre con los sectores que el propio disco declara.
    crate::ring0::dev::disk::verify_identity();
    // Y si convencio, el volumen de datos se monta con escritor. La particion
    // de arranque sigue montada sin el, y asi se queda.
    crate::ring0::fsys::fs::mount_data();
    // Y ESTRATOS, si alguna particion lleva uno. Solo lectura: el modulo no
    // sabe escribir, asi que montarlo no puede estropear nada.
    crate::ring0::fsys::estratos::mount();
    dash_log("== RING 0 : hardware al mando ==");

    // -- Acto II: RING 3 -- el userspace nace -----------------------------
    dash_log("== RING 3 : userspace ==");
    // Surface the Ring 3 init outcome on the (now cleared) dashboard so the
    // demo's state is visible without serial. If a tid was admitted, the next
    // timer tick will enter CPL3 and its 'ring3>' lines should follow below.
    {
        let mut summary = [0u8; 64];
        let head = b"[ring3] ";
        let status = crate::ring0::task::proc::init_status();
        let mut off = 0;
        for &b in head { if off < summary.len() { summary[off] = b; off += 1; } }
        for &b in status.as_bytes() { if off < summary.len() { summary[off] = b; off += 1; } }
        if let Some(tid) = ring3_tid {
            for &b in b" tid=" { if off < summary.len() { summary[off] = b; off += 1; } }
            if off < summary.len() { summary[off] = b'0' + (tid as u8 % 10); off += 1; }
        }
        if let Ok(s) = core::str::from_utf8(&summary[..off]) { s_log(s); }
    }

    // Y el escritorio, que ya NO viaja dentro del kernel. Va aqui y no en
    // `spawn_init` por una razon dura: `spawn_init` corre en el Acto I, cuando
    // el HBA SATA ni se ha tocado. Este es el primer punto del arranque en el
    // que existe un volumen de datos del que leerlo.
    //
    // * Y se ANUNCIA. El paso de Ring 0 a Ring 3 era invisible: el kernel
    // dejaba de pintar y o aparecia un escritorio o no aparecia nada, sin
    // forma de saber cual de los dos lados habia fallado. Decir que se cede y
    // a quien convierte ese silencio en un acto con testigos.
    // * PRIMERO que acaben los demos, LUEGO la entrega.
    //
    // Los demos de Ring 3 y el escritorio se admitian todos antes de encender
    // el timer, asi que arrancaban **a la vez** -- y `init_hello` reclama la
    // pantalla para demostrar que Ring 3 puede. Ganaba el, pintaba sus tres
    // lineas, terminaba, y al morir el kernel recuperaba la pantalla y
    // repintaba su panel... encima del escritorio que acababa de nacer.
    //
    // De ahi las dos cosas que se veian y nadie explicaba: el aviso de "el
    // dueno de la pantalla MURIO" en cada arranque (era el demo, no el
    // compositor) y el panel del kernel dibujado sobre la ventana.
    //
    // Los demos ya demostraron lo suyo. Ahora se les deja terminar antes de
    // entregar la pantalla, con tope: si uno se cuelga, el escritorio arranca
    // igual -- esperar para siempre a un programa de ejemplo seria cambiar un
    // arranque feo por uno que no llega.
    if timer_ready {
        crate::ring0::plat::timer::enable();
        esperar_a_los_demos();
    }

    dash_log("== RING 3 : LA ENTREGA ==");
    row("se cede", |l| {
        l.txt("la PANTALLA, la ENTRADA y una CONSOLA — y Ring 0 deja de pintar");
    });
    row("a", |l| { l.txt(RUTA_COMPOSITOR); l.txt("   desde el volumen de datos, con su firma"); });
    arrancar_escritorio();

    // FINAL checkpoint: bright green at row 236 = kernel finished ALL of
    // phase::main and is entering the shell. If this shows, Ring 0 fully
    // booted on real hardware.
    kbar!(236, 0xFF00_FF00u32);

    run_shell(ctx);
}
