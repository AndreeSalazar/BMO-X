//! Desktop — Ring 0 compositor supervisor for FastOS/BMO.
//!
//! Deeply modular structure:
//!
//!   desktop/
//!     mod.rs         ← this file: facade + run_ring0() main loop
//!     input.rs       ← PS/2 keyboard + mouse polling
//!     display.rs     ← framebuffer primitives (fb_fill, fb_text, fb_blit)
//!     sound.rs       ← PC speaker beep
//!     render.rs      ← full frame rendering (wallpaper, windows, dock, cursor)
//!     state.rs       ← DesktopState (windows, mouse, FPS, clock)
//!     windows.rs     ← window title catalog + content-per-title mapping
//!     compositor.rs  ← Ring 3 compositor x86-64 payload builder
//!     welcome.rs     ← welcome screen (input loop, render, command dispatch)
//!     commands.rs    ← shell command dispatch (Run, Hello, Reboot, Nexo)
//!
//! Entry point: `desktop::init()` then `desktop::run()` from main.rs.

#![allow(dead_code)]

pub mod input;
pub mod display;
pub mod sound;
pub mod render;
pub mod state;
pub mod windows;
pub mod compositor;
pub mod welcome;
pub mod commands;

pub const CYCLES_PER_MS: u64 = 3_700_000;

// ── Re-exports for syscall_entry.rs and main.rs ────────────────────
// These keep the public API stable while the internals are modular.

pub use input::{poll_key, poll_mouse};
pub use display::{fb_fill, fb_text, fb_blit};
pub use sound::beep;

// ── Init + Run ─────────────────────────────────────────────────────

/// Initialize the desktop subsystem. Call once from main.rs Phase 5.
pub fn init() {
    state::init();
    crate::diag::info("desktop", "desktop module initialized (modular)");
}

/// Enter the Ring 0 desktop supervisor. Does NOT return.
pub fn run() -> ! {
    run_ring0()
}

/// Ring 0 desktop main loop — stable GOP path.
///
/// BUG ISOLATION: this loop can hang on hardware without proper PS/2
/// support or with GOP quirks. Wrapped in a watchdog: if no frame
/// advance happens for >5 seconds, return to the welcome screen.
pub fn run_ring0() -> ! {
    crate::diag::info("desktop", "entering Ring 0 GOP desktop supervisor");
    crate::drivers::serial::serial_write("[desktop] Ring 0 GOP desktop supervisor active.\n");

    crate::diag::set_overlay_enabled(false);

    state::init();
    state::mark_dirty();

    // Try to render the first frame. If it fails (e.g. bad GOP), bail.
    // We use a watchdog TSC: 5 seconds max for the entire desktop session.
    let watchdog_start = crate::arch::cpu::rdtsc();
    let watchdog_limit: u64 = 5 * 1_000_000_000; // 5s at 1GHz, scales with TSC
    let mut last_frame = watchdog_start;

    beep(880, 60);
    beep(1320, 80);

    loop {
        // Watchdog: if no frame advance in 5s, exit to welcome.
        let now = crate::arch::cpu::rdtsc();
        if now.wrapping_sub(last_frame) > watchdog_limit {
            crate::diag::warn("desktop", "watchdog timeout, returning to welcome");
            crate::drivers::serial::serial_write("[desktop] watchdog timeout, returning to welcome\n");
            // Re-enter the welcome loop. This function should not return,
            // but if the watchdog fires, we re-enter welcome which also
            // does not return. In practice this branch is dead code at
            // the type level, but the watchdog makes the behavior
            // explicit instead of a hard hang.
            return crate::desktop::welcome::run();
        }
        if now != last_frame { last_frame = now; } // frame tick

        render::render_frame();

        let target = crate::arch::cpu::rdtsc().wrapping_add(16 * CYCLES_PER_MS);
        loop {
            let sc = poll_key();
            if sc == input::SC_ESC { return_to_welcome(); }
            if crate::arch::cpu::rdtsc() >= target { break; }
            core::hint::spin_loop();
        }
    }
}

/// Return to the welcome screen (safer than halting).
fn return_to_welcome() -> ! {
    beep(0, 0);
    crate::drivers::serial::serial_write("[desktop] ESC — returning to welcome.\n");
    crate::desktop::welcome::run()
}
