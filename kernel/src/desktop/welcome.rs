//! Welcome screen â€” pantalla profesional de bienvenida en framebuffer.
//!
//! Layout:
//!
//! ```text
//!   â•”â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•—
//!   â•‘                                                          â•‘
//!   â•‘                     FastOS / BMO                         â•‘
//!   â•‘                  Bare Metal Orchestrator                 â•‘
//!   â•‘                       v0.9.0                             â•‘
//!   â•‘                                                          â•‘
//!   â•‘   âœ“  Ring 0 + Ring 3 listos                              â•‘
//!   â•‘   âœ“  13 syscalls BMO activos                             â•‘
//!   â•‘   âœ“  Compositor Ring 0 cargado                           â•‘
//!   â•‘   âœ“  Mouse PS/2 + Beep PC speaker                        â•‘
//!   â•‘   âœ“  RAMdisk con file I/O                                â•‘
//!   â•‘                                                          â•‘
//!   â•‘   Escribe (Run) y pulsa Enter para entrar al escritorio: â•‘
//!   â•‘                                                          â•‘
//!   â•‘       > _                                                â•‘
//!   â•‘                                                          â•‘
//!   â•šâ•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•
//! ```
//!
//! Comandos aceptados (case-insensitive):
//!   - `Run`   â†’ lanza el escritorio estable: Ring 0 supervisor + Ring 3 preparado
//!   - `Hello` â†’ lanza el payload mÃ­nimo (`spawn_hello`)
//!   - `Reboot` â†’ reinicia
//!   - cualquier otra cosa â†’ muestra hint

use crate::boot_info;
use crate::ui::fb::Framebuffer;
use crate::ui::font;
use super::commands::{eq_ci, trim, should_enter_desktop, enter_desktop, nexo_test_compile};
use super::sound;

// â”€â”€ Paleta del welcome â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€
mod pal {
    // v1.6.7 palette: modern dark cyan/teal with warm orange accents.
    // Wallpaper is a dark teal->indigo gradient. Card body is a slightly
    // raised dark slate with a teal border. Accents use FastOS green for
    // OK badges and warm orange for hints and the Run button hover.
    pub const BG_TOP:    u32 = 0xFF050B12;     // near-black with cool tint
    pub const BG_BOT:    u32 = 0xFF0E1B2E;     // dark indigo

    pub const CARD_BG:   u32 = 0xFF0F1827;     // raised slate
    pub const CARD_BD:   u32 = 0xFF1F4D5C;     // teal border
    pub const CARD_SHADOW: u32 = 0xFF020610;   // deep shadow

    pub const TITLE:     u32 = 0xFFE6F1F5;     // near-white with cool tint
    pub const ACCENT:    u32 = 0xFF4ECCA3;     // BMO mint/teal-green
    pub const SUBTITLE:  u32 = 0xFF7B8FA1;     // cool gray
    pub const VERSION:   u32 = 0xFFE2C044;     // warm gold (draws eye)

    pub const OK_FG:     u32 = 0xFF4ECCA3;     // mint
    pub const OK_BG:     u32 = 0xFF0E2820;     // dark mint pill
    pub const ITEM:      u32 = 0xFFCBD7E0;     // soft white
    pub const PROMPT_BG: u32 = 0xFF070D17;     // deeper than card
    pub const PROMPT_BD: u32 = 0xFF4ECCA3;     // mint border
    pub const PROMPT_FG: u32 = 0xFFE6F1F5;
    pub const HINT:      u32 = 0xFFFFAA3D;     // warm orange

    pub const RUN_BTN:   u32 = 0xFFE07832;     // warm orange
    pub const RUN_BTN_HI:u32 = 0xFFFFA056;     // bright orange

    // Phase progress colors (used by the new phase progress bar)
    pub const PHASE_DONE:  u32 = 0xFF4ECCA3;
    pub const PHASE_CURR:  u32 = 0xFFE2C044;
    pub const PHASE_PEND:  u32 = 0xFF243140;
    pub const PHASE_BG:    u32 = 0xFF0A1018;
}

