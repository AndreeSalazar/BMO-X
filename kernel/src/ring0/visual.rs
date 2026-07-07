//! v1.8.8: Professional boot splash screen.
//!
//! Replaces the ugly "yellow text on black rows" overlay with a proper
//! Ring 0 splash:
//!
//!   - Dark teal-indigo gradient background
//!   - Centered "BMO-BMO" card with a mint accent bar
//!   - 5 phase progress bars (CPU, Mem, Dev, Disp, Sched) with phase colors
//!   - Live log area with rotating messages
//!   - Footer with build info
//!
//! All primitives are direct framebuffer writes (no_std). The boot banner
//! is drawn ONCE on first `log()` call, and updated incrementally as phases
//! progress.

use core::sync::atomic::{AtomicUsize, AtomicBool, Ordering};

// ── Font: real CP437/VGA 8×16 bitmap (ring0/font.rs) ──────────────
use super::font;

// ── Palette ────────────────────────────────────────────────────────

const BG_TOP:       u32 = 0xFF050B12;
const BG_BOT:       u32 = 0xFF0E1B2E;
const CARD_BG:      u32 = 0xFF0F1827;
const CARD_BD:      u32 = 0xFF1F4D5C;
const ACCENT:       u32 = 0xFF4ECCA3;
const NEON_GREEN:   u32 = 0xFF39FF14;
const NEON_DIM:     u32 = 0xFF147A4D;
const NEON_DARK:    u32 = 0xFF063822;
const TITLE:        u32 = 0xFFE6F1F5;
const SUBTITLE:     u32 = 0xFF7B8FA1;
const DIM:          u32 = 0xFF455364;

// Phase colors (one per Ring 0 phase 0..4)
const PH_COLORS: [u32; 5] = [
    0xFF58A6FF, // phase0 CPU  — blue
    0xFF4ECCA3, // phase1 Mem  — mint
    0xFFE2C044, // phase2 Dev  — gold
    0xFF56D4DD, // phase3 Disp — cyan
    0xFFCB6CE6, // phase4 Sched — violet
];
const PH_LABELS: [&[u8]; 5] = [
    b"CPU", b"Mem", b"Dev", b"Disp", b"Sched",
];

const DONE:  u32 = 0xFF4ECCA3;
const CURR:  u32 = 0xFFE2C044;
const PEND:  u32 = 0xFF243140;
const TRACK: u32 = 0xFF0A1018;

// ── Layout (1920×1080 reference; auto-scales below 1280) ──────────

const CARD_W: usize = 1100;
const CARD_H: usize = 520;
const LOG_ROWS: usize = 14;
const LOG_ROW_H: usize = 16;

// ── State ──────────────────────────────────────────────────────────

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

    // 4) Title "BMO-BMO" centered, scale 2×
    let title = b"BMO: Ok Ready";
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
    let foot = b"BMO  ::  Ryzen 5 5600X  ::  GOP framebuffer  ::  UEFI";
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
    // SFENCE ensures writes reach the WC (Write-Combining) buffer.
    unsafe { core::arch::asm!("sfence"); }

    // CPUID as a serializing instruction to drain the WC buffer.
    // CLFLUSHOPT on WC memory is implementation-specific — on AMD Zen 3
    // it discards writes instead of flushing them, making scattered
    // font glyph pixels invisible. CPUID is a documented serializing
    // instruction that drains the WC store buffer on all x86 CPUs.
    unsafe {
        let _ebx: u32;
        core::arch::asm!(
            "push rbx",
            "xor eax, eax",
            "cpuid",
            "mov {ebx_val:e}, ebx",
            "pop rbx",
            inout("eax") 0u32 => _,
            ebx_val = out(reg) _ebx,
            out("ecx") _,
            out("edx") _,
            options(nostack),
        );
    }
}

fn phase_color(phase: &str) -> u32 {
    // Map known phase labels to a consistent color.
    if phase.starts_with("phase0") || phase.contains("CPU")   { return PH_COLORS[0]; }
    if phase.starts_with("phase1") || phase.contains("mem")   { return PH_COLORS[1]; }
    if phase.starts_with("phase2") || phase.contains("dev")   { return PH_COLORS[2]; }
    if phase.starts_with("phase3") || phase.contains("disp")  { return PH_COLORS[3]; }
    if phase.starts_with("phase4") || phase.contains("sched") { return PH_COLORS[4]; }
    if phase.contains("ready") || phase.contains("idle")       { return ACCENT; }
    SUBTITLE
}

