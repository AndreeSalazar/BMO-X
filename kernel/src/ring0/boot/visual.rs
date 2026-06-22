//! v1.8.8: Professional boot splash screen.
//!
//! Replaces the ugly "yellow text on black rows" overlay with a proper
//! splash that matches the welcome card's visual language:
//!
//!   - Dark teal-indigo gradient background (same as welcome wallpaper)
//!   - Centered "FastOS-BMO" card with a mint accent bar
//!   - 5 phase progress bars (CPU, Mem, Dev, Disp, Sched) with phase colors
//!   - Live log area with rotating messages
//!   - Footer with build info
//!
//! All primitives are direct framebuffer writes (no_std). The boot banner
//! is drawn ONCE on first `log()` call, and updated incrementally as phases
//! progress.

use core::sync::atomic::{AtomicUsize, AtomicBool, Ordering};
use crate::bmo_core::ui::font;

// ── Palette (matches welcome) ──────────────────────────────────────

const BG_TOP:       u32 = 0xFF050B12;
const BG_BOT:       u32 = 0xFF0E1B2E;
const CARD_BG:      u32 = 0xFF0F1827;
const CARD_BD:      u32 = 0xFF1F4D5C;
const ACCENT:       u32 = 0xFF4ECCA3;
const TITLE:        u32 = 0xFFE6F1F5;
const SUBTITLE:     u32 = 0xFF7B8FA1;
const DIM:          u32 = 0xFF455364;

// Phase colors (one per phase 0..4 + welcome)
const PH_COLORS: [u32; 5] = [
    0xFF58A6FF, // phase0 CPU  — blue
    0xFF4ECCA3, // phase1 Mem  — mint
    0xFFE2C044, // phase2 Dev  — gold
    0xFF56D4DD, // phase3 Disp — cyan
    0xFFCB6CE6, // phase4 Sched— violet
];
const PH_LABELS: [&[u8]; 5] = [
    b"CPU", b"Mem", b"Dev", b"Disp", b"Sched",
];

const DONE:  u32 = 0xFF4ECCA3;
const CURR:  u32 = 0xFFE2C044;
const PEND:  u32 = 0xFF243140;
const TRACK: u32 = 0xFF0A1018;

// ── Layout (1920×1080 reference; auto-scales below 1280) ───────────

const CARD_W: usize = 1100;
const CARD_H: usize = 520;
const LOG_ROWS: usize = 14;
const LOG_ROW_H: usize = 16;

// ── State ─────────────────────────────────────────────────────────

static INITIALIZED: AtomicBool = AtomicBool::new(false);
static NEXT_LOG_ROW: AtomicUsize = AtomicUsize::new(0);
static CURRENT_PHASE: AtomicUsize = AtomicUsize::new(0);

/// Mark a phase as in-progress. The phase bar changes from pending to
/// "current" (gold) until the phase completes.
pub fn begin_phase(idx: usize) {
    if !INITIALIZED.load(Ordering::Relaxed) { init(); }
    CURRENT_PHASE.store(idx.min(4), Ordering::Relaxed);
    redraw_phase_strip();
}

/// Mark a phase as complete. The phase bar turns mint.
pub fn end_phase(idx: usize) {
    CURRENT_PHASE.store((idx + 1).min(5), Ordering::Relaxed);
    redraw_phase_strip();
}

// ── Init: draw the full splash once ────────────────────────────────

