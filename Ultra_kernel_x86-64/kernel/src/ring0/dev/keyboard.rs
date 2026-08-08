//! Teclado: scancodes Set 1 -> caracteres, con distribucion ESPANOLA.
//!
//! Dos productores entran por aqui: el i8042 (PS/2, muerto post-EBS en esta
//! placa) y el puente USB HID (`dev::usb`), que traduce sus reportes a los
//! mismos scancodes Set 1. Un solo sitio decide que letra es cada tecla.
//!
//! El scancode dice QUE TECLA se pulso, nunca que letra es: eso lo decide la
//! distribucion impresa en el teclado. La tabla US daba `;` donde el teclado
//! del usuario tiene `n`, y `-` donde tiene `'`. Aqui viven US, espanol
//! latinoamericano y espanol de Espana, y se cambian en caliente con el
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
/// el caso efectivo de una letra es Shift XOR Caps; los simbolos ignoran Caps.
static mut CAPS: bool = false;
/// AltGr (Alt derecho) mantenido: el tercer nivel del teclado espanol, donde
/// viven @ # \ | { } [ ] ~ -- todo lo que hace falta para programar.
static mut ALTGR: bool = false;
/// Ctrl mantenido: convierte las letras en codigos de control (Ctrl+A = 0x01),
/// que es como los terminales han mandado ordenes de edicion desde siempre.
static mut CTRL: bool = false;
/// Bloq Num. Arranca ENCENDIDO (el teclado numerico escribe digitos), como
/// cualquier PC.
static mut NUMLOCK: bool = true;

// -- Teclas que no son caracteres --------------------------------------------
//
// Van por la misma cola que las letras, con bytes del rango C1 de Latin-1
// (0x80..0x9F): ese rango no tiene glifo ni significado imprimible, asi que el
// shell los reconoce sin ambiguedad y nunca se dibujan por error.

pub const KEY_UP: u8 = 0x80;
pub const KEY_DOWN: u8 = 0x81;
pub const KEY_LEFT: u8 = 0x82;
pub const KEY_RIGHT: u8 = 0x83;
pub const KEY_HOME: u8 = 0x84;
pub const KEY_END: u8 = 0x85;
pub const KEY_DELETE: u8 = 0x86;
pub const KEY_PGUP: u8 = 0x87;
pub const KEY_PGDN: u8 = 0x88;

// -- Las teclas de funcion ---------------------------------------------------
//
// Set 1 les da 0x3B..0x44 mas 0x57 y 0x58, pero **no producian nada**: la
// distribucion no las resolvia a ningun byte, asi que llegaban al kernel y
// morian ahi. Un hueco limpio, y el mejor sitio del teclado para un atajo del
// sistema por una razon concreta:
//
// * **Una tecla de funcion no produce caracter en NINGUNA distribucion.** No
//   puede chocar con escribir. Cualquier combinacion con `Ctrl+Alt` si puede --
//   en espanol `Ctrl+Alt` *es* AltGr, lo que da `@ # [ ] \ | EUR`, y el
//   compositor ya lleva una danza entera (disparar al soltar, y solo si no
//   llego ningun caracter mientras tanto) para que su atajo no rompa nada.
//   Encadenar otro combo encima empeoraria justo lo que costo arreglar.
//
// Van en el mismo rango C1 que la navegacion, detras de ella.
pub const KEY_F1: u8 = 0x89;
pub const KEY_F2: u8 = 0x8A;
pub const KEY_F3: u8 = 0x8B;
pub const KEY_F4: u8 = 0x8C;
pub const KEY_F5: u8 = 0x8D;
pub const KEY_F6: u8 = 0x8E;
pub const KEY_F7: u8 = 0x8F;
pub const KEY_F8: u8 = 0x90;
pub const KEY_F9: u8 = 0x91;
pub const KEY_F10: u8 = 0x92;
pub const KEY_F11: u8 = 0x93;
pub const KEY_F12: u8 = 0x94;

/// Es una tecla de navegacion (no imprimible)?
pub fn is_nav(b: u8) -> bool { (KEY_UP..=KEY_PGDN).contains(&b) }

/// Es una tecla de funcion?
pub fn is_funcion(b: u8) -> bool { (KEY_F1..=KEY_F12).contains(&b) }

// -- LEDs --------------------------------------------------------------------

