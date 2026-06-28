//! Omniscient infrastructure — Ring 0 HAL extensions.
//!
//! Wires the missing pieces for the "omniscient" vision:
//!
//!   1. **HPET**   — Parse ACPI HPET table before timer init
//!   2. **Persist** — Flush cabina-daemon ring buffer to NVRAM/SSD
//!   3. **Watchdog** — ARM FCH hardware watchdog after boot
//!   4. **HUD**     — Live cabina-panels overlay connector
//!
//! Each subsystem can be enabled/disabled independently.
//! Call order: `init_early()` → boot → `init_late()`

pub mod hpet;
pub mod persist;
pub mod watchdog;
pub mod hud;

/// Early init — must run BEFORE phase0_arch (HPET needs ACPI base).
pub fn init_early(boot_info_ptr: *const fastos_boot_protocol::BootInfo) {
    hpet::init_early(boot_info_ptr);
}

/// Late init — run after Ring 0 boot is complete.
pub fn init_late() {
    watchdog::arm();
    persist::start();
    hud::start();
}