pub fn init() {
    let (addr, w, h, s) = fb();
    if addr.is_null() || w == 0 || h == 0 { return; }

    INITIALIZED.store(true, Ordering::Relaxed);

    // 1) Wallpaper gradient — cover the FULL screen, not just top 700px
    //    (v1.6.14 bug: h.min(700) left a 380px band at the bottom of the
    //    screen unpainted on 1080p displays, showing whatever UEFI left).
    //    Use simple gradient (no dither) so the splash is unmistakably dark.
    simple_gradient_v(addr, s, w, h, 0, 0, w, h, BG_TOP, BG_BOT);

    // 2) Centered card
    let cw = CARD_W.min(w.saturating_sub(40));
    let ch = CARD_H.min(h.saturating_sub(40));
    let cx = (w - cw) / 2;
    let cy = (h - ch) / 2;

    // shadow
    fill_rect(addr, s, w, h, cx + 8, cy + 12, cw, ch, 0xFF020610);
    // body
    fill_rect(addr, s, w, h, cx, cy, cw, ch, CARD_BG);
    draw_rect(addr, s, w, h, cx, cy, cw, ch, CARD_BD, 2);

    // 3) Top accent bar
    fill_rect(addr, s, w, h, cx + 24, cy + 24, cw - 48, 3, ACCENT);

    // 4) Title "FastOS-BMO" centered, scale 2×
    let title = b"FastOS-BMO";
    let tw = title.len() * 8 * 2;
    let tx = cx + (cw - tw) / 2;
    text_scaled(addr, s, w, h, tx, cy + 60, title, TITLE, 2);

    // 5) Subtitle
    let sub = b"Bare Metal Orchestrator  ::  v1.8.8";
    let sw = sub.len() * 8;
    let sx = cx + (cw - sw) / 2;
    text(addr, s, w, h, sx, cy + 110, sub, SUBTITLE);

    // 6) Divider
    fill_rect(addr, s, w, h, cx + 60, cy + 140, cw - 120, 1, CARD_BD);

    // 7) Phase strip — first paint with all phases pending
    CURRENT_PHASE.store(0, Ordering::Relaxed);
    redraw_phase_strip();

    // 8) Divider
    fill_rect(addr, s, w, h, cx + 60, cy + 270, cw - 120, 1, CARD_BD);

    // 9) Log header
    text(addr, s, w, h, cx + 60, cy + 290, b"Boot Log", DIM);
    fill_rect(addr, s, w, h, cx + 130, cy + 298, 60, 2, ACCENT);

    // 10) Footer — two lines
    let foot = b"FastOS / BMO  ::  Ryzen 5 5600X  ::  GOP framebuffer  ::  UEFI";
    let fw = foot.len() * 8;
    let fx = cx + (cw - fw) / 2;
    text(addr, s, w, h, fx, cy + ch - 44, foot, SUBTITLE);

    let powered = b"Powered by Eddi Andre Salazar Matos";
    let pw = powered.len() * 8;
    let px = cx + (cw - pw) / 2;
    text(addr, s, w, h, px, cy + ch - 24, powered, DIM);

    // SFENCE: ensure all splash screen writes are visible before continuing.
    unsafe { core::arch::asm!("sfence"); }
}

/// Repaint the 5 phase bars based on CURRENT_PHASE.
fn redraw_phase_strip() {
    let (addr, w, h, s) = fb();
    if addr.is_null() { return; }
    let cw = CARD_W.min(w.saturating_sub(40));
    let ch = CARD_H.min(h.saturating_sub(40));
    let cx = (w - cw) / 2;
    let cy = (h - ch) / 2;
    let current = CURRENT_PHASE.load(Ordering::Relaxed);

    // Strip geometry: 5 segments
    let strip_x = cx + 60;
    let strip_y = cy + 180;
    let strip_w = cw - 120;
    let seg_w = strip_w / 5;
    let seg_h = 14;

    // Background track
    fill_rect(addr, s, w, h, strip_x - 2, strip_y - 2, strip_w + 4, seg_h + 4, TRACK);
    fill_rect(addr, s, w, h, strip_x, strip_y, strip_w, seg_h, PEND);

    for i in 0..5usize {
        let sxi = strip_x + i * seg_w + 4;
        let swi = seg_w - 8;
        let color = if i < current { DONE }
                    else if i == current { CURR }
                    else { PEND };
        fill_rect(addr, s, w, h, sxi, strip_y, swi, seg_h, color);
    }

    // Phase labels under the bars
    for i in 0..5usize {
        let lab = PH_LABELS[i];
        let sxi = strip_x + i * seg_w + 4;
        let lw = lab.len() * 8;
        let lxoff = (seg_w - 8 - lw) / 2;
        let color = if i < current { DONE }
                    else if i == current { PH_COLORS[i] }
                    else { DIM };
        text(addr, s, w, h, sxi + lxoff, strip_y + 22, lab, color);
    }
}

