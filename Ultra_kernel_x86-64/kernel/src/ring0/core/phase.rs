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
/// CABINA. Sin este reparto los dos escribían en las mismas filas y se
/// borraban mutuamente (el log se comía la bitácora y viceversa).
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
/// reconoce. Un informe del shell no tiene emisor: tiene estructura — títulos,
/// etiquetas y valores — y quien la conoce es quien lo escribe.
pub fn dashboard_log_color(msg: &str, color: u32) {
    dashboard_log_impl(msg, Some(color));
}

fn dashboard_log_impl(msg: &str, color: Option<u32>) {
    if !crate::info::has_fb() { return; }
    let rows = log_rows();
    if rows == 0 { return; }

    // ── Líneas repetidas: se cuentan, no se apilan ──────────────────────────
    //
    // El censo de puertos del AHCI escupe una línea por puerto y la mayoría
    // son idénticas: catorce `p0x0 ssts=0x0` seguidas se comían medio panel y
    // barrían el arranque entero fuera de la pantalla. Y el panel es la única
    // ventana que hay — aquí no se puede hacer scroll hacia atrás.
    //
    // Una repetición NO es información nueva; el número de veces SÍ. Así que
    // la fila se queda donde está y se le añade el contador. Catorce líneas
    // pasan a ser una que dice `x14`, y las trece filas que ganamos son trece
    // hechos distintos que antes no cabían.
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
            // cambia, así que el color de la línea sigue siendo el suyo.
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

// ── Constructor de líneas del shell ─────────────────────────────────────────
//
// Cada comando del shell traía sus propias closures `txt`/`dec` copiadas.
// Esto es una sola, con lo que hace falta para alinear columnas y para decir
// un tamaño en la unidad que se entiende.

/// Colores de los informes del shell. Etiqueta apagada, valor claro, título
/// ámbar: la misma jerarquía en todos los comandos, para que `info` y `disk`
/// se lean como partes del mismo sistema y no como dos programas distintos.
const SH_TITLE: u32 = 0xFFF6C445; // ámbar
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
    /// etiquetas alineadas sin contar caracteres a mano en cada línea.
    fn col(&mut self, col: usize) {
        while self.o < col && self.o < self.b.len() { self.b[self.o] = b' '; self.o += 1; }
    }
    /// Un tamaño en la unidad que se entiende, con dos decimales.
    ///
    /// Sin coma flotante: la parte fraccionaria se saca multiplicando el resto
    /// por 100 antes de dividir. En Ring 0 no hay `f64` que valga — y aunque
    /// lo hubiera, el estado SSE de la tarea no se preserva todavía.
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
    // salía "particion3" y "generacion1", que se leen como una sola palabra.
    l.txt(" ");
    l.col(12);
    build(&mut l);
    // La etiqueta va en cian y el valor en blanco, pero el panel pinta una
    // línea de un solo color: se elige el del VALOR, que es lo que se lee.
    dashboard_log_color(l.as_str(), SH_VALUE);
    crate::ring0::dev::console::serial_write(l.as_str());
    crate::ring0::dev::console::serial_write("\n");
}

// Mirror the current in-progress shell line to the framebuffer's prompt area,
// con CURSOR PARPADEANTE. Antes se repintaba en CADA iteración del loop del
// shell (limpiar+dibujar sin cambio) → ese era el ghosting ocasional del
// prompt. Ahora solo repinta cuando: cambia la línea, parpadea el cursor, o
// hubo un clear. Pantalla estable + cursor vivo.
fn dash_prompt(line: &str, cursor: usize) {
    if !crate::info::has_fb() { return; }
    let ticks = crate::ring0::timer::ticks();
    let blink = ((ticks >> 6) & 1) == 0; // visible ~mitad del tiempo
    let n = line.len();
    static mut LAST_N: usize = usize::MAX;
    static mut LAST_CUR: usize = usize::MAX;
    static mut LAST_BLINK: bool = false;
    static mut LAST_GEN: u32 = u32::MAX;
    static mut LAST_LOCKS: u8 = 0xFF;
    // El estado de los bloqueos entra en la firma: al pulsar Bloq Mayús la
    // línea no cambia, pero el indicador sí — y hay que repintarlo.
    let (caps, num) = crate::ring0::dev::keyboard::lock_state();
    let locks = (caps as u8) | ((num as u8) << 1);
    unsafe {
        let gen = SCREEN_GEN;
        if LAST_N == n && LAST_CUR == cursor && LAST_BLINK == blink
            && LAST_GEN == gen && LAST_LOCKS == locks { return; }
        LAST_N = n; LAST_CUR = cursor; LAST_BLINK = blink; LAST_GEN = gen; LAST_LOCKS = locks;
    }
    splash::splash_dashboard_prompt(line, cursor, blink);
    // Indicadores a la derecha de la barra: distribución activa y bloqueos.
    // Los LEDs físicos de este teclado no responden; la pantalla nunca miente.
    splash::splash_status_right(crate::ring0::dev::keyboard::layout_name(), caps, num);
}

