//! Teclado: scancodes Set 1 → caracteres, con distribución ESPAÑOLA.
//!
//! Dos productores entran por aquí: el i8042 (PS/2, muerto post-EBS en esta
//! placa) y el puente USB HID (`dev::usb`), que traduce sus reportes a los
//! mismos scancodes Set 1. Un solo sitio decide qué letra es cada tecla.
//!
//! El scancode dice QUÉ TECLA se pulsó, nunca qué letra es: eso lo decide la
//! distribución impresa en el teclado. La tabla US daba `;` donde el teclado
//! del usuario tiene `ñ`, y `-` donde tiene `'`. Aquí viven US, español
//! latinoamericano y español de España, y se cambian en caliente con el
//! comando `layout` del shell.

const DATA: u16 = 0x60;
const STATUS: u16 = 0x64;

#[inline]
fn inb(port: u16) -> u8 {
    let v: u8;
    unsafe { core::arch::asm!("in al, dx", in("dx") port, out("al") v, options(nomem, nostack)); }
    v
}

#[inline]
fn outb(port: u16, val: u8) {
    unsafe { core::arch::asm!("out dx, al", in("dx") port, in("al") val, options(nomem, nostack)); }
}

// i8042 controller commands / config-byte bits.
const CMD_READ_CONFIG: u8 = 0x20;
const CMD_WRITE_CONFIG: u8 = 0x60;
const CFG_TRANSLATE: u8 = 1 << 6;         // Set 2 -> Set 1 translation
const CFG_KBD_CLOCK_DISABLE: u8 = 1 << 4; // 1 = clock off

/// Spin until the controller's input buffer is clear (safe to write), or a
/// bounded timeout elapses. Returns false on timeout so a dead/absent
/// controller can never hang the boot.
fn wait_input_clear() -> bool {
    let mut t = 200_000u32;
    while inb(STATUS) & 0x02 != 0 {
        t = t.saturating_sub(1);
        if t == 0 { return false; }
    }
    true
}

/// Spin until a byte is waiting in the output buffer, or timeout.
fn wait_output_full() -> bool {
    let mut t = 200_000u32;
    while inb(STATUS) & 0x01 == 0 {
        t = t.saturating_sub(1);
        if t == 0 { return false; }
    }
    true
}

/// Normalize the i8042 to deliver Scancode Set 1: enable controller
/// translation so a keyboard reporting Set 2 still reaches this driver as
/// Set 1 (which `resolve` decodes). Also re-enables the keyboard clock
/// and scanning. Every wait is bounded, so if the controller is dead or
/// absent (e.g. USB-legacy emulation stopped at ExitBootServices) this is
/// simply a no-op and the boot continues.
pub fn init() {
    if !wait_input_clear() { return; }
    outb(STATUS, CMD_READ_CONFIG);
    if !wait_output_full() { return; }
    let cfg = inb(DATA);

    let newcfg = (cfg | CFG_TRANSLATE) & !CFG_KBD_CLOCK_DISABLE;
    if !wait_input_clear() { return; }
    outb(STATUS, CMD_WRITE_CONFIG);
    if !wait_input_clear() { return; }
    outb(DATA, newcfg);

    // Re-issue "enable scanning" (0xF4) to the keyboard device.
    if !wait_input_clear() { return; }
    outb(DATA, 0xF4);
}

/// Left/right shift held? Tracked across polls for upper/lower case.
static mut SHIFT: bool = false;
/// Caps Lock activo (toggle, como Windows). Afecta SOLO las letras:
/// el caso efectivo de una letra es Shift XOR Caps; los símbolos ignoran Caps.
static mut CAPS: bool = false;
/// AltGr (Alt derecho) mantenido: el tercer nivel del teclado español, donde
/// viven @ # \ | { } [ ] ~ — todo lo que hace falta para programar.
static mut ALTGR: bool = false;

// ═══════════════════════════════════════════════════════════════════════════
//  Distribuciones
// ═══════════════════════════════════════════════════════════════════════════

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Layout {
    /// US QWERTY (la de siempre).
    Us,
    /// Español LATINOAMERICANO — la del teclado del usuario (SEISA). Se
    /// distingue de la de España en que `{ } [ ]` están en las teclas de la
    /// derecha y `@` es AltGr+Q.
    EsLatam,
    /// Español de ESPAÑA: ç junto al Enter, º/ª a la izquierda del 1,
    /// `@` es AltGr+2.
    EsSpain,
}

