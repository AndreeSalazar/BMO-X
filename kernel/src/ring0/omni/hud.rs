use core::sync::atomic::{AtomicBool, Ordering};
use cabina_panels::overlay::Overlay;

static mut OVERLAY: Overlay = Overlay::new();
static ACTIVE: AtomicBool = AtomicBool::new(false);

/// Start the HUD subsystem.
pub fn start() {
    ACTIVE.store(true, Ordering::Relaxed);
    cabina_daemon::info("omni/hud", "HUD overlay active");
}

/// Called from scheduler tick to repaint the overlay.
pub fn tick() {
    if !ACTIVE.load(Ordering::Relaxed) { return; }
    let snapshot = cabina_daemon::take_snapshot();
    unsafe {
        OVERLAY.paint(&mut crate::visual::GOP_FB, &snapshot);
    }
}

/// Enable/disable the HUD.
pub fn toggle() {
    let active = ACTIVE.load(Ordering::Relaxed);
    ACTIVE.store(!active, Ordering::Relaxed);
}