fn shell_prompt() {
    crate::ring0::dev::console::serial_write("> ");
    dash_prompt("", 0);
}

/// Generación de pantalla: se incrementa en cada limpieza. Los paneles de fila
/// FIJA (heartbeat, usb) la comparan para FORZAR un repintado tras un clear,
/// aunque sus valores no hayan cambiado — si no, la detección de cambios los
/// dejaría en blanco para siempre después de limpiar (bug real observado).
static mut SCREEN_GEN: u32 = 0;

/// Generación de pantalla actual — CABINA la usa para repintar su cockpit tras
/// un clear (mismo mecanismo anti-ghosting que los paneles fijos).
pub(crate) fn screen_gen() -> u32 { unsafe { SCREEN_GEN } }

/// Limpia la pantalla y re-dibuja el dashboard vacío (comando `cls` y
/// auto-limpieza al terminar un proceso). Reinicia el cursor rodante del log
/// para que el panel arranque de cero, como una terminal recién abierta.
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
// (usb, fila 12) VIVÍAN aquí. CABINA los absorbió: su banda inferior muestra la
// misma telemetría (ticks/switches/estado de Ring 3/USB detallado) en una vista
// coherente y con color semántico, y además con la bitácora de eventos que
// aquellos no tenían. Ya nadie los llamaba — código muerto pintando filas que
// el log rodante volvía a borrar. Se eliminan: CABINA es el único observador.

/// Último total de tareas visto por el shell, para detectar cuándo un proceso
/// TERMINÓ (el total baja) y limpiar la pantalla automáticamente.
static mut LAST_TASK_TOTAL: usize = 0;

// ── Historial de comandos ───────────────────────────────────────────────────
//
// Un anillo de las últimas líneas ejecutadas, recorrible con las flechas
// arriba/abajo. Sin esto, repetir un comando es volver a teclearlo entero.

const HIST_MAX: usize = 16;
const HIST_LINE: usize = 64;
static mut HIST: [[u8; HIST_LINE]; HIST_MAX] = [[0; HIST_LINE]; HIST_MAX];
static mut HIST_LEN: [usize; HIST_MAX] = [0; HIST_MAX];
static mut HIST_COUNT: usize = 0; // cuántas líneas hay (tope HIST_MAX)
static mut HIST_HEAD: usize = 0;  // dónde se escribe la siguiente

/// Guarda una línea en el historial. No repite la inmediatamente anterior:
/// pulsar Enter tres veces sobre el mismo comando no debería llenar el
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

/// Entrada `back` posiciones hacia atrás (1 = la última ejecutada).
fn hist_get(back: usize) -> Option<&'static [u8]> {
    unsafe {
        if back == 0 || back > HIST_COUNT { return None; }
        let idx = (HIST_HEAD + HIST_MAX - back) % HIST_MAX;
        Some(&HIST[idx][..HIST_LEN[idx]])
    }
}