static mut LAYOUT: Layout = Layout::EsLatam;

pub fn layout() -> Layout { unsafe { LAYOUT } }
pub fn set_layout(l: Layout) { unsafe { LAYOUT = l; } }
pub fn layout_name() -> &'static str {
    match layout() {
        Layout::Us => "us",
        Layout::EsLatam => "es-latam",
        Layout::EsSpain => "es-espana",
    }
}

// Bytes Latin-1 que produce el teclado español (el font los dibuja: ver
// toolchain/tools/fontgen). Un carácter = un byte, sin UTF-8 en Ring 0.
const N_TILDE_MIN: u8 = 0xF1; // ñ
const N_TILDE_MAY: u8 = 0xD1; // Ñ
const ACUTE: u8 = 0xB4;       // ´  (tecla muerta)
const DIAER: u8 = 0xA8;       // ¨  (tecla muerta)
const INV_EXCL: u8 = 0xA1;    // ¡
const INV_QUES: u8 = 0xBF;    // ¿
const MASC_ORD: u8 = 0xBA;    // º
const FEM_ORD: u8 = 0xAA;     // ª
const DEGREE: u8 = 0xB0;      // °
const MIDDOT: u8 = 0xB7;      // ·
const NOT_SIGN: u8 = 0xAC;    // ¬
const CEDIL_MIN: u8 = 0xE7;   // ç
const CEDIL_MAY: u8 = 0xC7;   // Ç

// AltGr llega como scancode propio desde el puente USB (`bmo_uhid::SC_ALTGR`);
// Set 1 lo expresa como `0xE0 0x38` y por el PS/2 se detecta con ese prefijo
// (ver `poll_event`). Una sola definición del código, en el productor.

/// Resultado de resolver una tecla.
enum Out {
    /// Nada que emitir (modificador, tecla sin significado en el shell).
    Nothing,
    /// Un carácter listo.
    Ch(u8),
    /// TECLA MUERTA: no imprime, espera a la siguiente para combinarse
    /// (´ + a = á). Lleva el byte del signo por si al final no combina.
    Dead(u8),
}

// ═══════════════════════════════════════════════════════════════════════════
//  Cola de salida
// ═══════════════════════════════════════════════════════════════════════════
//
// Una pulsación puede producir DOS caracteres (una tecla muerta que no combina
// emite el acento y luego la letra), y un solo sondeo del HID puede traer
// varias teclas a la vez. Antes se devolvía "la última, y las demás se
// pierden" — escribir rápido se comía letras. Ahora todo entra en esta cola y
// el shell la vacía a su ritmo.

const OUT_MAX: usize = 32;
static mut OUT_BUF: [u8; OUT_MAX] = [0; OUT_MAX];
static mut OUT_R: usize = 0;
static mut OUT_W: usize = 0;

fn push_out(b: u8) {
    unsafe {
        let next = (OUT_W + 1) % OUT_MAX;
        if next == OUT_R { return; } // cola llena: se descarta lo nuevo
        OUT_BUF[OUT_W] = b;
        OUT_W = next;
    }
}

/// Saca el siguiente carácter pendiente, si lo hay.
pub fn pop_out() -> Option<u8> {
    unsafe {
        if OUT_R == OUT_W { return None; }
        let b = OUT_BUF[OUT_R];
        OUT_R = (OUT_R + 1) % OUT_MAX;
        Some(b)
    }
}

/// Signo muerto pendiente (0 = ninguno).
static mut DEAD_PENDING: u8 = 0;

/// Combina un signo muerto con la letra siguiente. `None` si no hay
/// combinación válida (entonces se emiten los dos por separado).
fn combine(dead: u8, base: u8) -> Option<u8> {
    match (dead, base) {
        (ACUTE, b'a') => Some(0xE1), (ACUTE, b'e') => Some(0xE9),
        (ACUTE, b'i') => Some(0xED), (ACUTE, b'o') => Some(0xF3),
        (ACUTE, b'u') => Some(0xFA),
        (ACUTE, b'A') => Some(0xC1), (ACUTE, b'E') => Some(0xC9),
        (ACUTE, b'I') => Some(0xCD), (ACUTE, b'O') => Some(0xD3),
        (ACUTE, b'U') => Some(0xDA),
        (DIAER, b'u') => Some(0xFC), (DIAER, b'U') => Some(0xDC),
        _ => None,
    }
}

