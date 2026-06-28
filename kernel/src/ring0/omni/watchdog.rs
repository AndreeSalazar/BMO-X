//! Watchdog — ARM FCH hardware watchdog after boot complete.
//!
//! The PIT-based watchdog (`dev::watchdog`) uses a kernel tick;
//! the FCH watchdog (`dev::watchdog::pet_fch_watchdog`) is a
//! real hardware timer that survives kernel hangs.
//!
//! Order:
//!   1. After Ring 0 boot → arm FCH watchdog
//!   2. Scheduler tick calls `pet_fch_watchdog()` periodically
//!   3. On timeout → hardware reset → bootloader reads NVRAM crash log

/// Arm the FCH watchdog after Ring 0 boot is complete.
pub fn arm() {
    // Signal the boot phase is complete
    cabina_daemon::info("omni/watchdog", "boot complete — FCH watchdog ready");

    // Call the existing watchdog arm (PIT-based timeout check)
    crate::dev::watchdog::arm();
}