/// Lee una línea del teclado con edición completa: cursor, historial y los
/// atajos de Ctrl de toda la vida.
///
/// Devuelve `(largo, cancelada)`; cancelada = el usuario pulsó Ctrl+C.
fn shell_read_line(buf: &mut [u8]) -> usize {
    use crate::ring0::dev::keyboard as kb;
    let mut n = 0;     // largo de la línea
    let mut cur = 0;   // posición del cursor dentro de la línea
    let mut hist_at = 0; // 0 = línea nueva; >0 = navegando el historial

    // Inserta un byte en la posición del cursor, desplazando lo que haya.
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
        // Auto-limpieza: si un proceso terminó (el total de tareas bajó) y NO
        // estás escribiendo (línea vacía), limpia la pantalla — como una
        // terminal que se refresca al acabar el programa. Nunca borra a media
        // escritura (solo con n==0).
        let (total, _) = crate::ring0::scheduler::counts();
        unsafe {
            if n == 0 && total < LAST_TASK_TOTAL {
                clear_screen();
                dash_log("== proceso terminado : pantalla limpia ==");
            }
            LAST_TASK_TOTAL = total;
        }
        dash_prompt(core::str::from_utf8(&buf[..n]).unwrap_or(""), cur);
        // CABINA — cockpit omnisciente en la banda inferior.
        crate::ring0::cabina::render_hud();

        // Entrada: serial (COM1), teclado USB o PS/2, lo que tenga un byte.
        let mut byte = crate::ring0::dev::console::serial_read_byte();
        if byte.is_none() {
            byte = crate::ring0::dev::usb::poll_ascii();
        }
        if byte.is_none() {
            // PS/2 i8042 (mudo post-EBS en esta placa). Se conserva por si
            // algún día reviviera (adaptador PS/2, otra placa).
            if let Some((_raw, ascii)) = kb::poll_event() {
                byte = ascii;
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
                // Hacia atrás en el historial.
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
                    // Se acabó el historial: vuelta a la línea en blanco.
                    hist_at = 0;
                    n = 0;
                    cur = 0;
                }
            }
            0x01 => { cur = 0; }              // Ctrl+A: al principio
            0x05 => { cur = n; }              // Ctrl+E: al final
            0x03 => {                          // Ctrl+C: cancelar la línea
                crate::ring0::dev::console::serial_write("^C\n");
                return 0;
            }
            0x0C => { clear_screen(); }        // Ctrl+L: limpiar pantalla
            0x15 => { n = 0; cur = 0; }        // Ctrl+U: borrar la línea entera
            0x0B => { n = cur; }               // Ctrl+K: borrar hasta el final
            0x17 => {                          // Ctrl+W: borrar la palabra
                while cur > 0 && buf[cur - 1] == b' ' { erase(buf, &mut n, &mut cur); }
                while cur > 0 && buf[cur - 1] != b' ' { erase(buf, &mut n, &mut cur); }
            }
            // Imprimible = ASCII visible O byte Latin-1 alto (ñ, á, ¿, ...).
            // El teclado español entrega un byte por carácter y el font sabe
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
    // Por categorías y con las columnas alineadas por el mismo constructor que
    // usan `info` y `disk`: antes cada comando alineaba a ojo con espacios
    // contados a mano, y bastaba una palabra más larga para torcer la columna.
    dashboard_log_color("== BMO-X shell ==", SH_TITLE);
    row("sistema", |l| l.txt("info  mem  tasks  disk  ls  estratos  cabina  hist"));
    row("edicion", |l| l.txt("flechas  Inicio/Fin  Supr  ^A ^E ^U ^K ^W ^C ^L"));
    row("video", |l| l.txt("fb  splash  cls"));
    row("ring3", |l| l.txt("run <ruta>  bex  ktest"));
    row("poder", |l| l.txt("reboot  halt  panic"));
    row("ayuda", |l| l.txt("help"));
}

/// `disk` — qué disco tiene BMO delante y qué hay en él.
///
/// La tabla de particiones es cómo el kernel RECONOCE su disco: no se fía del
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

    // Quién es el disco, según él mismo. Con tres discos en la máquina y el
    // sistema del dueño en uno de ellos, esta línea es la que autoriza (o no)
    // a escribir algún día.
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
        // Sectores de 512 B → GiB: >>21 es dividir entre 2 Mi sectores.
        dec(&mut b, &mut o, p.sectors() >> 21, 9);
        txt(&mut b, &mut o, "  ");
        let tipo = if p.is_esp() { "ESP/boot " }
                   else if p.is_basic_data() { "datos    " }
                   else { "otro     " };
        txt(&mut b, &mut o, tipo);
        txt(&mut b, &mut o, p.name_str());
        if let Ok(s) = core::str::from_utf8(&b[..o]) { s_log(s); }
    }
    // El veredicto del gate, en palabras. Es la línea que decide si este disco
    // se puede escribir, así que se pinta siempre — diga que sí o que no.
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

