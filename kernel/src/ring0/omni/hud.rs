//! Live HUD overlay — connects cabina-panels to the kernel main loop.
//!
//! Renders the cabina-panels overlay on the GOP framebuffer using
//! the `GopFrameBuffer` in `visual.rs`.
//!
//! Status: skeleton — needs `overlay::tick()` integrated into
//! the scheduler / main loop.

/// Start the HUD subsystem.
pub fn start() {
    cabina_daemon::info("omni/hud", "HUD overlay registered");

    // Future: call cabina_panels::overlay::init(crate::visual::GopFrameBuffer)
    // Future: scheduler tick calls cabina_panels::overlay::tick(&snapshot)
}