// â”€â”€ State del welcome â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€

const MAX_INPUT: usize = 32;
static mut INPUT_BUF: [u8; MAX_INPUT] = [0; MAX_INPUT];
static mut INPUT_LEN: usize = 0;
static mut HINT_TIMER: u32 = 0;        // frames mostrando hint
static mut HINT_MSG: &[u8] = b"";

// â”€â”€ Modificadores de teclado â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€
static mut KBD_LSHIFT: bool = false;
static mut KBD_RSHIFT: bool = false;
static mut KBD_CAPS:   bool = false;

#[inline]
fn shift_held() -> bool { unsafe { KBD_LSHIFT || KBD_RSHIFT } }

#[inline]
fn caps_on() -> bool { unsafe { KBD_CAPS } }

/// Letras: si `shift XOR caps`, mayÃºscula. Otros sÃ­mbolos: shift selecciona el
/// glifo superior. Devuelve `None` para teclas sin texto.
fn translate_scancode(sc: u8) -> Option<u8> {
    // Base (sin shift) â€” PS/2 Set 1 US-ASCII estÃ¡ndar.
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
        0x27 => (164,  165), // Ã± y Ã‘ (distribuciÃ³n espaÃ±ola)
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
        // Letras: shift XOR caps â†’ mayÃºscula.
        shift_held() ^ caps_on()
    } else {
        shift_held()
    };
    Some(if upper { shifted } else { base })
}

/// Procesa un scancode crudo y, si es modifier, actualiza el estado;
/// si es tecla normal pulsada, devuelve el carÃ¡cter a insertar.
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
        0x1D | 0x38 => return None, // Ctrl / Alt â€” ignorados aquÃ­
        _ => {}
    }

    if released { return None; }
    translate_scancode(sc)
}

// â”€â”€ Drawing helpers â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€

fn fb() -> Option<Framebuffer> {
    let (addr, w, h, s) = unsafe {
        (boot_info::FB_ADDR, boot_info::FB_WIDTH, boot_info::FB_HEIGHT, boot_info::FB_STRIDE)
    };
    if addr == 0 || w == 0 { return None; }
    // FB_STRIDE llega desde GOP en pÃ­xeles por lÃ­nea. `Framebuffer::new`
    // espera pitch en bytes, igual que el escritorio Ring 0. Si pasamos el
    // stride crudo, el welcome sÃ³lo pinta bien el fondo y el texto/tarjetas
    // quedan corruptos o fuera de sitio en hardware real.
    Some(Framebuffer::new(addr, (s as u64) * 4, w, h))
}

/// Dibuja texto. Soporta escala 1Ã—, 2Ã— y 3Ã— (replicando pÃ­xeles).
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

// â”€â”€ Render del welcome â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€

/// GeometrÃ­a fija del prompt â€” usada tanto por el render principal como
/// por el repaint local del caret. Devuelve `(prompt_x, prompt_y, w, h)`.
fn prompt_rect(fb: &Framebuffer) -> (usize, usize, usize, usize) {
    // v1.6.12: match the new card geometry (1100×540)
    let cw = 1100usize;
    let ch = 540usize;
    let cx = (fb.width - cw) / 2;
    let cy = (fb.height - ch) / 2;
    let pw = 800usize;
    let ph = 60usize;
    let px = cx + (cw - pw) / 2;
    let py = cy + ch - ph - 80;
    (px, py, pw, ph)
}

/// PosiciÃ³n del caret en pÃ­xeles, dado `len` (caracteres ya escritos).
fn caret_pos(fb: &Framebuffer) -> (usize, usize) {
    let (px, py, _, _) = prompt_rect(fb);
    let len = unsafe { INPUT_LEN };
    let cx = px + 16 + 8 * 2 * 2 + len * 16;
    let cy = py + 20;
    (cx, cy)
}