/// Procesa UNA tecla pulsada y deja lo que produzca en la cola de salida.
/// Punto único por el que pasan tanto el teclado USB como el PS/2.
pub(crate) fn feed(code: u8, shift: bool, altgr: bool, caps: bool) {
    let out = resolve(code, shift, altgr, caps);
    unsafe {
        let pending = DEAD_PENDING;
        match out {
            Out::Nothing => {}
            Out::Dead(sign) => {
                // Dos signos muertos seguidos: el primero se imprime tal cual
                // (así se escribe un acento suelto).
                if pending != 0 { push_out(pending); }
                DEAD_PENDING = sign;
            }
            Out::Ch(c) => {
                if pending != 0 {
                    DEAD_PENDING = 0;
                    match combine(pending, c) {
                        Some(acc) => push_out(acc),
                        // No combinan: salen los dos, en orden.
                        None => { push_out(pending); push_out(c); }
                    }
                } else {
                    push_out(c);
                }
            }
        }
    }
}

/// Poll the controller once. Returns `(raw_scancode, Some(char))` when a key
/// produced a character, `(raw, None)` for anything else (releases,
/// modifiers, dead keys still pending). The raw byte lets the shell surface
/// the stream on screen — the difference between "no bytes at all" (legacy
/// emulation dead post-EBS) and "bytes in an unexpected scancode set"
/// (translation problem) in one look. Never blocks.
pub fn poll_event() -> Option<(u8, Option<u8>)> {
    let status = inb(STATUS);
    if status & 0x01 == 0 {
        return None; // output buffer empty — no byte waiting
    }
    if status & 0x20 != 0 {
        // Bit 5 set = second-port (mouse) byte. Drain it and ignore so it
        // does not desync the keyboard stream.
        let _ = inb(DATA);
        return None;
    }
    let code = inb(DATA);
    // Prefijo 0xE0: la tecla siguiente es "extendida" (AltGr, Ctrl derecho,
    // flechas...). Se recuerda para el próximo byte — así AltGr (0xE0 0x38)
    // no se confunde con el Alt izquierdo (0x38 a secas).
    static mut E0: bool = false;
    let ext = unsafe { let e = E0; E0 = false; e };
    match code {
        0xE0 => { unsafe { E0 = true; } }
        0x38 if ext => { unsafe { ALTGR = true; } }
        0xB8 if ext => { unsafe { ALTGR = false; } }
        0x2A | 0x36 => { unsafe { SHIFT = true; } }
        0xAA | 0xB6 => { unsafe { SHIFT = false; } }
        0x3A => { unsafe { CAPS = !CAPS; } }
        c if c & 0x80 != 0 => {} // cualquier otro release
        c => feed(c, unsafe { SHIFT }, unsafe { ALTGR }, unsafe { CAPS }),
    }
    Some((code, pop_out()))
}

/// Vista "solo carácter" del teclado PS/2. Primero vacía lo que quedó en la
/// cola (una tecla puede haber producido dos caracteres).
pub fn poll_ascii() -> Option<u8> {
    if let Some(b) = pop_out() { return Some(b); }
    poll_event().and_then(|(_, a)| a)
}

