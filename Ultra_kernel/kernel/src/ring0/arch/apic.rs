//! APIC wrapper — delegates to `irq::lapic`.
//!
//! This file exists for backwards compatibility. New code should use
//! `irq::lapic::init()` and `irq::lapic::eoi()` directly.

// Re-export constants for existing callers
pub const APIC_TIMER_VECTOR: u8 = crate::ring0::irq::lapic::TIMER_VECTOR;
pub const APIC_ID:         u32 = 0x020;
pub const APIC_SPURIOUS:   u32 = 0x0F0;
pub const APIC_ICR_LO:     u32 = 0x300;
pub const APIC_ICR_HI:     u32 = 0x310;

/// Initialize APIC timer (delegates to irq::lapic).
pub fn init_apic(tick_hz: u32) {
    crate::ring0::irq::lapic::init(tick_hz);
}

/// Send EOI (delegates to irq::lapic).
pub fn apic_eoi() {
    crate::ring0::irq::lapic::eoi();
}

/// Low-level register write (for existing callers in smp/).
pub unsafe fn apic_write(offset: u32, val: u32) {
    let base = crate::ring0::irq::lapic::base();
    core::ptr::write_volatile((base + offset as u64) as *mut u32, val);
}

/// Low-level register read (for existing callers in smp/).
pub unsafe fn apic_read(offset: u32) -> u32 {
    let base = crate::ring0::irq::lapic::base();
    core::ptr::read_volatile((base + offset as u64) as *const u32)
}
