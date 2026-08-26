//! **THE RING 0 SHELL: screen, line editor and built-ins.**
//!
//! === Why this is here and not in `phase.rs` ===
//!
//! Because `core/shell/` already existed --`hardware`, `ficheros`, `pantalla`,
//! `peligro`-- and the other half of the shell had stayed behind in a file
//! named after the boot phases. The commands lived in one place and the thing
//! that reads the command line lived in another, four hundred lines from any of
//! them.
//!
//! === The three layers in here, and they are worth telling apart ===
//!
//! 1. **The toolkit**: `SH_TITLE`/`SH_LABEL`/`SH_VALUE` and `L`/`row`. One
//!    hierarchy of colour for every command, so `info` and `disk` read as parts
//!    of one system instead of two programs.
//! 2. **The line editor**: `shell_read_line` plus the history ring. This is the
//!    only place in Ring 0 that owns a cursor, and the invariant it has to keep
//!    is that the cursor never runs past the text -- the same invariant whose
//!    absence in the Ring 3 field once took the compositor down.
//! 3. **The built-ins that are about the shell itself**: `help`, `hist`,
//!    `layout`. The ones about the machine are the sibling files.
//!
//! [!] Printed text stays in Spanish.

use super::super::splash;
use super::super::dashboard::{dashboard_log, dashboard_log_color};
use super::super::phase::s_log;
use super::super::dashboard::{dash_log, DASH_LOG_ROW};

/// Colores de los informes del shell. Etiqueta apagada, valor claro, titulo
/// ambar: la misma jerarquia en todos los comandos, para que `info` y `disk`
/// se lean como partes del mismo sistema y no como dos programas distintos.
pub(crate) const SH_TITLE: u32 = 0xFFF6C445; // ambar
pub(crate) const SH_LABEL: u32 = 0xFF00F0FF; // cian
pub(crate) const SH_VALUE: u32 = 0xFFE6EDF7; // texto

pub(crate) struct L { b: [u8; 96], o: usize }

