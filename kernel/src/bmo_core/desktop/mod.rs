//! BMO/BMO v1.8.8
//!
//! Desarrolado por Salazar.
//!
//! Desktop â€” Ring 0 compositor supervisor for BMO/BMO.
//!
//! Deeply modular structure:
//!
//!   desktop/
//!     mod.rs         â† this file: facade + run_ring0() main loop
//!     input.rs       â† PS/2 keyboard + mouse polling
//!     display.rs     â† framebuffer primitives (fb_fill, fb_text, fb_blit)
//!     sound.rs       â† PC speaker beep
//!     render.rs      â† full frame rendering (wallpaper, windows, dock, cursor)
//!     state.rs       â† DesktopState (windows, mouse, FPS, clock)
//!     windows.rs     â† window title catalog + content-per-title mapping
//!     compositor.rs  â† Ring 3 compositor x86-64 payload builder
//!     welcome.rs     â† welcome screen (input loop, render, command dispatch)
//!     commands.rs    â† shell command dispatch (Run, Hello, Reboot, Nexo)
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
pub mod theme;
pub mod wallpaper;

pub const CYCLES_PER_MS: u64 = 3_700_000;

// â”€â”€ Re-exports for syscall_entry.rs and main.rs â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€
// These keep the public API stable while the internals are modular.

pub use input::{poll_key, poll_mouse};
pub use display::{fb_fill, fb_text, fb_blit};
pub use sound::beep;

// â”€â”€ Init + Run â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€

/// Initialize the desktop subsystem. Call once from main.rs Phase 5.
pub fn init() {
    state::init();
    crate::cabina::info("desktop", "desktop module initialized (modular)");
}

/// Enter the Ring 0 desktop supervisor. Does NOT return.
pub fn run() -> ! {
    run_ring0()
}

/// Ring 0 desktop main loop â€” stable GOP path.
pub fn run_ring0() -> ! {
    crate::cabina::info("desktop", "entering Ring 0 GOP desktop supervisor");
    crate::dev::console::serial_write("[desktop] Entrando en escritorio Ring 0 supervisor.\n");

    // The desktop owns the screen now. Keep the watchdog and overlay out of the
    // first-frame path so hardware real does not look frozen behind diag.
    crate::dev::watchdog::disarm();
    crate::cabina::set_overlay_enabled(false);

    state::init();
    state::mark_dirty();
    render::render_frame();

    sound::beep(880, 50);

    loop {
        render::render_frame();
        crate::cabina::paint_overlay();

        let target = crate::cpu::rdtsc().wrapping_add(16 * CYCLES_PER_MS);
        loop {
            let sc = input::poll_key();
            if sc == input::SC_ESC { return_to_welcome(); }
            if crate::cpu::rdtsc() >= target { break; }
            core::hint::spin_loop();
        }
    }
}

/// Return to the welcome screen (safer than halting).
fn return_to_welcome() -> ! {
    beep(0, 0);
    crate::dev::console::serial_write("[desktop] ESC â€” returning to welcome.\n");
    crate::bmo_core::desktop::welcome::run()
}



