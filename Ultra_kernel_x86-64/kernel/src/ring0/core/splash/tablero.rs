//! **EL TABLERO** -- el panel persistente: la CABINA.
//!
//! Cuando el arranque termina, la pantalla no se apaga: se queda con un panel
//! vivo que es el equivalente visual del shell serie. Muestra el estado del
//! sistema, las ultimas lineas del log del kernel y un prompt, y lo que se
//! escriba por COM1 se ve aqui -- o sea que se puede usar la maquina sin tener
//! un terminal serie enchufado.
//!
//! ## Por que sale de `splash.rs`
//!
//! Porque no es el splash. Son cuatrocientas lineas que hablan de filas de log,
//! de colores por origen del mensaje y de un prompt con cursor; comparten con la
//! intro **el dibujado y nada mas**. Tenerlas en el mismo fichero hacia que el
//! arranque y la CABINA parecieran una sola cosa que se puede tocar de una vez,
//! y no lo son: la intro corre una vez y esto corre siempre.

use super::lienzo::{draw_rect_outline, fill_rect, put_pix, wc_flush};
use super::escena::intro_en_curso;
use super::texto::{draw_str, text_width, CHAR_H, CHAR_W, FONT_H};

// ===================================================================
//  Persistent Dashboard
// ===================================================================
//
// Once the boot splash finishes, the kernel switches to a
// persistent dashboard on the framebuffer. This is the visual
// equivalent of the serial shell: it shows the system status,
// the latest kernel log lines, and a prompt. Anything typed on
// the serial (COM1) is echoed on the screen so the user can
// interact even without a serial terminal attached.

const DASH_HEADER_H:  u32 = 44;  // top bar height
const DASH_FOOTER_H:  u32 = 36;  // bottom prompt bar height
const DASH_LOG_TOP:   u32 = 78;  // y of first log line
const DASH_LOG_W:     u32 = 80;  // max chars per line
const DASH_ROWS_MAX:  usize = 64; // tope duro (protege los buffers de filas)

/// Filas de log que CABEN de verdad en el panel, segun el alto REAL del
/// framebuffer.
///
/// Antes esto era una constante de 14. En 1080p (CHAR_H=20) caben ~49: se
/// desperdiciaban dos tercios del panel y, peor, obligaba al log rodante y a
/// CABINA a pelearse las mismas filas 2-13 borrandose mutuamente. El reparto
/// ahora lo decide el hardware, no un numero magico: preguntale al hardware
/// los HECHOS, hardcodea solo los CONTRATOS.
pub fn dash_rows() -> usize {
    let h = unsafe { crate::info::FB_HEIGHT };
    if h == 0 { return 0; }
    let avail = h.saturating_sub(DASH_FOOTER_H + DASH_LOG_TOP + 4);
    ((avail as usize) / CHAR_H).min(DASH_ROWS_MAX)
}

// -- PALETA: neon sobre negro ------------------------------------------------
//
// El fondo baja casi a negro puro a proposito: un neon solo brilla si lo que
// tiene alrededor esta apagado. El slate azulado anterior le robaba fuerza a
// todos los acentos porque ya era luminoso de por si.
//
// La familia son tres luces frias (cian, jade, violeta) contra tres calidas
// (ambar, oro, magenta), con el rojo lacado reservado EXCLUSIVAMENTE para lo
// que va mal. Que el rojo no se use de adorno es lo que hace que, cuando
// aparece, la vista vaya sola.

const VOID:           u32 = 0xFF04060C; // fuera del panel -- negro con tinte
const PANEL:          u32 = 0xFF080B14; // fondo del area de log
const CHROME:         u32 = 0xFF10151F; // barras superior e inferior
const EDGE:           u32 = 0xFF1E2738; // bordes apagados

const NEON_CYAN:      u32 = 0xFF00F0FF;
const NEON_MAGENTA:   u32 = 0xFFFF2D9B;
const NEON_AMBER:     u32 = 0xFFF6C445; // el amarillo de firma
const NEON_GOLD:      u32 = 0xFFFFB300;
const NEON_RED:       u32 = 0xFFFF3355; // solo para faults
const NEON_GREEN:     u32 = 0xFF39FF88;
const NEON_VIOLET:    u32 = 0xFFA78BFA;
const NEON_JADE:      u32 = 0xFF2DE2C5;

