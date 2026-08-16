//! **EL ARRANQUE** -- el panel de bienvenida y la barra de progreso.
//!
//! Es la pantalla de antes de la CABINA: el logo, el titulo, la barra y la
//! linea de estado. La otra mitad del arranque --la ciudad y el gato-- vive en
//! `escena.rs`; esta es la parte sobria, la que se ve si la intro no llega a
//! correr.
//!
//! [!] Se queda aqui `smooth_progress`, que anima la barra pixel a pixel con
//! esperas del TSC. Es de las pocas cosas del arranque que **si duermen**, y
//! puede hacerlo porque no tapa trabajo: la barra se mueve cuando ya no hay
//! nada corriendo detras.

use super::lienzo::{fill_rect, wc_flush};
use super::reloj::tsc_wait;
use super::texto::{draw_str, text_width, CHAR_H};
use super::{ACCENT, ACCENT2, BG};

/// Estado de la interpolacion de la barra: hasta donde se dibujo la ultima vez.
static mut LAST_PCT: u32 = 0;

/// Blend a foreground color over BG at a given alpha (0..255).
fn blend(fg: u32, alpha: u32) -> u32 {
    let a = alpha.min(255);
    let inv = 255 - a;
    let fg_r = (fg >> 16) & 0xFF;
    let fg_g = (fg >> 8) & 0xFF;
    let fg_b = fg & 0xFF;
    let bg_r = (BG >> 16) & 0xFF;
    let bg_g = (BG >> 8) & 0xFF;
    let bg_b = BG & 0xFF;
    let r = (fg_r * a + bg_r * inv) / 255;
    let g = (fg_g * a + bg_g * inv) / 255;
    let b = (fg_b * a + bg_b * inv) / 255;
    0xFF000000 | (r << 16) | (g << 8) | b
}

/// Create a gradient color along the progress bar (cyan -> indigo).
fn bar_gradient(x_off: u32, total_w: u32) -> u32 {
    if total_w == 0 { return ACCENT; }
    let t = (x_off * 255 / total_w).min(255);
    let inv = 255 - t;
    // ACCENT=0xFF00E5FF -> ACCENT2=0xFF818CF8
    let r = (0x00 * inv + 0x81 * t) / 255;
    let g = (0xE5 * inv + 0x8C * t) / 255;
    let b = (0xFF * inv + 0xF8 * t) / 255;
    0xFF000000 | (r << 16) | (g << 8) | b
}

// ?????? Smooth progress bar ?????????????????????????????????????????????????????????????????????????????????????????????????????????????????????????????????

/// Animate the progress bar smoothly from `LAST_PCT` to `target_pct`.
/// Uses sub-percentage pixel-level interpolation for ultra-smooth fill.
fn smooth_progress(bx: u32, by: u32, bar_w: u32, bar_h: u32, target_pct: u32) {
    let start_pix = unsafe { (bar_w as u64 * LAST_PCT as u64 / 100) as u32 };
    let end_pix = (bar_w as u64 * target_pct.min(100) as u64 / 100) as u32;

    if start_pix >= end_pix {
        unsafe { LAST_PCT = target_pct.min(100); }
        return;
    }

    // Animate pixel-by-pixel for maximum smoothness
    let mut px = start_pix;
    while px < end_pix {
        // Draw the new column with gradient color
        let col_color = bar_gradient(px, bar_w);
        fill_rect(bx + px, by, 1, bar_h, col_color);
        px += 1;

        // Adaptive speed: fast start, smooth middle, slow finish
        let progress_ratio = px * 100 / bar_w;
        let delay = if progress_ratio < 30 {
            800_000u64
        } else if progress_ratio < 70 {
            1_200_000u64
        } else {
            1_800_000u64
        };
        tsc_wait(delay);
    }

    unsafe { LAST_PCT = target_pct.min(100); }
}

// ?????? Public API ????????????????????????????????????????????????????????????????????????????????????????????????????????????????????????????????????????????????????????????