/// Pinta o borra el caret en su posiciÃ³n actual. No toca el resto del
/// frame: usado tanto por el render full como por el blink local.
fn paint_caret(fb: &Framebuffer, on: bool) {
    let (cx, cy) = caret_pos(fb);
    let color = if on { pal::PROMPT_FG } else { pal::PROMPT_BG };
    fb.fill_rect(cx, cy, 12, 24, color);
}

fn render(fb: &Framebuffer) {
    // 1) Wallpaper gradient — dithered in v1.6.12 to break visible bands
    fb.gradient_v(0, 0, fb.width, fb.height, pal::BG_TOP, pal::BG_BOT);

    // 2) Card centrado — more compact v1.6.12 (1100×540) to fill the screen
    //    better and reduce empty space between badges and prompt.
    let cw = 1100usize;
    let ch = 540usize;
    let cx = (fb.width - cw) / 2;
    let cy = (fb.height - ch) / 2;

    // sombra profunda
    fb.fill_rounded_rect(cx + 12, cy + 16, cw, ch, 24, pal::CARD_SHADOW);
    // cuerpo
    fb.fill_rounded_rect(cx, cy, cw, ch, 24, pal::CARD_BG);
    // border interno sutil
    fb.draw_rect(cx + 1, cy + 1, cw - 2, ch - 2, 0xFF1A2D3A, 1);
    // border principal teal
    fb.draw_rect(cx, cy, cw, ch, pal::CARD_BD, 2);

    // 3) Header bar — thin mint accent strip at the very top of the card
    fb.fill_rect(cx + 24, cy + 28, cw - 48, 3, pal::ACCENT);

    // 4) Title — "FastOS-BMO" big, scale 3×, mint dash, soft shadow
    let title_left  = b"FastOS";
    let title_dash  = b"-";
    let title_right = b"BMO";
    let scale_t = 3u32;
    let lw_l = title_left.len()  * 8 * scale_t as usize;
    let lw_d = title_dash.len()  * 8 * scale_t as usize;
    let lw_r = title_right.len() * 8 * scale_t as usize;
    let total = lw_l + lw_d + lw_r + 24;
    let title_y = cy + 60;
    let mut tx = cx + (cw - total) / 2;
    // soft shadow (3px offset, very dark)
    draw_text_scaled(fb, (tx + 3) as u32, (title_y + 3) as u32, title_left,  0xFF020610, scale_t);
    draw_text_scaled(fb, (tx + 3) as u32, (title_y + 3) as u32, title_dash,  0xFF020610, scale_t);
    draw_text_scaled(fb, (tx + 3) as u32, (title_y + 3) as u32, title_right, 0xFF020610, scale_t);
    draw_text_scaled(fb, tx as u32, title_y as u32, title_left,  pal::TITLE, scale_t);
    tx += lw_l + 12;
    draw_text_scaled(fb, tx as u32, title_y as u32, title_dash,  pal::ACCENT, scale_t);
    tx += lw_d + 12;
    draw_text_scaled(fb, tx as u32, title_y as u32, title_right, pal::TITLE, scale_t);

    // 5) Subtitle
    let sub = b"Bare Metal Orchestrator";
    let sw = sub.len() * 8 * 2;
    let sx = cx + (cw - sw) / 2;
    draw_text_scaled(fb, sx as u32, (cy + 130) as u32, sub, pal::SUBTITLE, 2);

    // 6) Version line
    let ver = b"v1.6.17 ::  Ring 0 + Ring 3  ::  [0-warnings|log-clipped|PCI-skip]";
    let vw = ver.len() * 8;
    let vx = cx + (cw - vw) / 2;
    draw_text(fb, vx as u32, (cy + 170) as u32, ver, pal::VERSION);

    // 7) Phase progress bar — 5 segments with shimmer highlight on done
    let pb_x = cx + 80;
    let pb_y = cy + 210;
    let pb_w = cw - 160;
    let pb_h = 12;
    fb.fill_rounded_rect(pb_x - 2, pb_y - 2, pb_w + 4, pb_h + 4, 6, pal::PHASE_BG);
    let phases_total = 5usize;
    let seg_w = pb_w / phases_total;
    let seg_gap = 6usize;
    let current_phase = 4usize;
    for i in 0..phases_total {
        let sxi = pb_x + i * seg_w + seg_gap / 2;
        let swi = seg_w - seg_gap;
        let color = if i < current_phase { pal::PHASE_DONE }
                    else if i == current_phase { pal::PHASE_CURR }
                    else { pal::PHASE_PEND };
        fb.fill_rounded_rect(sxi, pb_y, swi, pb_h, 5, color);
        // shimmer: thin mint highlight on the top 2 px of DONE segments
        if i < current_phase {
            fb.fill_rect(sxi + 2, pb_y + 1, swi - 4, 2, 0xFF8FF0CC);
        }
    }
    // Phase labels under the bar with icons
    let labels: [&[u8]; 5] = [b"CPU", b"Mem", b"Dev", b"Disp", b"Desk"];
    let mut lx = pb_x;
    for (i, lab) in labels.iter().enumerate() {
        let color = if i < current_phase { pal::OK_FG }
                    else if i == current_phase { pal::VERSION }
                    else { pal::SUBTITLE };
        let lw = lab.len() * 8;
        let lxoff = seg_w.saturating_sub(lw) / 2;
        draw_text(fb, (lx + lxoff) as u32, (pb_y + 22) as u32, lab, color);
        lx += seg_w;
    }

    // 8) Subsystem badges — v1.6.12 with ASCII icons
    //    Check mark "v" for done, gold dot for current
    let badges: [(&[u8], &[u8], &[u8]); 5] = [
        (b"v", b"Ring0+3",    b"active"),
        (b"v", b"Syscalls",   b"13 ops"),
        (b"v", b"Compositor", b"loaded"),
        (b"v", b"PS/2+Beep",  b"ready"),
        (b"v", b"RAMdisk+FS", b"open/rd/cls"),
    ];
    let by0 = cy + 260;
    let bw = (cw - 160 - 4 * 8) / 5;
    let bh = 56;
    for (i, (icon, label, value)) in badges.iter().enumerate() {
        let bx = cx + 80 + i * (bw + 8);
        // base fill
        fb.fill_rounded_rect(bx, by0, bw, bh, 10, pal::OK_BG);
        // mint border
        fb.draw_rect(bx, by0, bw, bh, pal::OK_FG, 1);
        // icon badge (small filled circle with check, 14x14 at top-left)
        fb.fill_circle(bx + 14, by0 + 14, 8, pal::OK_FG);
        draw_text(fb, (bx + 10) as u32, (by0 + 6) as u32, icon, pal::OK_BG);
        // label in mint, value in cool gray
        draw_text(fb, (bx + 28) as u32, (by0 + 8) as u32, label, pal::OK_FG);
        draw_text(fb, (bx + 28) as u32, (by0 + 26) as u32, value, pal::SUBTITLE);
        // second tiny icon next to value (•)
        draw_text(fb, (bx + 28) as u32, (by0 + 42) as u32, b"\x95 ok", 0xFF3D5C50);
    }

    // 9) Prompt box
    let (px, py, pw, ph) = prompt_rect(fb);

    // hint above the prompt
    let hint = b">>  Escribe (Run) y pulsa Enter para entrar al Ring 0 desktop";
    let hx = px;
    draw_text(fb, hx as u32, (py - 28) as u32, hint, pal::SUBTITLE);

    // caja
    fb.fill_rounded_rect(px, py, pw, ph, 10, pal::PROMPT_BG);
    fb.draw_rect(px, py, pw, ph, pal::PROMPT_BD, 2);
    // inner glow (subtle top border highlight)
    fb.fill_rect(px + 2, py + 2, pw - 4, 1, 0xFF6FFFD0);

    // prompt "> " + input
    draw_text_scaled(fb, (px + 16) as u32, (py + 18) as u32, b"> ", pal::ACCENT, 2);
    let len = unsafe { INPUT_LEN };
    if len > 0 {
        let txt = unsafe { &INPUT_BUF[..len] };
        draw_text_scaled(fb, (px + 16 + 8 * 2 * 2) as u32, (py + 18) as u32, txt, pal::PROMPT_FG, 2);
    } else {
        // v1.6.7: ghosted placeholder
        draw_text_scaled(fb, (px + 16 + 8 * 2 * 2) as u32, (py + 18) as u32, b"Run", 0xFF3D4F5F, 2);
    }

    // 10) Hint (si hay)
    let (timer, msg) = unsafe { (HINT_TIMER, HINT_MSG) };
    if timer > 0 && !msg.is_empty() {
        draw_text(fb, hx as u32, (py + ph + 16) as u32, msg, pal::HINT);
    }

    // 11) RUN button — a la derecha del prompt, with subtle gradient
    let btn_w = 120usize;
    let btn_h = 60usize;
    let bx = px + pw - btn_w;
    let by = py;
    let btn_active = unsafe { INPUT_LEN > 0 && {
        let s = &INPUT_BUF[..INPUT_LEN];
        eq_ci(s, b"run")
    } };
    let btn_color = if btn_active { pal::RUN_BTN_HI } else { pal::RUN_BTN };
    fb.fill_rounded_rect(bx, by, btn_w, btn_h, 10, btn_color);
    fb.draw_rect(bx, by, btn_w, btn_h, pal::CARD_BD, 1);
    // arrow icon on the left
    draw_text_scaled(fb, (bx + 12) as u32, (by + 18) as u32, b"\x10", pal::TITLE, 2);
    let lbl = b"RUN";
    let lw = lbl.len() * 8 * 2;
    let lx = bx + 36 + (btn_w - 36 - lw) / 2;
    draw_text_scaled(fb, lx as u32, (by + 18) as u32, lbl, pal::TITLE, 2);

    // 12) Pie — RAM, BIOS, build info
    let free_mb = unsafe { (crate::arch::page_alloc::free_count() * 4) / 1024 };
    let mut foot = [0u8; 96];
    let prefix = b"FastOS / BMO  ::  Ryzen 5 5600X  ::  ";
    let mut i = 0;
    while i < prefix.len() && i < foot.len() { foot[i] = prefix[i]; i += 1; }
    // append free_mb as decimal
    if free_mb > 0 {
        let mut v = free_mb as u32;
        if v == 0 { v = 0; }
        // simple itoa
        let mut buf = [0u8; 10];
        let mut j = 10;
        if v == 0 { j -= 1; buf[j] = b'0'; }
        else { while v > 0 { j -= 1; buf[j] = b'0' + (v % 10) as u8; v /= 10; } }
        let s = &buf[j..];
        for &b in s { if i < foot.len() - 12 { foot[i] = b; i += 1; } }
    }
    let suffix = b" MB free  ::  UEFI";
    for &b in suffix { if i < foot.len() { foot[i] = b; i += 1; } }
    let fw = i * 8;
    let fx = cx + (cw - fw) / 2;
    draw_text(fb, fx as u32, (cy + ch - 32) as u32, &foot[..i], pal::SUBTITLE);
    // build line below
    let build = b"build 1.6.12  ::  AMD64  ::  BMO ABI v0.4.0";
    let bw2 = build.len() * 8;
    let bx2 = cx + (cw - bw2) / 2;
    draw_text(fb, bx2 as u32, (cy + ch - 16) as u32, build, 0xFF455364);
}