impl L {
    pub(crate) fn new() -> Self { Self { b: [0u8; 96], o: 0 } }
    pub(crate) fn txt(&mut self, s: &str) {
        for &c in s.as_bytes() { if self.o < self.b.len() { self.b[self.o] = c; self.o += 1; } }
    }
    pub(crate) fn dec(&mut self, mut v: u64) {
        if v == 0 { self.txt("0"); return; }
        let mut tmp = [0u8; 20];
        let mut i = 0;
        while v > 0 { tmp[i] = b'0' + (v % 10) as u8; v /= 10; i += 1; }
        while i > 0 { i -= 1; if self.o < self.b.len() { self.b[self.o] = tmp[i]; self.o += 1; } }
    }
    pub(crate) fn hex(&mut self, v: u64, digits: usize) {
        const H: &[u8; 16] = b"0123456789ABCDEF";
        for i in (0..digits).rev() {
            if self.o < self.b.len() { self.b[self.o] = H[((v >> (i * 4)) & 0xF) as usize]; self.o += 1; }
        }
    }
    /// Rellena con espacios hasta la columna `col`. Es lo que mantiene las
    /// etiquetas alineadas sin contar caracteres a mano en cada linea.
    pub(crate) fn col(&mut self, col: usize) {
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
    pub(crate) fn size(&mut self, bytes: u64) {
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
    pub(crate) fn pct(&mut self, part: u64, whole: u64) {
        if whole == 0 { self.txt("0%"); return; }
        self.dec(part * 100 / whole);
        self.txt("%");
    }
    pub(crate) fn as_str(&self) -> &str { core::str::from_utf8(&self.b[..self.o]).unwrap_or("") }
}

/// Una fila de informe: etiqueta a la izquierda, valor alineado en la columna 10.
pub(crate) fn row(label: &str, build: impl FnOnce(&mut L)) {
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
pub(crate) fn dash_prompt(line: &str, cursor: usize) {
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

pub(crate) fn shell_prompt() {
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

pub(crate) const HIST_MAX: usize = 16;
pub(crate) const HIST_LINE: usize = 64;
static mut HIST: [[u8; HIST_LINE]; HIST_MAX] = [[0; HIST_LINE]; HIST_MAX];
static mut HIST_LEN: [usize; HIST_MAX] = [0; HIST_MAX];
static mut HIST_COUNT: usize = 0; // cuantas lineas hay (tope HIST_MAX)
static mut HIST_HEAD: usize = 0;  // donde se escribe la siguiente

/// Guarda una linea en el historial. No repite la inmediatamente anterior:
/// pulsar Enter tres veces sobre el mismo comando no deberia llenar el
/// historial de copias.
pub(crate) fn hist_push(line: &[u8]) {
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
pub(crate) fn hist_get(back: usize) -> Option<&'static [u8]> {
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
pub(crate) fn shell_read_line(buf: &mut [u8]) -> usize {
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
            // == LAS TECLAS DE FUNCION: EL PUESTO DE MANDO ================
            //
            // ** Este terminal es el SUELO. Es donde caes cuando el escritorio
            // se muere, o sea el sitio donde menos ganas tienes de escribir y
            // mas prisa tienes por ver que paso. Y hasta hoy todo --sin
            // excepcion-- habia que teclearlo entero, con un teclado que
            // ademas es el aparato que mas ha fallado en esta maquina.
            //
            // Las ocho ordenes que se piden el dia malo pasan a UNA tecla. No
            // hay menu ni modo: la tecla **escribe la orden en la linea y la
            // entrega**, exactamente como si la hubieras tecleado tu. Por eso
            // el despachador de `run_shell` no cambia ni una linea, y por eso
            // la orden queda en el HISTORIAL -- si pulsas F2 y luego flecha
            // arriba, ahi esta `consumo`, y aprendes el nombre sin que nadie
            // te lo ensene.
            //
            // [!] Y esto tapa un agujero que llevaba ahi desde siempre: el
            // brazo de "imprimible" de abajo excluye `is_nav` (0x80..0x88) pero
            // NO las de funcion (0x89..0x94), asi que pulsar F5 **insertaba un
            // byte de basura en la linea**. Ahora se consumen aqui, y las que
            // no tienen orden se ignoran en el `_ =>` en vez de escribirse.
            f if kb::is_funcion(f) => {
                let orden: &[u8] = match f {
                    kb::KEY_F1 => b"help",
                    kb::KEY_F2 => b"consumo",
                    kb::KEY_F3 => b"apps",
                    // La del dia malo, y por eso esta en el centro de la fila.
                    kb::KEY_F4 => b"fallo",
                    kb::KEY_F5 => b"info",
                    kb::KEY_F6 => b"tasks",
                    kb::KEY_F7 => b"mem",
                    kb::KEY_F8 => b"cabina",
                    // ** F9 y F10 entraron el 2026-08-24, y F9 llevaba
                    // PROMETIDA desde antes: `report.rs` decia *"el panel de F9
                    // tiene su propia tecla"* sobre un panel que no existia.
                    // Es la ley 8 --verde no es cableado-- en un comentario.
                    kb::KEY_F9 => b"net",
                    kb::KEY_F10 => b"placa",
                    _ => b"",
                };
                if !orden.is_empty() {
                    let k = orden.len().min(buf.len());
                    buf[..k].copy_from_slice(&orden[..k]);
                    crate::ring0::dev::console::serial_write("\n");
                    hist_push(&buf[..k]);
                    return k;
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

pub(crate) fn shell_help() {
    // Por categorias y con las columnas alineadas por el mismo constructor que
    // usan `info` y `disk`: antes cada comando alineaba a ojo con espacios
    // contados a mano, y bastaba una palabra mas larga para torcer la columna.
    dashboard_log_color("== BMO-X shell ==", SH_TITLE);
    // ** `ext` se cablo en el despachador y NO se anadio aqui ni a `ORDENES`,
    // asi que existia sin poder descubrirse: `ext` contestaba, pero quien no
    // supiera de memoria que existe no tenia como enterarse, y el aviso de
    // arranque manda literalmente a "escribe ext". Una orden invisible es una
    // orden que no esta. Arreglado el 2026-08-16, y el propio fallo es el
    // argumento de por que las dos listas viven en este fichero.
    row("sistema", |l| l.txt("info  cpu  ext  mem  consumo  apps  tasks  disk  net  placa  ls  estratos  cabina  hist"));
    // `fallo` en su propio renglon y con lo que hace escrito: es la orden que
    // hace falta el dia peor, y ese dia nadie se acuerda de una palabra suelta
    // en una fila de diez. Ver `shell_fallo`.
    row("fallos", |l| l.txt("fallo [0..3]   la autopsia de la ultima tarea de Ring 3 que murio"));
    row("nucleos", |l| l.txt("smp  (escribelo a secas y te dice sus opciones)"));
    row("teclas", |l| l.txt("F1 ayuda  F2 consumo  F3 apps  F4 fallo  F5 info  F6 tareas  F7 mem  F8 cabina  F9 red  F10 placa"));
    row("edicion", |l| l.txt("flechas  Inicio/Fin  Supr  ^A ^E ^U ^K ^W ^C ^L"));
    row("video", |l| l.txt("fb  splash  cls"));
    row("ring3", |l| l.txt("run <ruta>  bex  ktest"));
    // ** `escritorio` va en su propia fila y no metido entre las de `ring3`.
    //
    // Es la orden que se busca justo cuando se ha llegado aqui sin querer --por
    // la patada del kernel o por dos `Ctrl+Alt+Esc`-- y quien esta en esa
    // situacion no lee una fila de diez palabras: busca la suya. Una fila con
    // una sola orden es la que se encuentra el dia peor.
    row("volver", |l| l.txt("escritorio   levanta otra vez el de Ring 3"));
    row("rescate", |l| l.txt("Ctrl+Alt+Esc  quita la pantalla; DOS veces en 3s echa al escritorio"));
    row("poder", |l| l.txt("reboot  halt  panic"));
    row("ayuda", |l| l.txt("help"));
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
/// **Las ordenes del shell, en un sitio.** Devuelve el nombre si `texto` es una.
///
/// Existe para que `run net` conteste *"eso es una orden, escribe `net`"* en vez
/// de *"el archivo no esta: revisa la ruta"* -- que es lo que decia, y manda a
/// mirar el disco cuando lo que sobra es una palabra.
///
/// ** La lista esta aqui y no repartida por el `if` gigante a proposito: es la
/// misma que pinta `help`, y dos listas de ordenes que hay que acordarse de
/// mantener a la vez son dos listas que un dia no dicen lo mismo.
pub(crate) fn similar_command(texto: &str) -> Option<&'static str> {
    const ORDENES: &[&str] = &[
        "help", "ls", "disk", "net", "red", "placa", "firmware", "audio", "sonido", "cabina", "fallo", "estratos", "cpu",
        "ext", "extensiones", "consumo", "gasto", "w", "apps", "programas",
        "hist", "history", "layout", "cls", "clear", "info", "smp", "banda", "tasks", "mem",
        "ktest", "fb", "splash", "bex", "panic", "reboot", "halt",
    ];
    // Solo el primer trozo: `run smp prueba` tambien tiene que reconocerse.
    let primera = match texto.find(' ') {
        Some(i) => &texto[..i],
        None => texto,
    };
    ORDENES.iter().copied().find(|&o| o == primera)
}


/// `hist` -- la lista de comandos ejecutados, numerada. Lo mismo que recorren
/// las flechas arriba/abajo, pero de un vistazo.
pub(crate) fn shell_hist() {
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
pub(crate) fn shell_layout(arg: &[u8]) {
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