/// Resuelve un scancode Set 1 al carácter que le corresponde SEGÚN LA
/// DISTRIBUCIÓN ACTIVA. `caps` invierte el caso solo de letras (como Windows).
fn resolve(code: u8, shift: bool, altgr: bool, caps: bool) -> Out {
    // Común a todas las distribuciones: letras, control y teclado numérico.
    // La posición de las letras es la misma en US, España y Latinoamérica
    // (todas QWERTY); lo que cambia es la puntuación.
    let common = match code {
        0x0E => Some(0x08),  // Backspace
        0x0F => Some(b'\t'), // Tab
        0x1C => Some(b'\r'), // Enter
        0x39 => Some(b' '),  // Espacio
        0x10 => Some(b'q'), 0x11 => Some(b'w'), 0x12 => Some(b'e'), 0x13 => Some(b'r'),
        0x14 => Some(b't'), 0x15 => Some(b'y'), 0x16 => Some(b'u'), 0x17 => Some(b'i'),
        0x18 => Some(b'o'), 0x19 => Some(b'p'),
        0x1E => Some(b'a'), 0x1F => Some(b's'), 0x20 => Some(b'd'), 0x21 => Some(b'f'),
        0x22 => Some(b'g'), 0x23 => Some(b'h'), 0x24 => Some(b'j'), 0x25 => Some(b'k'),
        0x26 => Some(b'l'),
        0x2C => Some(b'z'), 0x2D => Some(b'x'), 0x2E => Some(b'c'), 0x2F => Some(b'v'),
        0x30 => Some(b'b'), 0x31 => Some(b'n'), 0x32 => Some(b'm'),
        // Teclado numérico (Num Lock ON = dígitos).
        0x47 => Some(b'7'), 0x48 => Some(b'8'), 0x49 => Some(b'9'),
        0x4A => Some(b'-'),
        0x4B => Some(b'4'), 0x4C => Some(b'5'), 0x4D => Some(b'6'),
        0x4E => Some(b'+'),
        0x4F => Some(b'1'), 0x50 => Some(b'2'), 0x51 => Some(b'3'),
        0x52 => Some(b'0'), 0x53 => Some(b'.'),
        0x37 => Some(b'*'),
        // '/' del numpad: codigo propio (ver bmo_uhid) para no chocar
        // con la tecla 0x35, que en español es '-'.
        0x62 => Some(b'/'),
        _ => None,
    };
    if let Some(c) = common {
        if c.is_ascii_lowercase() {
            // AltGr+Q = @ en Latinoamérica (viene impreso en esa tecla).
            if altgr && c == b'q' && layout() == Layout::EsLatam {
                return Out::Ch(b'@');
            }
            // Las letras se ponen en mayúscula con Shift XOR Caps.
            return Out::Ch(if shift ^ caps { c - 32 } else { c });
        }
        return Out::Ch(c);
    }

    match layout() {
        Layout::Us => resolve_us(code, shift),
        Layout::EsLatam => resolve_es_latam(code, shift, altgr),
        Layout::EsSpain => resolve_es_spain(code, shift, altgr),
    }
}

fn resolve_us(code: u8, shift: bool) -> Out {
    let c = match code {
        0x02 => if shift { b'!' } else { b'1' },
        0x03 => if shift { b'@' } else { b'2' },
        0x04 => if shift { b'#' } else { b'3' },
        0x05 => if shift { b'$' } else { b'4' },
        0x06 => if shift { b'%' } else { b'5' },
        0x07 => if shift { b'^' } else { b'6' },
        0x08 => if shift { b'&' } else { b'7' },
        0x09 => if shift { b'*' } else { b'8' },
        0x0A => if shift { b'(' } else { b'9' },
        0x0B => if shift { b')' } else { b'0' },
        0x0C => if shift { b'_' } else { b'-' },
        0x0D => if shift { b'+' } else { b'=' },
        0x1A => if shift { b'{' } else { b'[' },
        0x1B => if shift { b'}' } else { b']' },
        0x27 => if shift { b':' } else { b';' },
        0x28 => if shift { b'"' } else { b'\'' },
        0x29 => if shift { b'~' } else { b'`' },
        0x2B => if shift { b'|' } else { b'\\' },
        0x33 => if shift { b'<' } else { b',' },
        0x34 => if shift { b'>' } else { b'.' },
        0x35 => if shift { b'?' } else { b'/' },
        0x56 => if shift { b'>' } else { b'<' },
        _ => return Out::Nothing,
    };
    Out::Ch(c)
}