const DASH_BG:        u32 = PANEL;
const DASH_BAR:       u32 = CHROME;
const DASH_ACCENT:    u32 = NEON_CYAN;
const DASH_TEXT:      u32 = 0xFFE6EDF7;
const DASH_DIM:       u32 = 0xFF55647E;

// Colores-filtro por origen de linea (pedido del usuario): quien emite se
// reconoce por color sin leer el prefijo.
const DASH_RING3:     u32 = NEON_GREEN;   // salida de Ring 3
const DASH_TELEMETRY: u32 = NEON_AMBER;   // heartbeat r3hb (tablero)
const DASH_KBD:       u32 = NEON_VIOLET;  // entrada -- teclado y raton
const DASH_FAULT:     u32 = NEON_RED;     // reporter de CPU faults
const DASH_STORAGE:   u32 = NEON_JADE;    // disco y sistema de ficheros
const DASH_LANG_C:    u32 = NEON_CYAN;    // programas C
const DASH_LANG_COB:  u32 = NEON_GOLD;    // programas COBOL
const DASH_LANG_ASM:  u32 = NEON_MAGENTA; // programas en ensamblador
const DASH_STAGE:     u32 = NEON_AMBER;   // encabezados de acto

/// Color de una linea del log segun su prefijo. Un solo punto de decision:
/// TODOS los caminos que pintan al panel (rolling log, CABINA, faults) pasan
/// por aqui.
///
/// La tabla crecio con los emisores que ya existian y salian todos en blanco:
/// los tres lenguajes tenian el mismo color que un mensaje del kernel, asi que
/// la pantalla mas impresionante del proyecto --tres programas propios
/// entrelazandose-- se leia como un parrafo plano. Ahora cada voz tiene la suya.
fn dash_line_color(msg: &str) -> u32 {
    let b = msg.as_bytes();
    // Programas de Ring 3, por lenguaje: cada uno con su luz.
    if b.starts_with(b"C> ") {
        DASH_LANG_C
    } else if b.starts_with(b"COBOL>") {
        DASH_LANG_COB
    } else if b.starts_with(b"asm>") {
        DASH_LANG_ASM
    } else if b.starts_with(b"ring3>") || b.starts_with(b"[ring3]") {
        DASH_RING3
    } else if b.starts_with(b"==") {
        // Encabezados de etapa del boot ("== RING 0 ... ==") y del shell.
        DASH_STAGE
    } else if b.starts_with(b"r3hb") {
        DASH_TELEMETRY
    } else if b.starts_with(b"kbd ") || b.starts_with(b"[usb]") || b.starts_with(b"[xhci]")
        || b.starts_with(b"[uhid]") {
        DASH_KBD
    } else if b.starts_with(b"[disk]") || b.starts_with(b"[ahci]") || b.starts_with(b"[fs]")
        || b.starts_with(b"[cabina]") {
        DASH_STORAGE
    } else if b.starts_with(b"[ring0]") || b.starts_with(b"[bex]") {
        DASH_ACCENT
    } else if b.starts_with(b"***") || b.starts_with(b"vec ") || b.starts_with(b"flt") {
        DASH_FAULT
    } else {
        DASH_TEXT
    }
}

// -- Cromo: las piezas que dan el look ---------------------------------------

/// Linea horizontal de 1 px con degradado entre dos colores.
///
/// Es el truco mas barato que existe para que una interfaz deje de parecer un
/// terminal: una sola fila de pixeles interpolada cuesta un bucle y cambia por
/// completo la sensacion de la barra que subraya.
fn hline_gradient(x: u32, y: u32, w: u32, c1: u32, c2: u32, scale: u32) {
    if w == 0 { return; }
    let (r1, g1, b1) = ((c1 >> 16) & 0xFF, (c1 >> 8) & 0xFF, c1 & 0xFF);
    let (r2, g2, b2) = ((c2 >> 16) & 0xFF, (c2 >> 8) & 0xFF, c2 & 0xFF);
    for i in 0..w {
        // Media ponderada: multiplicar ANTES de dividir. Interpolar por canal
        // con una resta encadenada se rompe en cuanto el color destino es mas
        // oscuro que el de origen, y el degradado se queda plano sin avisar.
        let r = (r1 * (w - i) + r2 * i) / w * scale / 255;
        let g = (g1 * (w - i) + g2 * i) / w * scale / 255;
        let b = (b1 * (w - i) + b2 * i) / w * scale / 255;
        put_pix(x + i, y, 0xFF00_0000 | (r << 16) | (g << 8) | b);
    }
}