/// Public log entry point. Each call paints one line in the log area.
///
/// `color` is accepted for API symmetry with `boot::log::info` but the
/// splash always uses near-white for the message so it stays visible
/// against the row background (see v1.6.16). Pass any value; it's
/// intentionally ignored.
pub fn log(phase: &str, msg: &str, _color: u32) {
    if !INITIALIZED.load(Ordering::Relaxed) { init(); }

    let (addr, w, h, s) = fb();
    if addr.is_null() { return; }
    let cw = CARD_W.min(w.saturating_sub(40));
    let ch = CARD_H.min(h.saturating_sub(40));
    let cx = (w - cw) / 2;
    let cy = (h - ch) / 2;

    let row = NEXT_LOG_ROW.fetch_add(1, Ordering::Relaxed) % LOG_ROWS;
    let y = cy + 330 + row * LOG_ROW_H;

    // v1.6.17: log row width is now computed with a SAFETY cap to
    // never exceed the card body width. Previous versions used
    // `cw - 120` which assumed `cw = CARD_W` (1100). If the screen
    // is narrower than 1100, `cw` shrinks via `w.saturating_sub(40)`
    // and the log row could still overflow. We now cap the log row
    // width to the card width minus a 32-px margin on each side.
    let log_margin: usize = 32;
    let log_w: usize = if cw > 2 * log_margin { cw - 2 * log_margin } else { cw / 2 };
    let log_x: usize = cx + (cw.saturating_sub(log_w)) / 2;

    // v1.6.16: log row gets a LIGHTER bg (0xFF1A2638, distinguishable
    // from the card body 0xFF0F1827) and the message is painted in
    // high-contrast near-white (0xFFE6F1F5).
    fill_rect(addr, s, w, h, log_x, y, log_w, LOG_ROW_H, 0xFF1A2638);

    // Phase pill: small colored square (8x8) + label
    let phase_color = phase_color(phase);
    let phase_pill_w: usize = 8;
    let phase_gap: usize = 8;
    fill_rect(addr, s, w, h, log_x, y + 4, phase_pill_w, 8, phase_color);

    // SFENCE: flush write-combining buffers before text rendering.
    // The framebuffer is Write-Combining (WC) memory. Without SFENCE,
    // the CPU's WC buffer may reorder or lose small scattered writes
    // (like individual font glyph pixels) relative to the larger
    // fill_rect writes above.
    unsafe { core::arch::asm!("sfence"); }

    text(
        addr, s, w, h,
        log_x + phase_pill_w + phase_gap, y,
        phase.as_bytes(), phase_color,
    );

    // Arrow
    let arrow_x = log_x + phase_pill_w + phase_gap + phase.len() * 8 + 4;
    text(addr, s, w, h, arrow_x, y, b"->", DIM);

    // Message: compute available text columns and clip the message
    // to fit. We append "..." when truncated so the user sees
    // overflow without breaking the row width.
    let msg_x: usize = arrow_x + 24;
    let text_max_cols: usize = if log_w > (msg_x - log_x) + 16 {
        (log_w - (msg_x - log_x) - 16) / 8
    } else {
        0
    };
    let msg_bytes = msg.as_bytes();
    // v1.6.18: always paint SOMETHING (the phase label alone, even
    // if the message is empty or doesn't fit). Previous versions could
    // produce an empty row when text_max_cols was 0 on narrow screens.
    if msg_bytes.is_empty() {
        // No message — just paint the arrow alone so the user sees
        // that a log entry was emitted but had no body.
        unsafe { core::arch::asm!("sfence"); }
        return;
    }
    if msg_bytes.len() > text_max_cols {
        if text_max_cols >= 3 {
            text(addr, s, w, h, msg_x, y, &msg_bytes[..text_max_cols - 3], 0xFFE6F1F5);
            let dots_x = msg_x + (text_max_cols - 3) * 8;
            text(addr, s, w, h, dots_x, y, b"...", DIM);
        } else if text_max_cols > 0 {
            text(addr, s, w, h, msg_x, y, &msg_bytes[..text_max_cols], 0xFFE6F1F5);
        } else {
            // No room for the message — paint at least the phase pill
            // and label so the user sees the row was emitted.
            text(addr, s, w, h, log_x + phase_pill_w + phase_gap, y, b"(...)", 0xFFE6F1F5);
        }
    } else {
        text(addr, s, w, h, msg_x, y, msg_bytes, 0xFFE6F1F5);
    }
    // SFENCE: ensure all text pixel writes are flushed from the
    // WC (Write-Combining) framebuffer buffer before returning.
    // Without this trailing SFENCE, scattered glyph pixel writes
    // may sit in the CPU's WC buffer and never reach the display.
    unsafe { core::arch::asm!("sfence"); }

    // CLFLUSHOPT loop: explicitly flush every cache line touched by
    // the text pixels. SFENCE alone does NOT guarantee that the WC
    // buffer has been drained to memory — the WC buffer is only
    // drained when it's full, when a serializing instruction runs,
    // or when CLFLUSH/CLFLUSHOPT is issued on the same line.
    // For scattered glyph pixels (different rows, different chars),
    // the WC buffer may not fill up, so the writes sit there
    // forever. CLFLUSHOPT on each 64-byte-aligned address in the
    // text region forces the issue.
    let x_start = log_x;
    let x_end = log_x + log_w;
    let y_start = y;
    let y_end = y + LOG_ROW_H;
    // 16 pixels = 64 bytes = one cache line.
    let pixels_per_line: usize = 16;
    // Align start down to cache line boundary
    let first_line = (x_start / pixels_per_line) * pixels_per_line;
    let last_line = ((x_end + pixels_per_line - 1) / pixels_per_line) * pixels_per_line;
    for row in y_start..y_end {
        for col in (first_line..last_line).step_by(pixels_per_line) {
            let pixel_addr = unsafe { addr.add(row * s + col) };
            unsafe { core::arch::asm!("clflushopt [{}]", in(reg) pixel_addr, options(nostack)); }
        }
    }
    // Serializing instruction after CLFLUSHOPT to commit the flushes.
    unsafe { core::arch::asm!("sfence"); }
}