/// Español LATINOAMERICANO — el teclado del usuario.
fn resolve_es_latam(code: u8, shift: bool, altgr: bool) -> Out {
    match code {
        0x02 => Out::Ch(if shift { b'!' } else { b'1' }),
        0x03 => Out::Ch(if altgr { b'@' } else if shift { b'"' } else { b'2' }),
        0x04 => Out::Ch(if shift { b'#' } else { b'3' }),
        0x05 => Out::Ch(if shift { b'$' } else { b'4' }),
        0x06 => Out::Ch(if shift { b'%' } else { b'5' }),
        0x07 => Out::Ch(if altgr { NOT_SIGN } else if shift { b'&' } else { b'6' }),
        0x08 => Out::Ch(if shift { b'/' } else { b'7' }),
        0x09 => Out::Ch(if shift { b'(' } else { b'8' }),
        0x0A => Out::Ch(if shift { b')' } else { b'9' }),
        0x0B => Out::Ch(if shift { b'=' } else { b'0' }),
        0x0C => Out::Ch(if altgr { b'\\' } else if shift { b'?' } else { b'\'' }),
        0x0D => Out::Ch(if shift { INV_EXCL } else { INV_QUES }),
        // Tecla muerta: acento agudo / diéresis.
        0x1A => Out::Dead(if shift { DIAER } else { ACUTE }),
        0x1B => Out::Ch(if altgr { b'~' } else if shift { b'*' } else { b'+' }),
        0x27 => Out::Ch(if shift { N_TILDE_MAY } else { N_TILDE_MIN }),
        0x28 => Out::Ch(if altgr { b'^' } else if shift { b'[' } else { b'{' }),
        0x29 => Out::Ch(if altgr { NOT_SIGN } else if shift { DEGREE } else { b'|' }),
        0x2B => Out::Ch(if altgr { b'`' } else if shift { b']' } else { b'}' }),
        0x33 => Out::Ch(if shift { b';' } else { b',' }),
        0x34 => Out::Ch(if shift { b':' } else { b'.' }),
        0x35 => Out::Ch(if shift { b'_' } else { b'-' }),
        0x56 => Out::Ch(if shift { b'>' } else { b'<' }),
        _ => Out::Nothing,
    }
}

/// Español de ESPAÑA.
fn resolve_es_spain(code: u8, shift: bool, altgr: bool) -> Out {
    match code {
        0x02 => Out::Ch(if altgr { b'|' } else if shift { b'!' } else { b'1' }),
        0x03 => Out::Ch(if altgr { b'@' } else if shift { b'"' } else { b'2' }),
        0x04 => Out::Ch(if altgr { b'#' } else if shift { MIDDOT } else { b'3' }),
        0x05 => Out::Ch(if altgr { b'~' } else if shift { b'$' } else { b'4' }),
        0x06 => Out::Ch(if shift { b'%' } else { b'5' }),
        0x07 => Out::Ch(if altgr { NOT_SIGN } else if shift { b'&' } else { b'6' }),
        0x08 => Out::Ch(if shift { b'/' } else { b'7' }),
        0x09 => Out::Ch(if shift { b'(' } else { b'8' }),
        0x0A => Out::Ch(if shift { b')' } else { b'9' }),
        0x0B => Out::Ch(if shift { b'=' } else { b'0' }),
        0x0C => Out::Ch(if altgr { b'\\' } else if shift { b'?' } else { b'\'' }),
        0x0D => Out::Ch(if altgr { b'~' } else if shift { INV_QUES } else { INV_EXCL }),
        // En España esta tecla es grave/circunflejo. Sin glifos para à ni â se
        // emiten directos (ambos son ASCII y útiles al programar) en vez de
        // fingir una combinación que no sabríamos dibujar.
        0x1A => Out::Ch(if altgr { b'[' } else if shift { b'^' } else { b'`' }),
        0x1B => Out::Ch(if altgr { b']' } else if shift { b'*' } else { b'+' }),
        0x27 => Out::Ch(if shift { N_TILDE_MAY } else { N_TILDE_MIN }),
        0x28 => if altgr { Out::Ch(b'{') } else { Out::Dead(if shift { DIAER } else { ACUTE }) },
        0x29 => Out::Ch(if altgr { b'\\' } else if shift { FEM_ORD } else { MASC_ORD }),
        0x2B => Out::Ch(if altgr { b'}' } else if shift { CEDIL_MAY } else { CEDIL_MIN }),
        0x33 => Out::Ch(if shift { b';' } else { b',' }),
        0x34 => Out::Ch(if shift { b':' } else { b'.' }),
        0x35 => Out::Ch(if shift { b'_' } else { b'-' }),
        0x56 => Out::Ch(if shift { b'>' } else { b'<' }),
        _ => Out::Nothing,
    }
}