/// Regla de neon: un pixel encendido y otro apagandose debajo.
///
/// Dos filas al 100 % se leen como una barra blanca --asi salia en la foto del
/// hardware, porque el brillo satura la camara y tambien el ojo--. La caida
/// abajo es lo que hace que se lea como una LUZ y no como un separador.
fn neon_rule(x: u32, y: u32, w: u32, c1: u32, c2: u32) {
    hline_gradient(x, y, w, c1, c2, 255);
    hline_gradient(x, y + 1, w, c1, c2, 90);
}

/// Esquinas en L en vez de un marco cerrado.
///
/// Es la firma visual del genero: el ojo cierra el rectangulo solo y el panel
/// respira. Un borde continuo encajona; cuatro corchetes sugieren.
fn corner_brackets(x: u32, y: u32, w: u32, h: u32, len: u32, thick: u32, color: u32) {
    if w < len * 2 || h < len * 2 { return; }
    // Superior izquierda
    fill_rect(x, y, len, thick, color);
    fill_rect(x, y, thick, len, color);
    // Superior derecha
    fill_rect(x + w - len, y, len, thick, color);
    fill_rect(x + w - thick, y, thick, len, color);
    // Inferior izquierda
    fill_rect(x, y + h - thick, len, thick, color);
    fill_rect(x, y + h - len, thick, len, color);
    // Inferior derecha
    fill_rect(x + w - len, y + h - thick, len, thick, color);
    fill_rect(x + w - thick, y + h - len, thick, len, color);
}

/// Etiqueta de seccion con su bloque de acento delante: `| TEXTO`.
///
/// El bloque es un rectangulo, no un glifo: la fuente es de 95 caracteres
/// ASCII mas 25 de Latin-1 y no tiene caracteres de dibujo. Pintar el adorno
/// en vez de escribirlo evita inventar glifos que no existen.
fn section_label(x: u32, y: u32, text: &str, accent: u32) {
    fill_rect(x, y + 2, 4, FONT_H as u32 - 4, accent);
    draw_str(x + 12, y, text, DASH_DIM);
}