/// Render mÃ­nimo y robusto para hardware real.
///
/// Usa sÃ³lo `clear/fill_rect/text` y evita gradientes/rounded-corners en el
/// boot path. AsÃ­ el prompt y el hotkey diag funcionan aunque el renderer
/// avanzado tenga un problema de GOP/pitch especÃ­fico de la mÃ¡quina.
fn render_safe(fb: &Framebuffer) {
    fb.clear(0xFF07111F);
    fb.fill_rect(0, 0, fb.width, 42, 0xFF101820);
    draw_text(fb, 14, 13, b"FastOS / BMO - SAFE WELCOME", 0xFFE6EDF3);
    draw_text(fb, 14, 58, b"GOP framebuffer OK. Storage/NIC deferred for stable boot.", 0xFF76B900);
    draw_text(fb, 14, 82, b"F9: diag HUD   Ctrl+Alt: diag HUD   Run + Enter: desktop Ring 0", 0xFF8B949E);

    let y = (fb.height / 2).saturating_sub(32);
    fb.fill_rect(14, y, fb.width.saturating_sub(28), 72, 0xFF0D1117);
    fb.fill_rect(14, y, fb.width.saturating_sub(28), 2, 0xFF58A6FF);
    draw_text(fb, 30, (y + 16) as u32, b"> ", 0xFF58A6FF);

    let len = unsafe { INPUT_LEN };
    if len > 0 {
        let txt = unsafe { &INPUT_BUF[..len] };
        draw_text(fb, 54, (y + 16) as u32, txt, 0xFFE6EDF3);
    } else {
        draw_text(fb, 54, (y + 16) as u32, b"type Run", 0xFF30363D);
    }

    let (timer, msg) = unsafe { (HINT_TIMER, HINT_MSG) };
    if timer > 0 && !msg.is_empty() {
        draw_text(fb, 30, (y + 50) as u32, msg, 0xFFFFBD2E);
    }
}