fn phase_color(phase: &str) -> u32 {
    // Map known phase labels to a consistent color.
    if phase.starts_with("phase0") || phase.contains("CPU")   { return PH_COLORS[0]; }
    if phase.starts_with("phase1") || phase.contains("mem")   { return PH_COLORS[1]; }
    if phase.starts_with("phase2") || phase.contains("dev")   { return PH_COLORS[2]; }
    if phase.starts_with("phase3") || phase.contains("disp")  { return PH_COLORS[3]; }
    if phase.starts_with("phase4") || phase.contains("sched") { return PH_COLORS[4]; }
    if phase.starts_with("phase5") || phase.contains("desktop") || phase.contains("welcome") {
        return ACCENT;
    }
    SUBTITLE
}

/// Clear the splash (called before handing off to the welcome screen).
pub fn clear() {
    let (addr, w, h, s) = fb();
    if addr.is_null() { return; }
    gradient_v(addr, s, w, h, 0, 0, w, h, BG_TOP, BG_BOT);
    INITIALIZED.store(false, Ordering::Relaxed);
    NEXT_LOG_ROW.store(0, Ordering::Relaxed);
    CURRENT_PHASE.store(0, Ordering::Relaxed);
}

// ── Framebuffer primitives (avoiding Framebuffer struct dep) ───────

fn fb() -> (*mut u32, usize, usize, usize) {
    let (addr, w, h, s) = unsafe {
        (
            crate::boot::info::FB_ADDR as *mut u32,
            crate::boot::info::FB_WIDTH as usize,
            crate::boot::info::FB_HEIGHT as usize,
            crate::boot::info::FB_STRIDE as usize,
        )
    };
    (addr, w, h, s)
}

fn put(addr: *mut u32, s: usize, w: usize, h: usize, x: usize, y: usize, c: u32) {
    if x >= w || y >= h { return; }
    unsafe { addr.add(y * s + x).write_volatile(c); }
}

fn fill_rect(addr: *mut u32, s: usize, w: usize, h: usize, x: usize, y: usize, rw: usize, rh: usize, c: u32) {
    let x1 = (x + rw).min(w);
    let y1 = (y + rh).min(h);
    for yy in y..y1 {
        for xx in x..x1 {
            put(addr, s, w, h, xx, yy, c);
        }
    }
}

fn draw_rect(addr: *mut u32, s: usize, w: usize, h: usize, x: usize, y: usize, rw: usize, rh: usize, c: u32, t: usize) {
    if rw == 0 || rh == 0 { return; }
    for i in 0..t {
        fill_rect(addr, s, w, h, x + i, y + i, rw.saturating_sub(2 * i), 1, c);
        fill_rect(addr, s, w, h, x + i, y + rh.saturating_sub(1 + i), rw.saturating_sub(2 * i), 1, c);
        fill_rect(addr, s, w, h, x + i, y + i, 1, rh.saturating_sub(2 * i), c);
        fill_rect(addr, s, w, h, x + rw.saturating_sub(1 + i), y + i, 1, rh.saturating_sub(2 * i), c);
    }
}