pub const LED_NUM: u8 = 1 << 0;
pub const LED_CAPS: u8 = 1 << 1;
pub const LED_SCROLL: u8 = 1 << 2;

/// Como deberian estar las tres lucecitas AHORA. El teclado no lo decide solo:
/// hay que mandarselo (ver `UsbHidHal::set_leds`).
pub fn led_mask() -> u8 {
    let mut m = 0;
    unsafe {
        if NUMLOCK { m |= LED_NUM; }
        if CAPS { m |= LED_CAPS; }
    }
    m
}

/// Estado de los bloqueos para pintarlo en pantalla -- que las luces fisicas
/// funcionen o no no deberia ser la unica forma de saberlo.
pub fn lock_state() -> (bool, bool) { unsafe { (CAPS, NUMLOCK) } }

// ===========================================================================
//  Distribuciones
// ===========================================================================

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Layout {
    /// US QWERTY (la de siempre).
    Us,
    /// Espanol LATINOAMERICANO -- la del teclado del usuario (SEISA). Se
    /// distingue de la de Espana en que `{ } [ ]` estan en las teclas de la
    /// derecha y `@` es AltGr+Q.
    EsLatam,
    /// Espanol de ESPANA: c junto al Enter, o/a a la izquierda del 1,
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

// Bytes Latin-1 que produce el teclado espanol (el font los dibuja: ver
// toolchain/tools/fontgen). Un caracter = un byte, sin UTF-8 en Ring 0.
const N_TILDE_MIN: u8 = 0xF1; // n
const N_TILDE_MAY: u8 = 0xD1; // N
const ACUTE: u8 = 0xB4;       // '  (tecla muerta)
const DIAER: u8 = 0xA8;       // "  (tecla muerta)
const INV_EXCL: u8 = 0xA1;    // 
const INV_QUES: u8 = 0xBF;    // 
const MASC_ORD: u8 = 0xBA;    // o
const FEM_ORD: u8 = 0xAA;     // a
const DEGREE: u8 = 0xB0;      //  deg
const MIDDOT: u8 = 0xB7;      // -
const NOT_SIGN: u8 = 0xAC;    // !
const CEDIL_MIN: u8 = 0xE7;   // c
const CEDIL_MAY: u8 = 0xC7;   // C

// AltGr llega como scancode propio desde el puente USB (`bmo_uhid::SC_ALTGR`);
// Set 1 lo expresa como `0xE0 0x38` y por el PS/2 se detecta con ese prefijo
// (ver `poll_event`). Una sola definicion del codigo, en el productor.

/// Resultado de resolver una tecla.
enum Out {
    /// Nada que emitir (modificador, tecla sin significado en el shell).
    Nothing,
    /// Un caracter listo.
    Ch(u8),
    /// TECLA MUERTA: no imprime, espera a la siguiente para combinarse
    /// (' + a = a). Lleva el byte del signo por si al final no combina.
    Dead(u8),
}

// ===========================================================================
//  Cola de salida
// ===========================================================================
//
// Una pulsacion puede producir DOS caracteres (una tecla muerta que no combina
// emite el acento y luego la letra), y un solo sondeo del HID puede traer
// varias teclas a la vez. Antes se devolvia "la ultima, y las demas se
// pierden" -- escribir rapido se comia letras. Ahora todo entra en esta cola y
// el shell la vacia a su ritmo.

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

/// Saca el siguiente caracter pendiente, si lo hay.
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
/// combinacion valida (entonces se emiten los dos por separado).
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
/// Punto unico por el que pasan tanto el teclado USB como el PS/2.
pub(crate) fn feed(code: u8, shift: bool, altgr: bool, caps: bool) {
    feed_full(code, shift, altgr, caps, unsafe { CTRL })
}

/// Igual que `feed` pero con el estado de Ctrl explicito.
pub(crate) fn feed_full(code: u8, shift: bool, altgr: bool, caps: bool, ctrl: bool) {
    // Bloq Num: alterna, y de paso mantiene el LED sincronizado.
    if code == 0x45 {
        unsafe { NUMLOCK = !NUMLOCK; }
        return;
    }
    // Teclas de navegacion: no son caracteres, van tal cual por la cola.
    if let Some(nav) = nav_key(code) {
        push_out(nav);
        return;
    }
    // Con Bloq Num APAGADO el teclado numerico es de navegacion, como en
    // cualquier PC -- el segundo oficio que tienen impreso esas teclas.
    if !unsafe { NUMLOCK } {
        let nav = match code {
            0x47 => Some(KEY_HOME), 0x48 => Some(KEY_UP),   0x49 => Some(KEY_PGUP),
            0x4B => Some(KEY_LEFT), 0x4D => Some(KEY_RIGHT),
            0x4F => Some(KEY_END),  0x50 => Some(KEY_DOWN), 0x51 => Some(KEY_PGDN),
            0x53 => Some(KEY_DELETE),
            _ => None,
        };
        if let Some(n) = nav { push_out(n); return; }
    }
    // Ctrl + letra = codigo de control ASCII (Ctrl+A = 0x01, Ctrl+U = 0x15...).
    // Es la convencion de toda la vida de los terminales y no necesita un
    // canal aparte: cabe en el mismo byte que las letras.
    if ctrl {
        if let Out::Ch(c) = resolve(code, false, false, false) {
            if c.is_ascii_alphabetic() {
                push_out(c.to_ascii_uppercase() - b'A' + 1);
                return;
            }
        }
    }
    let out = resolve(code, shift, altgr, caps);
    unsafe {
        let pending = DEAD_PENDING;
        match out {
            Out::Nothing => {}
            Out::Dead(sign) => {
                // Dos signos muertos seguidos: el primero se imprime tal cual
                // (asi se escribe un acento suelto).
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
/// the stream on screen -- the difference between "no bytes at all" (legacy
/// emulation dead post-EBS) and "bytes in an unexpected scancode set"
/// (translation problem) in one look. Never blocks.
pub fn poll_event() -> Option<(u8, Option<u8>)> {
    let status = inb(STATUS);
    if status & 0x01 == 0 {
        return None; // output buffer empty -- no byte waiting
    }
    if status & 0x20 != 0 {
        // Bit 5 set = second-port (mouse) byte. Drain it and ignore so it
        // does not desync the keyboard stream.
        let _ = inb(DATA);
        return None;
    }
    let code = inb(DATA);
    // Prefijo 0xE0: la tecla siguiente es "extendida" (AltGr, Ctrl derecho,
    // flechas...). Se recuerda para el proximo byte -- asi AltGr (0xE0 0x38)
    // no se confunde con el Alt izquierdo (0x38 a secas).
    static mut E0: bool = false;
    let ext = unsafe { let e = E0; E0 = false; e };
    match code {
        0xE0 => { unsafe { E0 = true; } }
        0x38 if ext => { unsafe { ALTGR = true; } }
        0xB8 if ext => { unsafe { ALTGR = false; } }
        0x2A | 0x36 => { unsafe { SHIFT = true; } }
        0xAA | 0xB6 => { unsafe { SHIFT = false; } }
        0x1D => { unsafe { CTRL = true; } }
        0x9D => { unsafe { CTRL = false; } }
        0x3A => { unsafe { CAPS = !CAPS; } }
        c if c & 0x80 != 0 => {} // cualquier otro release
        c => feed(c, unsafe { SHIFT }, unsafe { ALTGR }, unsafe { CAPS }),
    }
    Some((code, pop_out()))
}

/// Vista "solo caracter" del teclado PS/2. Primero vacia lo que quedo en la
/// cola (una tecla puede haber producido dos caracteres).
pub fn poll_ascii() -> Option<u8> {
    if let Some(b) = pop_out() { return Some(b); }
    poll_event().and_then(|(_, a)| a)
}

/// Traduce los scancodes propios de navegacion (ver `bmo_uhid`) al byte que
/// viaja por la cola. `None` si la tecla no es de navegacion.
fn nav_key(code: u8) -> Option<u8> {
    Some(match code {
        c if c == bmo_uhid::SC_UP => KEY_UP,
        c if c == bmo_uhid::SC_DOWN => KEY_DOWN,
        c if c == bmo_uhid::SC_LEFT => KEY_LEFT,
        c if c == bmo_uhid::SC_RIGHT => KEY_RIGHT,
        c if c == bmo_uhid::SC_HOME => KEY_HOME,
        c if c == bmo_uhid::SC_END => KEY_END,
        c if c == bmo_uhid::SC_DELETE => KEY_DELETE,
        c if c == bmo_uhid::SC_PGUP => KEY_PGUP,
        c if c == bmo_uhid::SC_PGDN => KEY_PGDN,
        // Las de funcion traen su scancode Set 1 de siempre: `hid_to_ps2` ya
        // las traducia y aqui se caian por el `_ => None`.
        0x3B => KEY_F1,
        0x3C => KEY_F2,
        0x3D => KEY_F3,
        0x3E => KEY_F4,
        0x3F => KEY_F5,
        0x40 => KEY_F6,
        0x41 => KEY_F7,
        0x42 => KEY_F8,
        0x43 => KEY_F9,
        0x44 => KEY_F10,
        0x57 => KEY_F11,
        0x58 => KEY_F12,
        _ => return None,
    })
}

/// Resuelve un scancode Set 1 al caracter que le corresponde SEGUN LA
/// DISTRIBUCION ACTIVA. `caps` invierte el caso solo de letras (como Windows).
fn resolve(code: u8, shift: bool, altgr: bool, caps: bool) -> Out {
    // Comun a todas las distribuciones: letras, control y teclado numerico.
    // La posicion de las letras es la misma en US, Espana y Latinoamerica
    // (todas QWERTY); lo que cambia es la puntuacion.
    let common = match code {
        // ** ESC. Y esta linea faltaba, con TRES cosas colgando de ella.
        //
        // El scancode 0x01 no estaba en NINGUNA tabla --ni aqui, ni en
        // `nav_key`, ni en las tres distribuciones-- asi que `resolve`
        // contestaba `Out::Nothing` y **el byte 27 no existia en el sistema**.
        //
        // Encima de una tecla que no llegaba habia tres cosas escritas:
        //
        //   1. `ESC cierra` en el pie de las ventanas del escritorio, que
        //      compara contra `0x1B`.
        //   2. `if (tecla == 27) vivo = 0;` en el raycaster -- su unica salida.
        //   3. **El rescate `Ctrl+Alt+ESC`**, que empieza por `let b = t?`: sin
        //      byte, sale por el `?` y no llega ni a mirar los modificadores.
        //
        // Las tres se leian como fallos distintos. La 3 se probo en metal el
        // 2026-08-08 y no respondio; buscando por que, salio que el fallo no
        // estaba en el rescate sino tres capas mas abajo, en una fila de tabla
        // que nadie escribio.
        //
        // Va en `common` --antes de la distribucion-- a proposito: ESC es la
        // misma tecla en US, en Espana y en Latinoamerica, y ponerla aqui la
        // deja fuera del nivel de AltGr. Eso importa, porque en la
        // distribucion espanola `Ctrl+Alt` ES AltGr (ver `altgr_active`), y si
        // ESC dependiera del nivel, el atajo de rescate seria justo la
        // combinacion que lo apaga.
        0x01 => Some(27),    // ESC
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
        // Teclado numerico (Num Lock ON = digitos).
        0x47 => Some(b'7'), 0x48 => Some(b'8'), 0x49 => Some(b'9'),
        0x4A => Some(b'-'),
        0x4B => Some(b'4'), 0x4C => Some(b'5'), 0x4D => Some(b'6'),
        0x4E => Some(b'+'),
        0x4F => Some(b'1'), 0x50 => Some(b'2'), 0x51 => Some(b'3'),
        0x52 => Some(b'0'), 0x53 => Some(b'.'),
        0x37 => Some(b'*'),
        // '/' del numpad: codigo propio (ver bmo_uhid) para no chocar
        // con la tecla 0x35, que en espanol es '-'.
        0x62 => Some(b'/'),
        _ => None,
    };
    if let Some(c) = common {
        if c.is_ascii_lowercase() {
            // AltGr+Q = @ en Latinoamerica (viene impreso en esa tecla).
            if altgr && c == b'q' && layout() == Layout::EsLatam {
                return Out::Ch(b'@');
            }
            // Las letras se ponen en mayuscula con Shift XOR Caps.
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

/// Espanol LATINOAMERICANO -- el teclado del usuario.
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
        // Tecla muerta: acento agudo / dieresis.
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

/// Espanol de ESPANA.
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
        // En Espana esta tecla es grave/circunflejo. Sin glifos para la a con
        // acento grave ni para la a con circunflejo, los dos acentos se
        // emiten directos (ambos son ASCII y utiles al programar) en vez de
        // fingir una combinacion que no sabriamos dibujar.
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
