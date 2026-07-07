//! Power Manager (Ring 0 HAL).
//!
//! Manages CPU power states, thermal monitoring, and system sleep:
//!   - C-states: CPU idle states (C0/C1/C2/C3) for power saving
//!   - Thermal: Temperature monitoring, overheat protection
//!   - Sleep: ACPI S3 (suspend-to-RAM), S4 (hibernate)


/// System power state.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PowerState {
    Working,
    Idle,
    Sleep,
    Hibernate,
    Off,
}

/// Initialize the power management subsystem.
pub fn init() {
    crate::dev::console::serial_write("[power] initializing\n");
    crate::dev::cstates::init();
    crate::dev::thermal::init();
    crate::dev::console::serial_write("[power] initialized\n");
}