pub fn splash_init() {
    let w = unsafe { crate::info::FB_WIDTH };
    let h = unsafe { crate::info::FB_HEIGHT };
    let fb_addr = unsafe { crate::info::FB_ADDR };
    let fb_stride = unsafe { crate::info::FB_STRIDE };
    let fb_fmt = unsafe { crate::info::FB_PIXEL_FORMAT };

    // Log to serial (even if the user can't see it, it's useful later)
    crate::ring0::dev::console::serial_write("[splash] fb=0x");
    crate::ring0::dev::console::serial_write_u64(fb_addr, 16);
    crate::ring0::dev::console::serial_write(" ");
    crate::ring0::dev::console::serial_write_u64_dec(w as u64);
    crate::ring0::dev::console::serial_write("x");
    crate::ring0::dev::console::serial_write_u64_dec(h as u64);
    crate::ring0::dev::console::serial_write(" stride=");
    crate::ring0::dev::console::serial_write_u64_dec(fb_stride as u64);
    crate::ring0::dev::console::serial_write(" fmt=");
    crate::ring0::dev::console::serial_write_u64_dec(fb_fmt as u64);
    crate::ring0::dev::console::serial_write("\n");

    if w == 0 || h == 0 || fb_addr == 0 {
        crate::ring0::dev::console::serial_write("[splash] FB not available\n");
        return;
    }

    // -- Try filling the whole screen using rep stosd ---------------
    //    This is the fastest, most reliable way to write a GPU
    //    framebuffer: the CPU's string-store engine does 64-byte
    //    bursts internally and handles WC buffering correctly.
    //    After the fill, we use mfence to flush the WC buffer.
    let total = (fb_stride as usize) * (h as usize);
    crate::ring0::dev::console::serial_write("[splash] filling ");
    crate::ring0::dev::console::serial_write_u64_dec(total as u64);
    crate::ring0::dev::console::serial_write(" px\n");

    unsafe {
        let di = fb_addr;
        let color: u32 = 0xFFFFFF00u32;
        core::arch::asm!(
            "cld",
            "mov rdi, {di}",
            "mov eax, {color:e}",
            "mov ecx, {count:e}",
            "rep stosd",
            "mfence",
            di = in(reg) di,
            color = in(reg) color,
            count = in(reg) total,
            options(nostack, preserves_flags),
        );
    }

    crate::ring0::dev::console::serial_write("[splash] fill done -- screen should be yellow\n");

    // Wait a moment so the user can see the fill
    tsc_wait(300_000_000); // ~100 ms @ 3.7 GHz

    // Draw centered text over the fill
    let txt = "BMO-X";
    let tx = (w as u32).saturating_sub(text_width(txt)) / 2;
    let cy = h / 2;
    draw_str(tx, cy - 10, txt, 0xFF000000u32);
    wc_flush();
    crate::ring0::dev::console::serial_write("[splash] text drawn\n");

    // Skip the animated splash for now -- the fill test is priority
}

pub fn splash_progress(pct: u32, label: &str) {
    let w = unsafe { crate::info::FB_WIDTH };
    let h = unsafe { crate::info::FB_HEIGHT };
    if w == 0 || h == 0 { return; }

    let cy = h / 2;
    let bar_w = 320u32;
    let bar_h = 6u32;
    let bx = (w as u32).saturating_sub(bar_w) / 2;
    let bar_y = cy + 50;

    // Smooth pixel-level interpolated progress bar
    smooth_progress(bx, bar_y, bar_w, bar_h, pct);

    // Update label (clear old, draw new centered)
    let label_y = bar_y + bar_h + 12;
    fill_rect(0, label_y, w, CHAR_H as u32, BG);
    let lx = (w as u32).saturating_sub(text_width(label)) / 2;
    draw_str(lx, label_y, label, ACCENT2);
}

pub fn splash_clear() {
    let w = unsafe { crate::info::FB_WIDTH };
    let h = unsafe { crate::info::FB_HEIGHT };
    if w == 0 { return; }
    fill_rect(0, 0, w, h, BG);
}
