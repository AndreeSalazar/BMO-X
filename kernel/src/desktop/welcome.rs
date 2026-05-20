//! Welcome screen — pantalla profesional de bienvenida en framebuffer.
//!
//! Layout:
//!
//! ```text
//!   ╔══════════════════════════════════════════════════════════╗
//!   ║                                                          ║
//!   ║                     FastOS / BMO                         ║
//!   ║                  Bare Metal Orchestrator                 ║
//!   ║                       v0.9.0                             ║
//!   ║                                                          ║
//!   ║   ✓  Ring 0 + Ring 3 listos                              ║
//!   ║   ✓  13 syscalls BMO activos                             ║
//!   ║   ✓  Compositor Ring 0 cargado                           ║
//!   ║   ✓  Mouse PS/2 + Beep PC speaker                        ║
//!   ║   ✓  RAMdisk con file I/O                                ║
//!   ║                                                          ║
//!   ║   Escribe (Run) y pulsa Enter para entrar al escritorio:║
//!   ║                                                          ║
//!   ║       > _                                                ║
//!   ║                                                          ║
//!   ╚══════════════════════════════════════════════════════════╝
//! ```
//!
//! Comandos aceptados (case-insensitive):
//!   - `Run`   → lanza el escritorio Ring 3 (`spawn_desktop`)
//!   - `Hello` → lanza el payload mínimo (`spawn_hello`)
//!   - `Reboot` → reinicia
//!   - cualquier otra cosa → muestra hint

use crate::boot_info;
use crate::fb::Framebuffer;
use crate::font;
use crate::desktop;
use crate::sched::user_init;

// ── Paleta del welcome ─────────────────────────────────────────────
mod pal {
    pub const BG_TOP:    u32 = 0xFF0E1729;
    pub const BG_BOT:    u32 = 0xFF1F1145;

    pub const CARD_BG:   u32 = 0xFF1A2238;
    pub const CARD_BD:   u32 = 0xFF3A4878;
    pub const CARD_SHADOW: u32 = 0xFF040611;

    pub const TITLE:     u32 = 0xFFE6EDF3;
    pub const ACCENT:    u32 = 0xFF76B900;     // BMO green
    pub const SUBTITLE:  u32 = 0xFF8B949E;
    pub const VERSION:   u32 = 0xFF56D4DD;

    pub const OK_FG:     u32 = 0xFF27C93F;
    pub const ITEM:      u32 = 0xFFCBD2DB;
    pub const PROMPT_BG: u32 = 0xFF0B1224;
    pub const PROMPT_BD: u32 = 0xFF56D4DD;
    pub const PROMPT_FG: u32 = 0xFFE6EDF3;
    pub const HINT:      u32 = 0xFFFFBD2E;

    pub const RUN_BTN:   u32 = 0xFF0078D4;
    pub const RUN_BTN_HI:u32 = 0xFF1A8BE0;
}

// ── State del welcome ──────────────────────────────────────────────

const MAX_INPUT: usize = 32;
static mut INPUT_BUF: [u8; MAX_INPUT] = [0; MAX_INPUT];
static mut INPUT_LEN: usize = 0;
static mut HINT_TIMER: u32 = 0;        // frames mostrando hint
static mut HINT_MSG: &[u8] = b"";

// ── Modificadores de teclado ───────────────────────────────────────
static mut KBD_LSHIFT: bool = false;
static mut KBD_RSHIFT: bool = false;
static mut KBD_CAPS:   bool = false;

#[inline]
fn shift_held() -> bool { unsafe { KBD_LSHIFT || KBD_RSHIFT } }

#[inline]
fn caps_on() -> bool { unsafe { KBD_CAPS } }