/// Clear the Ring 0 splash and reset visual state.
pub fn clear() {
    let (addr, w, h, s) = fb();
    if addr.is_null() { return; }
    gradient_v(addr, s, w, h, 0, 0, w, h, BG_TOP, BG_BOT);
    INITIALIZED.store(false, Ordering::Relaxed);
    NEXT_LOG_ROW.store(0, Ordering::Relaxed);
    CURRENT_PHASE.store(0, Ordering::Relaxed);
}

/// Final Ring 0-owned GOP screen.
///
/// This is intentionally small and self-contained: it does not call any
/// higher-layer UI code. It proves that Ring 0 completed, GOP is still
/// writable, and the CPU is in a controlled heartbeat loop instead of the
/// old silent `hlt` path.
pub fn ring0_ready_loop(ctx: &crate::context::BootContext) -> ! {
    let (addr, w, h, s) = fb();
    if addr.is_null() || w == 0 || h == 0 {
        crate::dev::console::serial_write("[ring0] ready: no GOP framebuffer; serial idle\n");
        loop {
            core::hint::spin_loop();
        }
    }

    INITIALIZED.store(true, Ordering::Relaxed);
    NEXT_LOG_ROW.store(0, Ordering::Relaxed);
    CURRENT_PHASE.store(5, Ordering::Relaxed);

    crate::uefi_rt::write_boot_stage("ring0_ready_idle");
    cabina_daemon::info("ring0", "Ring 0 ready screen entered");
    crate::dev::console::serial_write("[ring0] ready: GOP heartbeat idle\n");

    simple_gradient_v(addr, s, w, h, 0, 0, w, h, BG_TOP, BG_BOT);
    draw_cyber_grid(addr, s, w, h);

    let cw = if w > 80 { 760usize.min(w - 40) } else { w };
    let ch = if h > 80 { 380usize.min(h - 40) } else { h };
    let cx = (w - cw) / 2;
    let cy = (h - ch) / 2;

    // Neon cyberpunk glow: several translucent-looking hard-color outlines.
    draw_rect(addr, s, w, h, cx.saturating_sub(10), cy.saturating_sub(10), cw + 20, ch + 20, NEON_DARK, 1);
    draw_rect(addr, s, w, h, cx.saturating_sub(6), cy.saturating_sub(6), cw + 12, ch + 12, NEON_DIM, 1);
    draw_rect(addr, s, w, h, cx.saturating_sub(3), cy.saturating_sub(3), cw + 6, ch + 6, ACCENT, 1);
    fill_rect(addr, s, w, h, cx + 8, cy + 10, cw, ch, 0xFF020610);
    fill_rect(addr, s, w, h, cx, cy, cw, ch, CARD_BG);
    draw_rect(addr, s, w, h, cx, cy, cw, ch, CARD_BD, 2);
    if cw > 48 {
        fill_rect(addr, s, w, h, cx + 24, cy + 24, cw - 48, 2, NEON_DIM);
        fill_rect(addr, s, w, h, cx + 24, cy + 27, cw - 48, 3, NEON_GREEN);
    }

    let title = b"RING 0 READY";
    let scale = if cw >= 420 { 2 } else { 1 };
    let tw = title.len() * 8 * scale as usize;
    let tx = cx + cw.saturating_sub(tw) / 2;
    text_scaled(addr, s, w, h, tx, cy + 56, title, TITLE, scale);

    let sub = b"Hardware init complete :: GOP framebuffer alive";
    let sw = sub.len() * 8;
    let sx = cx + cw.saturating_sub(sw) / 2;
    text(addr, s, w, h, sx, cy + 106, sub, NEON_GREEN);

    if cw > 120 {
        fill_rect(addr, s, w, h, cx + 60, cy + 138, cw - 120, 1, CARD_BD);
    }

    let mx = cx + 60usize.min(cw / 8);
    let mut my = cy + 164;
    draw_metric(addr, s, w, h, mx, my, b"Free RAM:      ", ctx.memory.free_mb, b" MiB");
    my += 22;
    draw_metric(addr, s, w, h, mx, my, b"Heap total:    ", ctx.memory.heap_total_bytes, b" bytes");
    my += 22;
    draw_metric(addr, s, w, h, mx, my, b"TSC frequency: ", ctx.cpu.tsc_freq_hz, b" Hz");
    my += 22;
    draw_metric(addr, s, w, h, mx, my, b"PCI devices:   ", ctx.devices.pci_devices_found as u64, b"");
    my += 22;
    text(addr, s, w, h, mx, my, b"Allocator:     ", DIM);
    text(addr, s, w, h, mx + 15 * 8, my, allocator_name(), NEON_GREEN);

    let status = b"Safe Ring 0 idle: neon heartbeat below. Higher UI not connected here.";
    text(addr, s, w, h, mx, cy + ch.saturating_sub(72), status, SUBTITLE);

    let track_x = mx;
    let track_y = cy + ch.saturating_sub(42);
    let track_w = cw.saturating_sub(120).max(1);
    fill_rect(addr, s, w, h, track_x, track_y, track_w, 12, TRACK);
    unsafe { core::arch::asm!("sfence"); }

    let period = if ctx.cpu.tsc_freq_hz > 0 { ctx.cpu.tsc_freq_hz / 12 } else { 50_000_000 };
    let mut step = 0usize;
    loop {
        crate::dev::watchdog::pet_fch_watchdog();
        fill_rect(addr, s, w, h, track_x, track_y, track_w, 12, TRACK);
        let pos_span = track_w.saturating_sub(48).max(1);
        let x = track_x + ((step * 31) % pos_span);
        draw_heartbeat(addr, s, w, h, track_x, track_y, track_w, x);
        unsafe { core::arch::asm!("sfence"); }
        delay_cycles(period.max(1));
        step = step.wrapping_add(1);
    }
}