// â”€â”€ State global del welcome â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€
//
// `DIRTY` se pone a true cada vez que algo del frame cambia (input,
// hint, etc). El loop principal sÃ³lo re-renderiza el frame entero
// cuando `DIRTY = true`; el blink del caret se gestiona aparte
// repintando sÃ³lo la zona del caret. Esto elimina el ghost flicker
// que se veÃ­a cuando hacÃ­amos full repaint cada 32 ms.

static mut DIRTY: bool = true;
static mut LAST_BLINK_ON: bool = false;
static mut LAST_HINT_TIMER: u32 = 0;

#[inline]
fn mark_dirty() { unsafe { DIRTY = true; } }

fn show_hint(msg: &'static [u8]) {
    unsafe {
        HINT_MSG = msg;
        HINT_TIMER = 120; // ~2 seg a 60 FPS
    }
    mark_dirty();
}

// â”€â”€ Self-test commands â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€

fn run_phase_self_test(n: u8) {
    use crate::boot::phases::report_self_test;
    let report = match n {
        0 => crate::boot::phases::phase0_cpu::self_test(),
        1 => crate::boot::phases::phase1_memory::self_test(),
        2 => crate::boot::phases::phase2_devices::self_test(),
        3 => crate::boot::phases::phase3_display::self_test(),
        4 => crate::boot::phases::phase4_scheduler::self_test(),
        5 => crate::boot::phases::phase5_desktop::self_test(),
        _ => {
            crate::diag::warn("welcome", "Unknown phase index");
            return;
        }
    };
    crate::diag::info("welcome", "Phase self-test");
    report_self_test(&report);
}

