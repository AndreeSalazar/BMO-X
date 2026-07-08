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

/// Paint the overlay once (called from HAL when bmo_core requests it).
pub fn paint() {
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

/// Enable the HUD explicitly.
pub fn set_enabled(on: bool) {
    ACTIVE.store(on, Ordering::Relaxed);
    if on { start(); }
}

/// Is the HUD currently active?
pub fn is_active() -> bool {
    ACTIVE.load(Ordering::Relaxed)
}

/// Cycle to next tab (called from HAL).
pub fn cycle_tab() {
    unsafe { OVERLAY.cycle_tab(); }
}

/// Cycle to next query (called from HAL).
pub fn cycle_query() -> bool {
    unsafe {
        OVERLAY.cycle_query();
        true
    }
}