/// Letras: si `shift XOR caps`, mayúscula. Otros símbolos: shift selecciona el
/// glifo superior. Devuelve `None` para teclas sin texto.
fn translate_scancode(sc: u8) -> Option<u8> {
    // Base (sin shift) — PS/2 Set 1 US-ASCII estándar.
    let (base, shifted): (u8, u8) = match sc {
        0x29 => (b'`',  b'~'),
        0x02 => (b'1',  b'!'),
        0x03 => (b'2',  b'@'),
        0x04 => (b'3',  b'#'),
        0x05 => (b'4',  b'$'),
        0x06 => (b'5',  b'%'),
        0x07 => (b'6',  b'^'),
        0x08 => (b'7',  b'&'),
        0x09 => (b'8',  b'*'),
        0x0A => (b'9',  b'('),
        0x0B => (b'0',  b')'),
        0x0C => (b'-',  b'_'),
        0x0D => (b'=',  b'+'),
        0x0F => (b'\t', b'\t'),
        0x10 => (b'q',  b'Q'),
        0x11 => (b'w',  b'W'),
        0x12 => (b'e',  b'E'),
        0x13 => (b'r',  b'R'),
        0x14 => (b't',  b'T'),
        0x15 => (b'y',  b'Y'),
        0x16 => (b'u',  b'U'),
        0x17 => (b'i',  b'I'),
        0x18 => (b'o',  b'O'),
        0x19 => (b'p',  b'P'),
        0x1A => (b'[',  b'{'),
        0x1B => (b']',  b'}'),
        0x1E => (b'a',  b'A'),
        0x1F => (b's',  b'S'),
        0x20 => (b'd',  b'D'),
        0x21 => (b'f',  b'F'),
        0x22 => (b'g',  b'G'),
        0x23 => (b'h',  b'H'),
        0x24 => (b'j',  b'J'),
        0x25 => (b'k',  b'K'),
        0x26 => (b'l',  b'L'),
        0x27 => (b';',  b':'),
        0x28 => (b'\'', b'"'),
        0x2B => (b'\\', b'|'),
        0x2C => (b'z',  b'Z'),
        0x2D => (b'x',  b'X'),
        0x2E => (b'c',  b'C'),
        0x2F => (b'v',  b'V'),
        0x30 => (b'b',  b'B'),
        0x31 => (b'n',  b'N'),
        0x32 => (b'm',  b'M'),
        0x33 => (b',',  b'<'),
        0x34 => (b'.',  b'>'),
        0x35 => (b'/',  b'?'),
        0x39 => (b' ',  b' '),
        0x1C => (b'\n', b'\n'),
        0x0E => (8,     8),
        _ => return None,
    };

    let is_letter = base.is_ascii_lowercase();
    let upper = if is_letter {
        // Letras: shift XOR caps → mayúscula.
        shift_held() ^ caps_on()
    } else {
        shift_held()
    };
    Some(if upper { shifted } else { base })
}

/// Procesa un scancode crudo y, si es modifier, actualiza el estado;
/// si es tecla normal pulsada, devuelve el carácter a insertar.
fn process_scancode(raw: u8) -> Option<u8> {
    let released = (raw & 0x80) != 0;
    let sc = raw & 0x7F;

    // Modificadores
    match sc {
        0x2A => { unsafe { KBD_LSHIFT = !released; } return None; }
        0x36 => { unsafe { KBD_RSHIFT = !released; } return None; }
        0x3A => {
            if !released { unsafe { KBD_CAPS = !KBD_CAPS; } }
            return None;
        }
        0x1D | 0x38 => return None, // Ctrl / Alt — ignorados aquí
        _ => {}
    }

    if released { return None; }
    translate_scancode(sc)
}

// ── Drawing helpers ────────────────────────────────────────────────

fn fb() -> Option<Framebuffer> {
    let (addr, w, h, s) = unsafe {
        (boot_info::FB_ADDR, boot_info::FB_WIDTH, boot_info::FB_HEIGHT, boot_info::FB_STRIDE)
    };
    if addr == 0 || w == 0 { return None; }
    Some(Framebuffer::new(addr, (s as u64) * 4, w, h))
}

/// Dibuja texto. Soporta escala 1×, 2× y 3× (replicando píxeles).
fn draw_text_scaled(fb: &Framebuffer, x: u32, y: u32, text: &[u8], color: u32, scale: u32) {
    let mut cx = x as usize;
    let cy = y as usize;
    let s = scale.max(1) as usize;
    let gw = 8 * s;
    let gh = 16 * s;
    for &ch in text {
        if cx + gw > fb.width || cy + gh > fb.height { break; }
        let glyph = font::get_glyph(ch);
        for py in 0..16 {
            let row = glyph[py];
            for px in 0..8 {
                if (row & (0x80 >> px)) != 0 {
                    for ry in 0..s {
                        for rx in 0..s {
                            fb.put_pixel(cx + px * s + rx, cy + py * s + ry, color);
                        }
                    }
                }
            }
        }
        cx += gw;
    }
}

#[inline]
fn draw_text(fb: &Framebuffer, x: u32, y: u32, text: &[u8], color: u32) {
    draw_text_scaled(fb, x, y, text, color, 1);
}

// ── Render del welcome ─────────────────────────────────────────────