fn run_phase_self_test_ring3() {
    use crate::boot::phases::report_self_test;
    let report = crate::boot::phases::ring3_tests::self_test();
    crate::diag::info("welcome", "Ring 3 self-test");
    report_self_test(&report);
}

fn run_test_all_phases() {
    use crate::boot::phases::report_self_test;
    let reports = [
        crate::boot::phases::phase0_cpu::self_test(),
        crate::boot::phases::ring3_tests::self_test(),
        crate::boot::phases::phase1_memory::self_test(),
        crate::boot::phases::phase2_devices::self_test(),
        crate::boot::phases::phase3_display::self_test(),
        crate::boot::phases::phase4_scheduler::self_test(),
        crate::boot::phases::phase5_desktop::self_test(),
    ];
    crate::diag::info("welcome", "All-phase self-test");
    for r in &reports {
        report_self_test(r);
    }
    let total_failed: usize = reports.iter().map(|r| r.failed_count()).sum();
    if total_failed == 0 {
        sound::beep(880, 60);
        sound::beep(1175, 60);
        show_hint(b"All phase self-tests PASSED.");
    } else {
        show_hint(b"Self-test failures - see serial log.");
    }
}

fn process_enter() {
    let cmd = unsafe { &INPUT_BUF[..INPUT_LEN] };
    let trimmed_cmd = trim(cmd);

    if trimmed_cmd.is_empty() {
        show_hint(b"Escribe (Run) y Enter.");
    } else if should_enter_desktop(trimmed_cmd) {
        enter_desktop();
        // If we get here, the desktop returned (shouldn't happen but
        // the watchdog can cause this). Just redraw the welcome.
        crate::diag::info("welcome", "Desktop returned; re-entering welcome");
        show_hint(b"Desktop returned unexpectedly. Type test for diagnostics.");
    } else if eq_ci(trimmed_cmd, b"hello") {
        crate::diag::info("welcome", "Hello command accepted; preparing Ring 3 test");
        sound::beep(440, 80);
        crate::sched::user_init::spawn_hello();
    } else if eq_ci(trimmed_cmd, b"ring3") {
        // Alias for "hello" â€” explicit Ring 3 transition test.
        crate::diag::info("welcome", "Ring3 command accepted; testing Ring 0 -> Ring 3");
        sound::beep(440, 80);
        crate::sched::user_init::spawn_hello();
    } else if eq_ci(trimmed_cmd, b"reboot") {
        crate::diag::warn("welcome", "Reboot command accepted");
        unsafe { core::arch::asm!("out dx, al", in("dx") 0x64u16, in("al") 0xFEu8); }
    } else if eq_ci(trimmed_cmd, b"nexo") {
        crate::diag::info("welcome", "NEXO compiler test â€” compiling hello program");
        nexo_test_compile();
    } else if eq_ci(trimmed_cmd, b"test desktop") {
        // Isolated desktop test: render one frame and report.
        crate::diag::info("welcome", "test desktop: rendering single frame");
        crate::drivers::serial::serial_write("[welcome] test desktop: calling render_frame()\n");
        crate::desktop::render::render_frame();
        crate::drivers::serial::serial_write("[welcome] test desktop: render_frame() returned OK\n");
        crate::diag::info("welcome", "test desktop: render_frame OK");
    } else if eq_ci(trimmed_cmd, b"test") {
        run_test_all_phases();
    } else if eq_ci(trimmed_cmd, b"test phase 0") {
        run_phase_self_test(0);
    } else if eq_ci(trimmed_cmd, b"test phase 1") {
        run_phase_self_test(1);
    } else if eq_ci(trimmed_cmd, b"test phase 2") {
        run_phase_self_test(2);
    } else if eq_ci(trimmed_cmd, b"test phase 3") {
        run_phase_self_test(3);
    } else if eq_ci(trimmed_cmd, b"test phase 4") {
        run_phase_self_test(4);
    } else if eq_ci(trimmed_cmd, b"test phase 5") {
        run_phase_self_test(5);
    } else if eq_ci(trimmed_cmd, b"test ring3") {
        run_phase_self_test_ring3();
    } else {
        crate::diag::warn("welcome", "Unknown command at welcome prompt");
        show_hint(b"Comandos: Run, Hello, Ring3, Nexo, Test, Reboot.");
    }
    unsafe { INPUT_LEN = 0; }
    mark_dirty();
}