fn gradient_v(addr: *mut u32, s: usize, w: usize, h: usize, x: usize, y: usize, rw: usize, rh: usize, top: u32, bot: u32) {
    let x1 = (x + rw).min(w);
    let y1 = (y + rh).min(h);
    if y1 <= y { return; }
    let r0 = (top >> 16) & 0xFF;
    let g0 = (top >> 8) & 0xFF;
    let b0 = top & 0xFF;
    let r1 = (bot >> 16) & 0xFF;
    let g1 = (bot >> 8) & 0xFF;
    let b1 = bot & 0xFF;
    let dr = (r1 as i32 - r0 as i32) as i32;
    let dg = (g1 as i32 - g0 as i32) as i32;
    let db = (b1 as i32 - b0 as i32) as i32;
    let span = (y1 - y) as i32;
    for yy in y..y1 {
        let t = ((yy - y) as i32 * 256) / span.max(1);
        let r = (r0 as i32 + (dr * t) / 256) as u32;
        let g = (g0 as i32 + (dg * t) / 256) as u32;
        let b = (b0 as i32 + (db * t) / 256) as u32;
        let c = 0xFF000000 | (r << 16) | (g << 8) | b;
        for xx in x..x1 {
            put(addr, s, w, h, xx, yy, c);
        }
    }
}

/// v1.6.14: simple (no-dither) gradient for the splash background. The
/// dithered version in the welcome card can look noisy on the splash
/// when the screen is mostly empty. Splash gets a clean dark gradient.
fn simple_gradient_v(addr: *mut u32, s: usize, w: usize, h: usize, x: usize, y: usize, rw: usize, rh: usize, top: u32, bot: u32) {
    gradient_v(addr, s, w, h, x, y, rw, rh, top, bot);
}

fn text(addr: *mut u32, s: usize, w: usize, h: usize, x: usize, y: usize, txt: &[u8], color: u32) {
    let mut cx = x;
    for &ch in txt {
        if cx + 8 >= w || y + 16 >= h { break; }
        let glyph = font::get_glyph(ch);
        for py in 0..16 {
            let bits = glyph[py];
            for px in 0..8 {
                if (bits & (0x80 >> px)) != 0 {
                    put(addr, s, w, h, cx + px, y + py, color);
                }
            }
        }
        cx += 8;
    }
}

fn text_scaled(addr: *mut u32, s: usize, w: usize, h: usize, x: usize, y: usize, txt: &[u8], color: u32, scale: u32) {
    let sc = scale.max(1) as usize;
    let gw = 8 * sc;
    let gh = 16 * sc;
    let mut cx = x;
    for &ch in txt {
        if cx + gw >= w || y + gh >= h { break; }
        let glyph = font::get_glyph(ch);
        for py in 0..16 {
            let bits = glyph[py];
            for px in 0..8 {
                if (bits & (0x80 >> px)) != 0 {
                    for ry in 0..sc {
                        for rx in 0..sc {
                            put(addr, s, w, h, cx + px * sc + rx, y + py * sc + ry, color);
                        }
                    }
                }
            }
        }
        cx += gw;
    }
}

#[allow(dead_code)]
fn text_dummy() {} // placeholder to satisfy old callers (deprecated)

pub mod color {
    pub(crate) const OK: u32 = 0xFF4ECCA3;
    pub(crate) const WARN: u32 = 0xFFFFBD2E;
    pub(crate) const FAULT: u32 = 0xFFFF2A2A;
    #[allow(dead_code)]
    pub(crate) const HEADER: u32 = 0xFF58A6FF;
    #[allow(dead_code)]
    pub(crate) const TEXT: u32 = 0xFFE6F1F5;
}

/// True if the visual overlay is active (framebuffer present and not
/// cleared by clear()). After the desktop is up, drivers should NOT
/// call into the visual overlay; only serial + diag are kept.
pub fn is_active() -> bool {
    use core::sync::atomic::Ordering;
    if !INITIALIZED.load(Ordering::Relaxed) { return false; }
    unsafe { crate::boot::info::FB_ADDR != 0 }
}