fn allocator_name() -> &'static [u8] {
    #[cfg(feature = "alloc-llfree")]
    { b"LLFree lock-free" }
    #[cfg(not(feature = "alloc-llfree"))]
    { b"Buddy" }
}

fn draw_cyber_grid(addr: *mut u32, s: usize, w: usize, h: usize) {
    let horizon = h.saturating_mul(62) / 100;
    let step_x = 96usize;
    let step_y = 36usize;

    let mut x = 0usize;
    while x < w {
        let color = if x % (step_x * 2) == 0 { NEON_DARK } else { 0xFF082C22 };
        fill_rect(addr, s, w, h, x, horizon, 1, h.saturating_sub(horizon), color);
        x = x.saturating_add(step_x);
    }

    let mut y = horizon;
    while y < h {
        let color = if ((y - horizon) / step_y) % 2 == 0 { NEON_DARK } else { 0xFF082C22 };
        fill_rect(addr, s, w, h, 0, y, w, 1, color);
        y = y.saturating_add(step_y);
    }

    // Two diagonal neon rails, intentionally approximate and cheap.
    let mut yy = horizon;
    while yy < h {
        let t = yy - horizon;
        let left = w / 2usize;
        let spread = t.saturating_mul(2);
        if left > spread {
            fill_rect(addr, s, w, h, left - spread, yy, 2, 1, NEON_DIM);
        }
        let right = (w / 2).saturating_add(spread);
        if right < w {
            fill_rect(addr, s, w, h, right, yy, 2, 1, NEON_DIM);
        }
        yy = yy.saturating_add(3);
    }
}

fn draw_heartbeat(addr: *mut u32, s: usize, w: usize, h: usize, track_x: usize, track_y: usize, track_w: usize, x: usize) {
    let glow_w = 72usize.min(track_w);
    let core_w = 38usize.min(track_w);
    let tail_w = 18usize.min(track_w);

    if x > track_x + tail_w {
        fill_rect(addr, s, w, h, x - tail_w, track_y + 3, tail_w, 6, NEON_DIM);
    }
    fill_rect(addr, s, w, h, x, track_y + 1, glow_w, 10, NEON_DARK);
    fill_rect(addr, s, w, h, x, track_y + 3, core_w, 6, NEON_GREEN);
    if track_w > 8 {
        fill_rect(addr, s, w, h, track_x, track_y, track_w, 1, NEON_DIM);
        fill_rect(addr, s, w, h, track_x, track_y + 11, track_w, 1, NEON_DIM);
    }
}

fn draw_metric(
    addr: *mut u32,
    s: usize,
    w: usize,
    h: usize,
    x: usize,
    y: usize,
    label: &[u8],
    value: u64,
    suffix: &[u8],
) {
    text(addr, s, w, h, x, y, label, DIM);
    let mut buf = [0u8; 20];
    let digits = dec_bytes(value, &mut buf);
    let vx = x + label.len() * 8;
    text(addr, s, w, h, vx, y, digits, TITLE);
    text(addr, s, w, h, vx + digits.len() * 8, y, suffix, SUBTITLE);
}

fn dec_bytes<'a>(mut value: u64, buf: &'a mut [u8; 20]) -> &'a [u8] {
    if value == 0 {
        buf[19] = b'0';
        return &buf[19..20];
    }
    let mut i = buf.len();
    while value > 0 && i > 0 {
        i -= 1;
        buf[i] = b'0' + (value % 10) as u8;
        value /= 10;
    }
    &buf[i..]
}