fn render(fb: &Framebuffer) {
    // 1) Wallpaper gradient
    fb.gradient_v(0, 0, fb.width, fb.height, pal::BG_TOP, pal::BG_BOT);

    // 2) Card centrado
    let cw = 980usize;
    let ch = 620usize;
    let cx = (fb.width - cw) / 2;
    let cy = (fb.height - ch) / 2;

    // sombra
    fb.fill_rounded_rect(cx + 10, cy + 14, cw, ch, 24, pal::CARD_SHADOW);
    // cuerpo
    fb.fill_rounded_rect(cx, cy, cw, ch, 24, pal::CARD_BG);
    fb.draw_rect(cx, cy, cw, ch, pal::CARD_BD, 2);

    // 3) Header — "FastOS / BMO" en grande
    let title = b"FastOS / BMO";
    let tw = title.len() * 8 * 3;
    let tx = cx + (cw - tw) / 2;
    draw_text_scaled(fb, tx as u32, (cy + 60) as u32, title, pal::TITLE, 3);

    // 4) Subtítulo
    let sub = b"Bare Metal Orchestrator";
    let sw = sub.len() * 8 * 2;
    let sx = cx + (cw - sw) / 2;
    draw_text_scaled(fb, sx as u32, (cy + 120) as u32, sub, pal::SUBTITLE, 2);

    // 5) Versión
    let ver = b"v0.9.0  ::  Ring 0 + Ring 3";
    let vw = ver.len() * 8;
    let vx = cx + (cw - vw) / 2;
    draw_text(fb, vx as u32, (cy + 170) as u32, ver, pal::VERSION);

    // 6) Linea divisoria
    fb.fill_rect(cx + 60, cy + 200, cw - 120, 1, pal::CARD_BD);

    // 7) Lista de status
    let items: [&[u8]; 5] = [
        b"  [OK]  Ring 0 + Ring 3 activos",
        b"  [OK]  13 syscalls BMO operativos",
        b"  [OK]  Compositor Ring 0 cargado",
        b"  [OK]  Mouse PS/2 + Beep PC speaker",
        b"  [OK]  RAMdisk + FileOpen/Read/Close",
    ];
    let mut iy = cy + 220;
    for it in items {
        draw_text(fb, (cx + 80) as u32, iy as u32, it, pal::ITEM);
        // marker verde en la palabra [OK]
        draw_text(fb, (cx + 80) as u32, iy as u32, b"  [OK]", pal::OK_FG);
        iy += 24;
    }

    // 8) Prompt box
    let pw = 720usize;
    let ph = 60usize;
    let px = cx + (cw - pw) / 2;
    let py = cy + ch - ph - 110;

    // hint sobre el prompt
    let hint = b"Escribe (Run) y pulsa Enter para entrar al escritorio:";
    let hx = px;
    draw_text(fb, hx as u32, (py - 28) as u32, hint, pal::TITLE);

    // caja
    fb.fill_rounded_rect(px, py, pw, ph, 10, pal::PROMPT_BG);
    fb.draw_rect(px, py, pw, ph, pal::PROMPT_BD, 2);

    // prompt "> " + input + caret blinking
    draw_text_scaled(fb, (px + 16) as u32, (py + 18) as u32, b"> ", pal::ACCENT, 2);

    // Caret a ~1.5 Hz (parpadeo agradable, no epiléptico).
    let (len, frame_blink) = unsafe { (INPUT_LEN, (crate::arch::cpu::rdtsc() / 1_250_000_000) as u32 & 1) };
    if len > 0 {
        let txt = unsafe { &INPUT_BUF[..len] };
        draw_text_scaled(fb, (px + 16 + 8 * 2 * 2) as u32, (py + 18) as u32, txt, pal::PROMPT_FG, 2);
    }
    if frame_blink != 0 {
        let cx_caret = px + 16 + 8 * 2 * 2 + len * 16;
        fb.fill_rect(cx_caret, py + 20, 12, 24, pal::PROMPT_FG);
    }

    // 9) Hint (si hay)
    let (timer, msg) = unsafe { (HINT_TIMER, HINT_MSG) };
    if timer > 0 && !msg.is_empty() {
        draw_text(fb, hx as u32, (py + ph + 16) as u32, msg, pal::HINT);
    }

    // 10) Botón "RUN" visual a la derecha del prompt
    let btn_w = 120usize;
    let btn_h = 60usize;
    let bx = px + pw - btn_w;
    let by = py;
    let btn_color = if unsafe { INPUT_LEN > 0 && {
        let s = &INPUT_BUF[..INPUT_LEN];
        eq_ci(s, b"run")
    } } { pal::RUN_BTN_HI } else { pal::RUN_BTN };
    fb.fill_rounded_rect(bx, by, btn_w, btn_h, 10, btn_color);
    fb.draw_rect(bx, by, btn_w, btn_h, pal::CARD_BD, 2);
    let lbl = b"RUN";
    let lw = lbl.len() * 8 * 3;
    let lx = bx + (btn_w - lw) / 2;
    draw_text_scaled(fb, lx as u32, (by + 12) as u32, lbl, pal::TITLE, 3);

    // 11) Pie
    let foot = b"FastOS / BMO  ::  Ryzen 5 5600X  ::  RTX 3060 12G  ::  UEFI";
    let fw = foot.len() * 8;
    let fx = cx + (cw - fw) / 2;
    draw_text(fb, fx as u32, (cy + ch - 36) as u32, foot, pal::SUBTITLE);
}