/// Draw the persistent dashboard frame. Called once after the
/// splash finishes -- replaces the cleared screen with a UI that
/// stays visible for the rest of the kernel's lifetime.
pub fn splash_dashboard_init() {
    // ** MIENTRAS LA INTRO ESTA EN PANTALLA, EL PANEL NO SE PINTA (2026-08-15).
    //
    // === El bug, y por que el arreglo anterior solo tapo la mitad ===
    //
    // Se taparon las FILAS del log y se dio el problema por resuelto. El video
    // del Ryzen enseno la otra mitad: un rectangulo verdeazulado enorme comiendose
    // la esquina superior, con el rotulo `KERNEL LOG` dentro y el gato cortado por
    // la mitad. No eran las filas. Era **el panel entero** -- esta funcion, que
    // empieza rellenando la pantalla de `VOID` y sigue con cabecera, pie, marco y
    // corchetes.
    //
    // === Y quien la llamaba en mitad de la intro ===
    //
    // `phase1_ui`, desde `phase::main`, con el comentario *"aterrizar en el
    // dashboard persistente"* -- que era verdad **cuando la intro bloqueaba**. En
    // aquel modelo la animacion terminaba y despues se aterrizaba. Desde el truco
    // de Santa Monica la intro ya no espera: corre repartida entre los pasos del
    // arranque, asi que esa llamada dejo de ser "despues" y paso a ser "encima".
    //
    // Un cambio que nadie toco rompio una llamada que nadie toco. Por eso el
    // arreglo va AQUI y no solo en el sitio que llamaba: quien pinta el panel es
    // el que sabe si tiene derecho a la pantalla, y asi ninguna llamada futura
    // puede volver a colarse en medio.
    if intro_en_curso() {
        return;
    }
    let w = unsafe { crate::info::FB_WIDTH };
    let h = unsafe { crate::info::FB_HEIGHT };
    if w == 0 || h == 0 { return; }

    // 1. El vacio. Todo lo que no es panel ni barra queda casi negro para que
    //    el neon tenga contra que brillar.
    fill_rect(0, 0, w, h, VOID);

    // 2. Barra superior: identidad del sistema.
    fill_rect(0, 0, w, DASH_HEADER_H, CHROME);
    // Marca de acento a la izquierda -- el bloque vertical que ancla el titulo.
    fill_rect(0, 0, 5, DASH_HEADER_H, NEON_MAGENTA);
    // El nombre en dos pesos: la marca en ambar, el subsistema en magenta.
    // Separarlos dice de un vistazo QUE es y DONDE esta corriendo.
    draw_str(22, 14, "BMO-X", NEON_AMBER);
    let x_after = 22 + text_width("BMO-X") + 12;
    draw_str(x_after, 14, "// RING 0", NEON_MAGENTA);
    let x_sub = x_after + text_width("// RING 0") + 16;
    draw_str(x_sub, 14, "bare metal orchestrator", DASH_DIM);
    // Subrayado de neon que recorre la barra: cian a la izquierda, magenta a
    // la derecha. Es la pieza que mas cambia la sensacion por menos pixeles.
    neon_rule(0, DASH_HEADER_H - 2, w, NEON_CYAN, NEON_MAGENTA);

    // 3. Barra inferior: el prompt.
    let fy = h - DASH_FOOTER_H;
    fill_rect(0, fy, w, DASH_FOOTER_H, CHROME);
    fill_rect(0, fy, 5, DASH_FOOTER_H, NEON_CYAN);
    neon_rule(0, fy, w, NEON_MAGENTA, NEON_CYAN);

    // 4. El panel del log: fondo propio, un punto mas claro que el vacio, para
    //    que se lea como una superficie y no como un agujero.
    let log_y = DASH_LOG_TOP;
    let log_h = h - DASH_FOOTER_H - log_y - 4;
    fill_rect(8, log_y - 6, w - 16, log_h, PANEL);
    // Bordes tenues + esquinas en L encendidas.
    draw_rect_outline(8, log_y - 6, w - 16, log_h, EDGE);
    corner_brackets(8, log_y - 6, w - 16, log_h, 22, 2, NEON_CYAN);

    // 5. Etiqueta de seccion. Va anclada al BORDE DE LA CABECERA, no restando
    //    del log: calculada hacia atras desde el log caia justo sobre la regla
    //    de neon y en el hardware el texto salia montado en la linea.
    section_label(14, DASH_HEADER_H + 8, "KERNEL LOG", NEON_CYAN);
}

/// Write a single log line into the dashboard's log area at
/// line `row` (0 = top, growing downward). Newer lines overwrite
/// older ones on the same row, so callers can manage a ring of
/// `dash_rows()` rows.
pub fn splash_dashboard_log(row: usize, msg: &str) {
    let c = dash_line_color(msg);
    splash_dashboard_log_color(row, msg, c);
}

/// Regla de separacion con etiqueta, a la altura de una fila del panel.
///
/// Es lo que separa el log rodante del cockpit de CABINA. Antes las dos zonas
/// se tocaban y la unica pista de donde acababa una era leer el contenido;
/// ahora hay una frontera que se ve sin leer. La linea se apaga hacia la
/// derecha para no competir con el texto que viene debajo.
///
/// El texto tiene que ser ASCII: la consola es Latin-1 de un byte por caracter
/// y un literal Rust con acentos viajaria en UTF-8, o sea dos glifos raros
/// donde deberia haber uno.
pub fn splash_dash_rule(row: usize, label: &str, accent: u32) {
    // La misma puerta que el resto del panel: esto rellena una franja de `PANEL`
    // y pintarla sobre la ciudad es el mismo bug con otra forma.
    if intro_en_curso() {
        return;
    }
    let w = unsafe { crate::info::FB_WIDTH };
    if w == 0 || row >= dash_rows() { return; }
    let y = DASH_LOG_TOP + (row as u32) * CHAR_H as u32;
    fill_rect(14, y, w - 28, CHAR_H as u32, PANEL);
    fill_rect(14, y + 3, 4, CHAR_H as u32 - 8, accent);
    draw_str(28, y + 1, label, accent);
    let lx = 28 + text_width(label) + 14;
    let right = w.saturating_sub(20);
    if right > lx {
        hline_gradient(lx, y + (CHAR_H as u32) / 2, right - lx, accent, PANEL, 255);
    }
}

