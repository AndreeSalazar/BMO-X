//! IOAPIC — I/O APIC interrupt redirection.
//!
//! Parses the ACPI MADT table to discover IOAPIC entries,
//! maps their MMIO regions, and configures redirection entries
//! for PCI interrupt lines → LAPIC vectors.

/// Initialize all IOAPICs found in the ACPI MADT.
pub fn init() {
    // TODO: Parse MADT, discover IOAPIC base addresses, map MMIO
}

/// Redirect a PCI interrupt (GSI) to a LAPIC vector.
pub fn redirect(_gsi: u32, _vector: u8) {
    // TODO: Write IOREDTBL entry
}