fn delay_cycles(cycles: u64) {
    let start = crate::cpu::rdtsc();
    while crate::cpu::rdtsc().wrapping_sub(start) < cycles {
        core::hint::spin_loop();
    }
}

// ── Framebuffer primitives (avoiding Framebuffer struct dep) ──────

fn fb() -> (*mut u32, usize, usize, usize) {
    let (addr, w, h, s) = unsafe {
        (
            crate::info::FB_ADDR as *mut u32,
            crate::info::FB_WIDTH as usize,
            crate::info::FB_HEIGHT as usize,
            crate::info::FB_STRIDE as usize,
        )
    };
    (addr, w, h, s)
}

/// Convert a 0xAARRGGBB color to the framebuffer's native pixel format.
#[inline]
fn fix_color(c: u32) -> u32 {
    let fmt = unsafe { crate::info::FB_PIXEL_FORMAT };
    match fmt {
        bmo_boot_protocol::PixelFormat::Rgb => {
            // U32 0xAARRGGBB in memory (little-endian): BB GG RR AA
            // RGB framebuffer reads as: R=BB, G=GG, B=RR → R and B swapped
            // Fix: swap R and B channels
            let a = c & 0xFF000000;
            let r = (c >> 16) & 0xFF;
            let g = (c >> 8) & 0xFF;
            let b = c & 0xFF;
            a | (b << 16) | (g << 8) | r
        }
        _ => c, // Bgr or Unknown — no conversion needed
    }
}

fn put(addr: *mut u32, s: usize, w: usize, h: usize, x: usize, y: usize, c: u32) {
    if x >= w || y >= h { return; }
    unsafe { addr.add(y * s + x).write_volatile(fix_color(c)); }
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
/// dithered variants can look noisy on the splash
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

pub mod color {
    pub(crate) const OK: u32 = 0xFF4ECCA3;
    pub(crate) const WARN: u32 = 0xFFFFBD2E;
    pub(crate) const FAULT: u32 = 0xFFFF2A2A;
    #[allow(dead_code)]
    pub(crate) const HEADER: u32 = 0xFF58A6FF;
    #[allow(dead_code)]
    pub(crate) const TEXT: u32 = 0xFFE6F1F5;
}

/// True if the Ring 0 visual overlay is active (framebuffer present and
/// not cleared by clear()). If another screen owner is connected later,
/// it should call `clear()` or otherwise take explicit ownership.
pub fn is_active() -> bool {
    use core::sync::atomic::Ordering;
    if !INITIALIZED.load(Ordering::Relaxed) { return false; }
    unsafe { crate::info::FB_ADDR != 0 }
}

// ── GopFrameBuffer: FrameBuffer trait impl for cabina-panels ──────

pub struct GopFrameBuffer;
pub static mut GOP_FB: GopFrameBuffer = GopFrameBuffer;

impl GopFrameBuffer {
    fn fb_addr(&self) -> *mut u32 {
        unsafe { crate::info::FB_ADDR as *mut u32 }
    }
    fn fb_stride(&self) -> usize {
        unsafe { crate::info::FB_STRIDE as usize }
    }
}

impl cabina_panels::fb::FrameBuffer for GopFrameBuffer {
    fn width(&self) -> u32 {
        unsafe { crate::info::FB_WIDTH as u32 }
    }
    fn height(&self) -> u32 {
        unsafe { crate::info::FB_HEIGHT as u32 }
    }
    fn put_pixel(&mut self, x: u32, y: u32, color: u32) {
        let addr = self.fb_addr();
        let s = self.fb_stride();
        let w = self.width() as usize;
        let h = self.height() as usize;
        put(addr, s, w, h, x as usize, y as usize, color);
    }
    fn fill_rect(&mut self, x: u32, y: u32, w: u32, h: u32, color: u32) {
        let addr = self.fb_addr();
        let s = self.fb_stride();
        let fb_w = self.width() as usize;
        let fb_h = self.height() as usize;
        fill_rect(addr, s, fb_w, fb_h, x as usize, y as usize, w as usize, h as usize, color);
    }
    fn glyph(&self, ch: u8) -> &[u8; 16] {
        font::get_glyph(ch)
    }
    fn now_ns(&self) -> u64 {
        unsafe {
            let lo: u32;
            let hi: u32;
            core::arch::asm!("rdtsc", out("eax") lo, out("edx") hi);
            let tsc = ((hi as u64) << 32) | (lo as u64);
            let freq = crate::cpu::tsc_per_sec();
            if freq == 0 { 0 } else { tsc.wrapping_mul(1_000_000_000) / freq }
        }
    }
}