/// Igual que `splash_dashboard_log` pero con COLOR EXPLICITO -- para que CABINA
/// pinte cada fila segun su estado (verde=bien, ambar=atencion, rojo=problema)
/// en vez de un solo color plano.
pub fn splash_dashboard_log_color(row: usize, msg: &str, color: u32) {
    // ** MIENTRAS LA INTRO ESTA EN PANTALLA, ESTO NO PINTA (2026-08-15).
    //
    // Es la mitad que faltaba del truco de Santa Monica. La intro dejo de
    // SUMARSE al arranque y paso a TAPARLO... y el log siguio pintando **encima
    // de ella**. En el video del Ryzen se ve el resultado: un panel oscuro
    // comiendose los dos tercios de arriba de la pantalla con la ciudad
    // asomando por debajo. Dos capas peleandose por el mismo sitio, que es
    // literalmente lo que el dueno describio: *"la capa estan mezcladas"*.
    //
    // El dueno tambien dijo que hacer con eso, y sin ambiguedad: *"en codigos de
    // kernel en tiempo real esta en 0% a la vista, claro, porque eso no importa
    // sino la presentacion"*.
    //
    // [!] No se pierde NADA. Esta funcion solo PINTA: la linea ya viaja por
    // serie y ya esta guardada en el anillo de CABINA, que es de donde sale F11.
    // Lo unico que se apaga son los pixeles, y solo durante los dos segundos de
    // la intro. Si la intro no llegara a cerrarse, el arranque se veria mudo en
    // pantalla y seguiria hablando por el cable -- que es el canal del que
    // depura, y el que importa cuando algo va mal.
    if intro_en_curso() {
        return;
    }
    let w = unsafe { crate::info::FB_WIDTH };
    let h = unsafe { crate::info::FB_HEIGHT };
    if w == 0 || h == 0 { return; }
    if row >= dash_rows() { return; }
    let y = DASH_LOG_TOP + (row as u32) * CHAR_H as u32;
    // Clear the row (background)
    fill_rect(14, y, w - 28, CHAR_H as u32, DASH_BG);
    // Marca de canaleta: una barrita del color de la linea en el margen.
    //
    // El color del texto ya dice quien habla, pero hay que LEER la linea para
    // notarlo. Una columna de marcas alineadas se lee de un vistazo: se ve
    // cuantas voces distintas hay en pantalla y donde cambia el turno, sin
    // leer una sola palabra. Es lo que convierte el log en algo que se OJEA.
    //
    // El texto normal no lleva marca: si todo estuviera marcado, la columna no
    // diria nada. Marcar es distinguir.
    if color != DASH_TEXT {
        fill_rect(14, y + 4, 3, CHAR_H as u32 - 8, color);
    }
    // Draw up to DASH_LOG_W characters
    let mut buf = [0u8; DASH_LOG_W as usize];
    let bytes = msg.as_bytes();
    let n = bytes.len().min(buf.len());
    buf[..n].copy_from_slice(&bytes[..n]);
    if let Ok(s) = core::str::from_utf8(&buf[..n]) {
        draw_str(28, y, s, color);
    }
}