/// `cabina` — vuelca la bitácora de vuelo a disco.
///
/// Es el punto donde todo lo demás cobra sentido: hasta ahora CABINA lo veía
/// todo y lo olvidaba al apagar. Un registrador que solo existe mientras vuela
/// el avión no sirve para investigar la caída.
fn shell_cabina() {
    use crate::ring0::fs;
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

/// `ls` — recorre `EFI\BOOT\BOOTX64.EFI` de la partición de arranque y lo lee.
///
/// Lo que esto demuestra: que el camino entero —AHCI, GPT, FAT32, directorios,
/// cadena de clústeres— funciona de punta a punta contra un archivo real.
///
/// Lo que NO demuestra: que ese archivo sea el nuestro. La versión anterior
/// remataba con "es un ejecutable UEFI: SOY YO" a partir de la firma `MZ`, que
/// la lleva CUALQUIER ejecutable de Windows. En este disco la partición de
/// arranque es la ESP de 0,6 GB que comparte con el sistema del dueño, así que
/// bien puede ser su cargador. Se dice lo que se sabe.
fn shell_ls() {
    use crate::ring0::fs;
    if !fs::is_mounted() {
        s_log("[fs] no hay volumen montado (mira la bitacora de CABINA)");
        return;
    }

    dashboard_log_color("== volumen de arranque ==", SH_TITLE);
    row("formato", |l| { l.txt(fs::fs_name()); l.txt("  LBA "); l.dec(fs::mounted_lba()); l.txt("  solo lectura"); });

    // Los nombres van en 8.3 crudo: 8 de nombre + 3 de extensión, con
    // espacios de relleno. Feo, pero es como FAT los guarda en disco.
    let efi = match fs::find_dir(b"EFI        ") {
        Some(c) => c,
        None => { s_log("[fs] no encuentro el directorio EFI"); return; }
    };
    let boot = match fs::find_dir_in(b"BOOT       ", efi) {
        Some(c) => c,
        None => { s_log("[fs] no encuentro EFI\\BOOT"); return; }
    };
    // ★ Y buscar el archivo DENTRO de `boot`, no en la raiz. El primer
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

/// `estratos` — el estado del volumen propio y su raíz.
///
/// Es la primera vez que BMO-X lee un sistema de ficheros **suyo**: FAT32 es
/// un formato prestado que había que entender; ESTRATOS lo escribió él.
fn shell_estratos() {
    use crate::ring0::estratos as est;
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
    // El gate del diseño: si el volumen no nació aquí, se dice EN ALTO. Hoy
    // solo se lee, pero el día que se escriba esta línea es la que decide.
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

/// `run <ruta>` — carga un `.bex` del disco y lo ejecuta.
///
/// Es el punto donde el trabajo del disco cobra sentido. Hasta ahora los
/// programas Ring 3 vivían DENTRO del kernel (`include_bytes!`): cambiar un
/// "hola mundo" obligaba a recompilar el sistema operativo entero y
/// reflashear. Ahora se copia el `.bex` a la partición desde el anfitrión y se
/// escribe `run apps/hola.bex`.
///
/// El buffer es estático y no local: un `.bex` son varios KiB y la pila del
/// kernel son 64 KiB para todo.
fn shell_run(arg: &[u8]) {
    const MAX_BEX: usize = 64 * 1024;
    static mut IMAGE: [u8; MAX_BEX] = [0u8; MAX_BEX];

    let path = match core::str::from_utf8(arg) {
        Ok(s) => s.trim(),
        Err(_) => { s_log("[run] la ruta tiene bytes que no son texto"); return; }
    };
    if path.is_empty() {
        s_log("[run] uso: run apps/hola.bex   (o A:/apps/hola.bex)");
        return;
    }
    if !crate::ring0::proc::has_room() {
        s_log("[run] no quedan huecos de proceso");
        return;
    }

    let buf = unsafe { &mut *core::ptr::addr_of_mut!(IMAGE) };
    let n = match crate::ring0::fs::load(path, buf) {
        Ok(n) => n,
        Err(e) => {
            // El motivo exacto: "no esta" y "no cabe en 8.3" mandan a hacer
            // cosas distintas, y un "no se pudo" no manda a ninguna.
            let mut l = L::new();
            l.txt("[run] ");
            l.txt(e.name());
            l.txt(": ");
            l.txt(path);
            dashboard_log_color(l.as_str(), SH_TITLE);
            crate::ring0::dev::console::serial_write(l.as_str());
            crate::ring0::dev::console::serial_write("\n");
            return;
        }
    };

    dashboard_log_color("== run ==", SH_TITLE);
    row("archivo", |l| { l.txt(path); });
    row("leido", |l| { l.size(n as u64); });

    // El nombre del proceso es el último componente de la ruta: es lo que el
    // usuario reconoce en el log, no la ruta entera.
    let name = {
        let b = path.as_bytes();
        match b.iter().rposition(|&c| c == b'/' || c == b'\\') {
            Some(i) => &path[i + 1..],
            None => path,
        }
    };

    match crate::ring0::proc::admit_from_disk(name, &buf[..n]) {
        Some(tid) => {
            row("admitido", |l| { l.txt("tid "); l.dec(tid as u64); l.txt("   corre en el siguiente tick"); });
        }
        None => {
            row("rechazado", |l| { l.txt("el .bex no paso la admision (mira CABINA)"); });
        }
    }
}

/// `hist` — la lista de comandos ejecutados, numerada. Lo mismo que recorren
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

/// `layout` — muestra o cambia la distribución del teclado EN CALIENTE.
/// El scancode dice qué tecla se pulsó, no qué letra es; si lo que sale no
/// coincide con lo impreso en tus teclas, prueba otra aquí mismo.
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

/// `info` — el informe completo de la máquina.
///
/// Antes escribía TODO al puerto serie y al panel no llegaba nada: en una
/// máquina sin cable serie el comando parecía no hacer nada. Ahora cada línea
/// va a los dos sitios.
fn shell_info(ctx: &BootContext) {
    use crate::ring0::mm::phys;
    const PAGE: u64 = 4096;

    dashboard_log_color("== BMO-X : informe del sistema ==", SH_TITLE);

    // ── CPU ──
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

    // ── MEMORIA: lo que hay, lo que se está comiendo y en qué ──
    //
    // `used` es lo que el asignador de marcos NO tiene disponible: la imagen
    // del kernel, su bitmap, las pilas, las tablas de páginas, los buffers de
    // DMA y las regiones que el firmware declaró inutilizables. No se desglosa
    // más porque el asignador no lo sabe, y un desglose inventado sería peor
    // que ninguno.
    let (total_frames, free_frames) = phys::stats();
    let total_b = total_frames * PAGE;
    let free_b = free_frames * PAGE;
    let used_b = total_b.saturating_sub(free_b);

    dashboard_log_color("== memoria ==", SH_TITLE);
    row("total", |l| { l.size(total_b); l.txt("   "); l.dec(total_frames); l.txt(" marcos de 4 KiB"); });
    row("usada", |l| { l.size(used_b); l.txt("   "); l.pct(used_b, total_b); l.txt("   "); l.dec(total_frames - free_frames); l.txt(" marcos"); });
    row("libre", |l| { l.size(free_b); l.txt("   "); l.pct(free_b, total_b); l.txt("   "); l.dec(free_frames); l.txt(" marcos"); });

    // El tamaño REAL del kernel en RAM: desde donde lo linkea el script hasta
    // el final de su .bss (que incluye la pila de 64 KiB). Es un dato medido,
    // no el tamaño del archivo.
    extern "C" { static __bss_end: u8; }
    let kernel_end = unsafe { &__bss_end as *const u8 as u64 };
    row("kernel", |l| { l.size(kernel_end.saturating_sub(0x400000)); l.txt("   en 0x400000"); });

    if crate::info::has_fb() {
        let (fw, fh, fs) = unsafe { (crate::info::FB_WIDTH as u64, crate::info::FB_HEIGHT as u64, crate::info::FB_STRIDE as u64) };
        row("video", |l| { l.size(fs * fh * 4); l.txt("   "); l.dec(fw); l.txt("x"); l.dec(fh); l.txt("x32  fb 0x"); l.hex(unsafe { crate::info::FB_ADDR }, 8); });
    }

    // ── ALMACENAMIENTO ──
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
        let fs = crate::ring0::fs::fs_name();
        row("arranque", |l| { l.txt(fs); l.txt("  LBA "); l.dec(crate::ring0::fs::mounted_lba()); l.txt("  solo lectura"); });
        if crate::ring0::fs::data_mounted() {
            row("datos", |l| { l.txt("LBA "); l.dec(crate::ring0::fs::data_lba()); l.txt("  LECTURA/ESCRITURA"); });
        } else {
            row("datos", |l| { l.txt("sin montar"); });
        }
    }

    // ── PROCESOS Y ARRANQUE ──
    dashboard_log_color("== sistema ==", SH_TITLE);
    let (tasks, runnable) = crate::ring0::scheduler::counts();
    row("tareas", |l| { l.dec(tasks as u64); l.txt(" totales   "); l.dec(runnable as u64); l.txt(" ejecutables"); });
    row("ticks", |l| { l.txt("0x"); l.hex(crate::ring0::timer::ticks(), 8); });
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
    let (total, runnable) = crate::ring0::scheduler::counts();
    crate::ring0::dev::console::serial_write("[tasks] total=");
    crate::ring0::dev::console::serial_write_u64(total as u64, 10);
    crate::ring0::dev::console::serial_write(" runnable=");
    crate::ring0::dev::console::serial_write_u64(runnable as u64, 10);
    crate::ring0::dev::console::serial_write(" current_tid=");
    crate::ring0::dev::console::serial_write_u64(
        crate::ring0::scheduler::current_tid() as u64,
        10,
    );
    crate::ring0::dev::console::serial_write(" ticks=");
    crate::ring0::dev::console::serial_write_u64(crate::ring0::timer::ticks(), 10);
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

/// `bex` — la tabla de programas que este kernel ha ejecutado.
///
/// El log cuenta la historia según pasa y se la lleva el desplazamiento; esto
/// es la FOTO, consultable en cualquier momento: qué se admitió, de qué
/// tamaño, dónde entra, con qué pid, cómo acabó y cuánto llegó a escribir.
fn shell_bex() {
    let progs = crate::ring0::proc::programs();
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
            match crate::ring0::scheduler::tid_state(p.tid) {
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
/// shell prompt on serial — that interleaving IS the context switch.
extern "C" fn ktest_main(arg: u64) -> ! {
    use crate::ring0::dev::console::{serial_write, serial_write_u64};
    serial_write("[ktest] start tid=");
    serial_write_u64(crate::ring0::scheduler::current_tid() as u64, 10);
    serial_write(" arg=");
    serial_write_u64(arg, 10);
    serial_write("\n");
    for i in 0..3u64 {
        serial_write("[ktest] window ");
        serial_write_u64(i, 10);
        serial_write("\n");
        // Busy window ~250 ms so the timer preempts us several times and
        // the shell task runs in between (look for the '>' echoes).
        let start = crate::ring0::scheduler::rdtsc();
        let span = crate::ring0::scheduler::tsc_freq() / 4;
        while crate::ring0::scheduler::rdtsc().wrapping_sub(start) < span {
            core::hint::spin_loop();
        }
    }
    serial_write("[ktest] park 2000 ms (WAIT deadline)\n");
    let deadline = crate::ring0::scheduler::rdtsc()
        + crate::ring0::scheduler::ns_to_tsc(2_000_000_000);
    crate::ring0::scheduler::park_until(deadline);
    serial_write("[ktest] woke; exit via reaper\n");
    crate::ring0::scheduler::exit_and_park();
}

fn shell_ktest() {
    match crate::ring0::scheduler::spawn_kernel(ktest_main as usize as u64, 0xB0, 1) {
        Some(tid) => {
            crate::ring0::dev::console::serial_write("[ktest] spawned tid=");
            crate::ring0::dev::console::serial_write_u64(tid as u64, 10);
            crate::ring0::dev::console::serial_write("\n");
        }
        None => s_log("[ktest] spawn failed (no frames or task slots)"),
    }
}

fn shell_mem() {
    let (total, free) = crate::ring0::mm::phys::stats();
    const PAGE: u64 = 4096;
    let total_b = total * PAGE;
    let free_b = free * PAGE;
    let used_b = total_b.saturating_sub(free_b);

    // Antes esto pintaba en el panel la línea "[mem] stats printed on serial",
    // que es la definición de un comando inútil: te dice que la información
    // existe en un sitio donde no estás mirando.
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
    s_log("[shell] reboot (keyboard reset pulse)");
    unsafe { core::arch::asm!("out 0x64, al", in("al") 0xFEu8); }
    loop { unsafe { core::arch::asm!("hlt"); } }
}

fn shell_halt() -> ! {
    s_log("[shell] halting");
    loop { unsafe { core::arch::asm!("sti; hlt"); } }
}

fn run_shell(ctx: &BootContext) -> ! {
    // Normalize the i8042 (translation → Set 1, re-enable scanning) so the
    // physical keyboard reaches shell_read_line. No-op if the controller is
    // dead/absent (bounded timeouts inside). El stack USB real (xHCI+HID)
    // ya despertó en el Acto I de main().
    crate::ring0::dev::keyboard::init();
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
    if !ctx.is_valid() {
        // Make an invalid BootContext VISIBLE (red @90) instead of a silent
        // halt — otherwise a magic mismatch looks identical to a hang.
        kbar!(90, 0xFFFF_0000u32);
        s_log("[ring0] FATAL: BootContext magic mismatch");
        loop { unsafe { core::arch::asm!("hlt"); } }
    }
    kbar!(110, 0xFFFF_FFFFu32); // white @110: BootContext valid, before percpu

    crate::ring0::dev::console::serial_write("[ring0] BootContext OK, version=");
    crate::ring0::dev::console::serial_write_u64(ctx.version as u64, 10);
    crate::ring0::dev::console::serial_write("\n");
    // Primera entrada de la bitácora, ANTES de que exista framebuffer: CABINA
    // graba desde el minuto cero y lo muestra cuando haya pantalla. Si el
    // kernel muere entre aquí y el shell, el anillo ya tiene lo que pasó.
    crate::ring0::cabina::info("ring0", "BootContext valido, kernel arrancando", ctx.version as u64);

    // Kernel-init checkpoints live in the empty band starting at row 140
    // (well below the boot bars that end near row 120), so any new bar is
    // unmistakably kernel progress — not a repeat of an s1/s2 color.
    crate::ring0::percpu::init_bsp();
    kbar!(140, 0xFF00_FF00u32); // green: percpu OK
    crate::ring0::scheduler::init(ctx.tsc_freq);
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
    crate::ring0::channel::init(ctx);
    crate::ring0::svc::register_all();
    crate::ring0::syscall::init();
    // Arm the on-screen fault reporter before anything can enter Ring 3, so a
    // CPL3 crash paints its vector/RIP/CR2 instead of the silent serial-halt
    // the boot stage installs.
    crate::ring0::faults::init(ctx);
    let timer_ready = crate::ring0::timer::init(ctx);
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
    // errata/speculation mitigations all live behind the profile —
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
    crate::ring0::bex::announce();
    kbar!(212, 0xFF00_FFFFu32); // aqua: bex announce OK, before proc::spawn_init

    // F2: if the boot chain reserved a Ring 3 payload, admit it as the
    // init process. With no payload this is a no-op and the boot flow is
    // exactly the Ring 0 shell as before.
    crate::ring0::proc::init(ctx);
    let ring3_tid = crate::ring0::proc::spawn_init(ctx);
    if let Some(tid) = ring3_tid {
        crate::ring0::dev::console::serial_write("[ring0] Ring 3 init task ready, tid=");
        crate::ring0::dev::console::serial_write_u64(tid as u64, 10);
        crate::ring0::dev::console::serial_write("\n");
        crate::ring0::cabina::info("ring3", "proceso init admitido", tid as u64);
    } else {
        crate::ring0::cabina::warn("ring3", crate::ring0::proc::init_status(), 0);
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

    // ── Intro cinemática (logo → preparando → RING 0 → RING 3) ──────────
    // Escenas centradas con fundido y transición, al estilo de un arranque
    // moderno. Al terminar aterrizamos en el dashboard, donde el trabajo
    // REAL de cada etapa fluye como log (igual que Windows: la animación
    // juega, luego apareces en el escritorio).
    if crate::info::has_fb() {
        splash::boot_intro();
    } else {
        s_log("[splash] no framebuffer, skipping splash");
    }

    // Aterrizar en el dashboard persistente.
    phase1_ui(ctx);

    // ── Acto I: RING 0 despierta el hardware (log real) ─────────────────
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
    // CABINA abre los ojos y censa el almacenamiento (scan PCI). Va AQUÍ, en el
    // acto donde el kernel despierta hardware — antes vivía dentro del render y
    // clavaba ~65k lecturas de config PCI en el primer frame del cockpit.
    crate::ring0::cabina::boot_probe();
    // USB en su lugar narrativo: el kernel despierta teclado y mouse AQUI.
    crate::ring0::dev::usb::init(ctx);
    // Y el disco: el HBA SATA (no el NVMe — ahi vive el sistema del dueño) y
    // su tabla de particiones. Ver dev/disk.rs.
    crate::ring0::dev::disk::init();
    crate::ring0::dev::disk::scan_partitions();
    // Y el sistema de ficheros: de sectores a ARCHIVOS. Monta la partición de
    // arranque, que es donde vive el BOOTX64.EFI con el que arrancamos.
    crate::ring0::fs::mount();
    // El gate: el disco tiene que decir QUIÉN ES antes de que se le pueda
    // escribir. Va DESPUÉS de leer la GPT porque una de las pruebas es que la
    // tabla cuadre con los sectores que el propio disco declara.
    crate::ring0::dev::disk::verify_identity();
    // Y si convenció, el volumen de datos se monta con escritor. La partición
    // de arranque sigue montada sin él, y así se queda.
    crate::ring0::fs::mount_data();
    // Y ESTRATOS, si alguna partición lleva uno. Solo lectura: el módulo no
    // sabe escribir, así que montarlo no puede estropear nada.
    crate::ring0::estratos::mount();
    dash_log("== RING 0 : hardware al mando ==");

    // ── Acto II: RING 3 — el userspace nace ─────────────────────────────
    dash_log("== RING 3 : userspace ==");
    // Surface the Ring 3 init outcome on the (now cleared) dashboard so the
    // demo's state is visible without serial. If a tid was admitted, the next
    // timer tick will enter CPL3 and its 'ring3>' lines should follow below.
    {
        let mut summary = [0u8; 64];
        let head = b"[ring3] ";
        let status = crate::ring0::proc::init_status();
        let mut off = 0;
        for &b in head { if off < summary.len() { summary[off] = b; off += 1; } }
        for &b in status.as_bytes() { if off < summary.len() { summary[off] = b; off += 1; } }
        if let Some(tid) = ring3_tid {
            for &b in b" tid=" { if off < summary.len() { summary[off] = b; off += 1; } }
            if off < summary.len() { summary[off] = b'0' + (tid as u8 % 10); off += 1; }
        }
        if let Ok(s) = core::str::from_utf8(&summary[..off]) { s_log(s); }
    }

    // FINAL checkpoint: bright green at row 236 = kernel finished ALL of
    // phase::main and is entering the shell. If this shows, Ring 0 fully
    // booted on real hardware.
    kbar!(236, 0xFF00_FF00u32);

    if timer_ready {
        crate::ring0::timer::enable();
    }

    run_shell(ctx);
}
