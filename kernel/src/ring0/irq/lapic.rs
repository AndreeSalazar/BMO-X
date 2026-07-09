//! Local APIC — timer calibration, EOI, IPI sending.
//!
//! ## Memory Layout
//! The LAPIC is memory-mapped at `IA32_APIC_BASE` (MSR 0x1B).
//! Each core has its own LAPIC at the same physical address
//! but mapped into its own virtual address space.

/// Initialize the LAPIC for the current core.
/// Called during Phase 0 (CPU init) and AP startup.
pub fn init() {
    // TODO: LAPIC timer calibration + periodic mode setup
    crate::dev::console::serial_write("[lapic] init stub\n");
}

/// Send End-Of-Interrupt to the LAPIC.
/// Must be called at the end of every interrupt handler.
pub fn eoi() {
    unsafe {
        core::ptr::write_volatile(LAPIC_BASE.wrapping_add(0xB0) as *mut u32, 0);
    }
}

/// APIC timer initial count register.
static mut LAPIC_BASE: usize = 0;

/// Set the LAPIC base address (called after MMIO mapping).
pub fn set_base(base: u64) {
    unsafe { LAPIC_BASE = base as usize; }
}