/// Update the bottom prompt area with the current command being
/// typed. The caller passes the in-progress line (up to a
/// reasonable limit). The prompt always starts with "serial > ".
pub fn splash_dashboard_prompt(line: &str, cursor: usize, blink: bool) {
    // La tercera puerta del panel. Hoy nadie escribe en el prompt durante la
    // intro, pero las tres pintan `CHROME` sobre la pantalla y la regla tiene
    // que ser una sola: **mientras la intro esta, el panel no existe**. Dejar una
    // sin puerta es dejar el mismo bug esperando a que alguien la llame.
    if intro_en_curso() {
        return;
    }
    let w = unsafe { crate::info::FB_WIDTH };
    let h = unsafe { crate::info::FB_HEIGHT };
    if w == 0 || h == 0 { return; }
    let y = h - DASH_FOOTER_H + 10;
    fill_rect(20, y, w - 40, CHAR_H as u32, CHROME);
    // El prompt ya no dice "serial": el teclado USB escribe desde hace tiempo
    // y la etiqueta se habia quedado contando una etapa anterior del proyecto.
    // La marca en ambar, el signo en magenta -- los mismos dos colores del
    // titulo, para que cabecera y pie se lean como el mismo sistema.
    const PROMPT: &str = "bmo-x";
    draw_str(20, y, PROMPT, NEON_AMBER);
    let sign_x = 20 + text_width(PROMPT) + 8;
    draw_str(sign_x, y, ">", NEON_MAGENTA);
    let prefix_w = text_width(PROMPT) + 8 + text_width("> ") + 4;
    let max_chars = ((w - 40 - prefix_w) / CHAR_W as u32) as usize;
    let n = line.len().min(max_chars);
    let s = &line[..n];
    draw_str(20 + prefix_w, y, s, DASH_TEXT);
    // Cursor de bloque parpadeante EN SU POSICION dentro de la linea, no
    // siempre al final: con las flechas se edita en medio, y el cursor tiene
    // que estar donde va a caer la siguiente letra. Si tapa un caracter, se
    // redibuja encima en el color del fondo -- video inverso, como una terminal
    // de verdad.
    if blink {
        let cx = 20 + prefix_w + (cursor.min(n) as u32) * CHAR_W as u32;
        fill_rect(cx, y, (CHAR_W as u32) - 2, FONT_H as u32, NEON_MAGENTA);
        if cursor < n {
            let one = [line.as_bytes()[cursor]];
            if let Ok(ch) = core::str::from_utf8(&one) {
                draw_str(cx, y, ch, CHROME);
            }
        }
    }
    wc_flush();
}


/// Indicadores de la barra superior: distribucion de teclado activa y estado
/// de los bloqueos. Las lucecitas fisicas de un teclado pueden no responder
/// (firmware, emulacion); la pantalla no depende de eso.
pub fn splash_status_right(layout: &str, caps: bool, num: bool) {
    let w = unsafe { crate::info::FB_WIDTH };
    if w == 0 { return; }

    // La franja se limpia entera antes de escribir: al apagarse un indicador su
    // texto tiene que desaparecer, no quedarse pegado.
    let bar_x = w.saturating_sub(460);
    fill_rect(bar_x, 8, w.saturating_sub(bar_x + 16), DASH_HEADER_H - 12, CHROME);

    // Los bloqueos dejan de ser texto suelto y pasan a ser PASTILLAS: fondo
    // encendido y letra oscura. Un estado activo se ve encendido, no escrito --
    // que es justo lo que un teclado cuyas lucecitas no responden necesita.
    let caps_w = text_width("MAYUS") + 14;
    let num_w  = text_width("NUM") + 14;
    let mut kbd = [0u8; 32];
    let mut ko = 0usize;
    for &c in b"kbd ".iter() { if ko < kbd.len() { kbd[ko] = c; ko += 1; } }
    for &c in layout.as_bytes() { if ko < kbd.len() { kbd[ko] = c; ko += 1; } }
    let kbd_s = core::str::from_utf8(&kbd[..ko]).unwrap_or("");
    let kbd_w = text_width(kbd_s);

    let mut total = kbd_w;
    if caps { total += caps_w + 10; }
    if num  { total += num_w + 10; }
    let mut x = w.saturating_sub(total + 20);

    draw_str(x, 14, kbd_s, DASH_DIM);
    x += kbd_w + 10;
    if caps {
        fill_rect(x, 10, caps_w, FONT_H as u32 + 8, NEON_AMBER);
        draw_str(x + 7, 14, "MAYUS", CHROME);
        x += caps_w + 10;
    }
    if num {
        fill_rect(x, 10, num_w, FONT_H as u32 + 8, NEON_JADE);
        draw_str(x + 7, 14, "NUM", CHROME);
    }
}