// â”€â”€ Main loop â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€

pub fn run() -> ! {
    crate::drivers::serial::serial_write("[welcome] Pantalla de bienvenida activa.\n");

    // v1.6.11: wipe the boot splash before painting the welcome card.
    crate::boot::visual::clear();

    // v1.5.0: Windows-inspired logon sound (sweep A4 â†’ A5)
    crate::gustos::tracks::windows::logon();
    crate::drivers::serial::serial_write("[welcome] gustOS logon sound played\n");

    loop {
        // 1) Full repaint solo si algo cambiÃ³.
        if unsafe { DIRTY } {
            if let Some(fb) = fb() {
                render(&fb);
                // Pintar el caret en el estado actual del blink para que
                // no aparezca/desaparezca en el siguiente sub-loop.
                let on = blink_on();
                paint_caret(&fb, on);
                unsafe { LAST_BLINK_ON = on; }
            }
            unsafe { DIRTY = false; }
        }

            if crate::diag::is_overlay_enabled() {
                crate::diag::paint_overlay();
            }

        // 3) Sub-loop de ~16 ms: drena input y gestiona blink local.
        let cycles = 16u64 * 3_700_000;
        let start = crate::arch::cpu::rdtsc();
        while (crate::arch::cpu::rdtsc() - start) < cycles {
            let overlay_was_enabled = crate::diag::is_overlay_enabled();
            let sc = super::input::poll_key();
            if crate::diag::is_overlay_enabled() != overlay_was_enabled {
                mark_dirty();
            }
            if sc != 0 {
                if sc == 0x1C {
                    process_enter();
                } else if let Some(ch) = process_scancode(sc) {
                    handle_char(ch);
                }
            }

            if let Some(mut ch) = crate::drivers::serial::serial_read_byte() {
                if ch == b'\r' { ch = b'\n'; }
                handle_char(ch);
            }

            // Blink local: si el estado cambia, repintar SOLO el caret.
            let cur = blink_on();
            if cur != unsafe { LAST_BLINK_ON } {
                if let Some(fb) = fb() {
                    paint_caret(&fb, cur);
                }
                unsafe { LAST_BLINK_ON = cur; }
            }

            core::hint::spin_loop();
        }

        // 3) Decrementar hint timer â€” si llega a 0, marcar dirty para
        //    borrarlo del frame.
        unsafe {
            let prev = HINT_TIMER;
            if HINT_TIMER > 0 { HINT_TIMER -= 1; }
            if (prev > 0 && HINT_TIMER == 0) || (prev != LAST_HINT_TIMER && HINT_TIMER == 0) {
                DIRTY = true;
            }
            LAST_HINT_TIMER = HINT_TIMER;
        }
    }
}

/// Estado del caret blink basado en TSC (~1.5 Hz). Centralizado para
/// que el render full y el blink local lean exactamente el mismo bit.
fn blink_on() -> bool {
    (crate::arch::cpu::rdtsc() / 1_250_000_000) & 1 != 0
}

fn handle_char(ch: u8) {
    match ch {
        b'\n' => process_enter(),
        8 => unsafe {
            if INPUT_LEN > 0 {
                // Antes de mover el caret, borrarlo de la posiciÃ³n vieja
                // para evitar ghost de la barra del caret previo.
                if let Some(fb) = fb() { paint_caret(&fb, false); }
                INPUT_LEN -= 1;
                mark_dirty();
            }
        },
        c if (c >= 32 && c <= 126) || c == 164 || c == 165 => unsafe {
            if INPUT_LEN < MAX_INPUT - 1 {
                if let Some(fb) = fb() { paint_caret(&fb, false); }
                INPUT_BUF[INPUT_LEN] = c;
                INPUT_LEN += 1;
                // Ya NO auto-disparamos al escribir "run": el usuario
                // debe pulsar Enter para evitar congelamientos.
                mark_dirty();
            }
        },
        _ => {}
    }
}
