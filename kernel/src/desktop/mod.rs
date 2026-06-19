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
/// BUG ISOLATION: temporarily a no-op stub to isolate the hang.
/// Returns to welcome immediately so we can debug safely.
pub fn run_ring0() -> ! {
    crate::diag::warn("desktop", "run_ring0: STUBBED for debug, returning to welcome");
    crate::drivers::serial::serial_write("[desktop] run_ring0 STUBBED — returning to welcome\n");

    // Disarm watchdog so the stub can take its time
    crate::drivers::watchdog::disarm();

    // Draw a visible "desktop disabled" marker on the framebuffer so
    // the user knows what happened if serial isn't connected.
    {
        let w = 1920u32;
        let h = 1080u32;
        // Big red rectangle in the middle of the screen
        crate::desktop::display::fb_fill(50, h / 4, w - 100, 60, 0x00FF2A2A);
        // Text overlay
        crate::desktop::display::fb_text(
            100,
            (h / 4 + 20),
            b"[DESKTOP STUBBED] Run 'test' or 'Run' again to continue",
            0xFFFFFFFF,
        );
    }
    return crate::desktop::welcome::run();
}

/// Return to the welcome screen (safer than halting).
fn return_to_welcome() -> ! {
    beep(0, 0);
    crate::drivers::serial::serial_write("[desktop] ESC — returning to welcome.\n");
    crate::desktop::welcome::run()
}
