//! Shell commands — command dispatch for the welcome screen.

#![allow(dead_code)]

/// Case-insensitive comparison.
pub fn eq_ci(a: &[u8], b: &[u8]) -> bool {
    if a.len() != b.len() { return false; }
    for i in 0..a.len() {
        let ca = a[i].to_ascii_lowercase();
        let cb = b[i].to_ascii_lowercase();
        if ca != cb { return false; }
    }
    true
}

/// Check if command should enter the desktop.
pub fn should_enter_desktop(cmd: &[u8]) -> bool {
    eq_ci(cmd, b"run") || eq_ci(cmd, b"desktop") || eq_ci(cmd, b"start") || eq_ci(cmd, b"go")
}

/// Trim whitespace from both ends of a byte slice.
pub fn trim(s: &[u8]) -> &[u8] {
    let mut i = 0;
    while i < s.len() && s[i] == b' ' { i += 1; }
    let mut j = s.len();
    while j > i && s[j-1] == b' ' { j -= 1; }
    &s[i..j]
}

/// Enter the desktop environment (calls into sched::user_init).
///
/// v1.6.18: DESKTOP STUBBED — refined stub screen with:
///   - Centered card (1080 wide × 380 tall, well within 1920×1080)
///   - Clear "in development" badge at the top
///   - Countdown bar inside the card, with 5 segments showing progress
///   - "REBOOT" hint below the bar in warm orange
///   - 5 s countdown animation that loops the bar color from mint → gold → red
pub fn enter_desktop() {
    crate::diag::set_overlay_enabled(false);
    crate::diag::info("welcome", "Run accepted; desktop STUBBED in v1.6.18 (REBOOT to return to welcome)");
    crate::drivers::serial::serial_write("[welcome] Run aceptado: DESKTOP STUBBED v1.6.18. Reboot to recover.\n");

    let w = 1920u32;
    let h = 1080u32;

    // 1) Background — solid dark
    crate::desktop::display::fb_fill(0, 0, w, h, 0xFF050B12);

    // 2) Centered card (1080×380) — well within the 1920×1080 frame
    let cw = 1080u32;
    let ch = 380u32;
    let cx = (w - cw) / 2;
    let cy = (h - ch) / 2;
    // Outer warm-orange border (8 px thick)
    crate::desktop::display::fb_fill(cx - 4, cy - 4, cw + 8, ch + 8, 0xFFE07832);
    // Inner dark slate
    crate::desktop::display::fb_fill(cx, cy, cw, ch, 0xFF0F1827);

    // 3) Top accent bar (mint, 4 px) — visual interest
    crate::desktop::display::fb_fill(cx + 8, cy + 8, cw - 16, 3, 0xFF4ECCA3);

    // 4) "DESKTOP STUB" badge (small pill in upper-left of card)
    let badge_x = cx + 40;
    let badge_y = cy + 40;
    crate::desktop::display::fb_fill(badge_x, badge_y, 220, 36, 0xFF3A1B0E);
    crate::desktop::display::fb_text(
        badge_x + 16,
        badge_y + 10,
        b"DESKTOP STUB  v1.6.18",
        0xFFFFAA3D,
    );

    // 5) Main title (gold, big-ish)
    crate::desktop::display::fb_text(
        cx + 40,
        cy + 110,
        b"FastOS-BMO  ::  Ring 0 Desktop",
        0xFFE2C044,
    );

    // 6) Subtitle / explanation
    crate::desktop::display::fb_text(
        cx + 40,
        cy + 150,
        b"Disabled in v1.6.x for stability while we harden the",
        0xFFCBD7E0,
    );
    crate::desktop::display::fb_text(
        cx + 40,
        cy + 170,
        b"ECAM/heap path. Phase 5 (desktop) is up next in the queue.",
        0xFFCBD7E0,
    );

    // 7) Countdown bar (5 segments inside the card, no overflow)
    let bar_x = cx + 40;
    let bar_y = cy + 240;
    let bar_total_w = cw - 80;
    let seg_w = bar_total_w / 5;
    let seg_h = 28;
    // Background track
    crate::desktop::display::fb_fill(bar_x - 4, bar_y - 4, bar_total_w + 8, seg_h + 8, 0xFF050B12);
    crate::desktop::display::fb_fill(bar_x, bar_y, bar_total_w, seg_h, 0xFF1F2A38);

    // 8) REBOOT hint below the bar
    crate::desktop::display::fb_text(
        cx + 40,
        cy + 310,
        b"REBOOT the PC to return to the welcome screen.",
        0xFFFFAA3D,
    );

    // 9) Animated countdown: fill segments one by one over ~5 s.
    //
    // v1.6.13 FIX: the previous version computed `partial` as
    // `(step * 5) % total_steps` which ranges 0..99 instead of 0..(total_steps/5).
    // When `step=99`, partial=95 and the active segment was filled to
    // 95/20 = 4.75× its own width, painting a bar that extended far
    // beyond the card. The correct math: each segment fills over
    // `total_steps / num_segments` frames, so partial_in_sub must be
    // 0..(total_steps/num_segments) and the fill width is
    // `(seg_w - gap) * partial_in_sub / steps_per_segment`.
    let total_steps: u32 = 100;
    let total_ms: u32 = 5_000;
    let step_ms: u32 = total_ms / total_steps;
    let num_segments: u32 = 5;
    let steps_per_segment: u32 = total_steps / num_segments; // 20
    for step in 0..total_steps {
        // Re-paint track to clear previous fill
        crate::desktop::display::fb_fill(bar_x, bar_y, bar_total_w, seg_h, 0xFF1F2A38);
        // Which segment is currently filling?
        let active = step / steps_per_segment;            // 0..5
        let sub_step = step % steps_per_segment;          // 0..19
        for i in 0..num_segments {
            let sxi = bar_x + i * seg_w;
            if i < active {
                // Fully filled
                let c = match i {
                    0 => 0xFF4ECCA3, // mint
                    1 => 0xFF4ECCA3,
                    2 => 0xFFE2C044, // gold
                    3 => 0xFFE2C044,
                    _ => 0xFFFF7B72,  // red
                };
                crate::desktop::display::fb_fill(sxi + 2, bar_y, seg_w - 4, seg_h, c);
            } else if i == active && sub_step > 0 {
                // Partial fill on the active segment.
                // sub_step ranges 0..steps_per_segment. Map to 0..seg_w-4.
                let pw = ((seg_w - 4) * sub_step) / steps_per_segment;
                let c = match i {
                    0 | 1 => 0xFF4ECCA3,
                    2 | 3 => 0xFFE2C044,
                    _ => 0xFFFF7B72,
                };
                crate::desktop::display::fb_fill(sxi + 2, bar_y, pw, seg_h, c);
            }
        }
        crate::arch::cpu::busy_wait_ms(step_ms as u64);
    }

    // 10) HLT loop (rebooting is the only way back)
    loop {
        unsafe { core::arch::asm!("hlt"); }
    }
}

/// Test compile a ÑEXO program.
pub fn nexo_test_compile() {
    use crate::lang::nexo;
    let source = b"fn main() -> num { retorna 42 }\n";
    crate::diag::info("nexo", "Compiling test program");
    match nexo::compile(source) {
        Ok(bytes) => {
            crate::diag::info("nexo", "Compilation succeeded");
            crate::diag::info_u64("nexo", "Generated bytes", bytes.len() as u64);
            crate::diag::info_u64("nexo", "First byte", bytes.first().copied().unwrap_or(0) as u64);
        }
        Err(_e) => {
            crate::diag::warn("nexo", "Compilation failed");
        }
    }
}