// ── Helpers ────────────────────────────────────────────────────────

fn eq_ci(a: &[u8], b: &[u8]) -> bool {
    if a.len() != b.len() { return false; }
    for i in 0..a.len() {
        let ca = a[i].to_ascii_lowercase();
        let cb = b[i].to_ascii_lowercase();
        if ca != cb { return false; }
    }
    true
}

fn show_hint(msg: &'static [u8]) {
    unsafe {
        HINT_MSG = msg;
        HINT_TIMER = 120; // ~2 seg a 60 FPS
    }
}

fn process_enter() {
    let cmd = unsafe { &INPUT_BUF[..INPUT_LEN] };
    let trimmed = trim(cmd);

    if trimmed.is_empty() {
        show_hint(b"Escribe (Run) y Enter.");
    } else if eq_ci(trimmed, b"run") {
        // Beep de confirmación y arrancar el escritorio
        desktop::beep(880, 80);
        desktop::beep(1320, 80);
        // No vuelve (spawn_desktop es noreturn)
        user_init::spawn_desktop();
    } else if eq_ci(trimmed, b"hello") {
        desktop::beep(440, 80);
        user_init::spawn_hello();
    } else if eq_ci(trimmed, b"reboot") {
        unsafe { core::arch::asm!("out dx, al", in("dx") 0x64u16, in("al") 0xFEu8); }
    } else {
        show_hint(b"Comando desconocido. Usa: Run, Hello, Reboot.");
    }
    unsafe { INPUT_LEN = 0; }
}

fn trim(s: &[u8]) -> &[u8] {
    let mut i = 0;
    while i < s.len() && s[i] == b' ' { i += 1; }
    let mut j = s.len();
    while j > i && s[j-1] == b' ' { j -= 1; }
    &s[i..j]
}

// ── Main loop ──────────────────────────────────────────────────────

pub fn run() -> ! {
    crate::drivers::serial::serial_write("[welcome] Pantalla de bienvenida activa.\n");

    // Beep suave de arranque
    desktop::beep(523, 40);
    desktop::beep(659, 40);
    desktop::beep(784, 80);

    loop {
        if let Some(fb) = fb() {
            render(&fb);
        }

        // Dormir ~32 ms (≈30 FPS — suficiente para input)
        let cycles = 32u64 * 3_700_000;
        let start = crate::arch::cpu::rdtsc();
        while (crate::arch::cpu::rdtsc() - start) < cycles {
            // Drenar todas las teclas disponibles (no perder pulsaciones).
            // Pasamos el scancode tal cual al procesador: él se encarga de
            // distinguir press/release y de actualizar Shift/Caps/Ctrl/Alt.
            let sc = desktop::poll_key();
            if sc != 0 {
                if let Some(ch) = process_scancode(sc) {
                    handle_char(ch);
                }
            }
            core::hint::spin_loop();
        }

        unsafe { if HINT_TIMER > 0 { HINT_TIMER -= 1; } }
    }
}

fn handle_char(ch: u8) {
    match ch {
        b'\n' => process_enter(),
        8 => unsafe { if INPUT_LEN > 0 { INPUT_LEN -= 1; } },
        c if c >= 32 && c <= 126 => unsafe {
            if INPUT_LEN < MAX_INPUT - 1 {
                INPUT_BUF[INPUT_LEN] = c;
                INPUT_LEN += 1;
            }
        },
        _ => {}
    }
}
