//! Interrupt subsystem — LAPIC, IOAPIC, MSI/MSI-X.
//!
//! ```text
//! Hardware IRQ → IDT vector → irq::dispatch(vector) → registered handler
//! ```
//!
//! ## Initialization order
//! 1. `lapic::init()` — calibrate APIC timer, map LAPIC MMIO
//! 2. `ioapic::init()` — parse ACPI MADT, configure IOAPIC redirections
//! 3. `msi::init()` — enable MSI/MSI-X for PCI devices (XHCI, AHCI, NVMe)

pub mod lapic;
pub mod ioapic;
pub mod msi;
