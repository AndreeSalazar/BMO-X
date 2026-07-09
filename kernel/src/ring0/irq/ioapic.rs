//! IOAPIC — I/O Advanced Programmable Interrupt Controller.
//!
//! Parses the ACPI MADT table to discover IOAPIC entries,
//! maps their MMIO regions, and configures redirection entries
//! for PCI interrupt lines → LAPIC vectors.

/// Initialize all IOAPICs found in the ACPI MADT.
pub fn init() {
    crate::dev::console::serial_write("[ioapic] init stub\n");
}
