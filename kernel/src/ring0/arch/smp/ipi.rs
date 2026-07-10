//! IPI (Inter-Processor Interrupt) delivery.
//!
//! IPIs are sent via the Local APIC's ICR (Interrupt Command Register).
//! They are used for:
//!   - INIT/SIPI (AP startup)
//!   - TLB shootdown
//!   - Scheduler wake-up
//!   - Function call on remote CPU
//!
//! ICR format (64-bit, two 32-bit writes):
//!   Low (0x300): vector[7:0] | delivery[3:0] | dest_mode | level | trigger
//!   High (0x310): destination APIC ID[31:24]

use core::arch::asm;
use super::super::apic;

/// ICR delivery modes.
pub const DELIVERY_FIXED: u32 = 0x00;
pub const DELIVERY_LOWEST: u32 = 0x01;
pub const DELIVERY_SMI: u32 = 0x02;
pub const DELIVERY_NMI: u32 = 0x04;
pub const DELIVERY_INIT: u32 = 0x05;
pub const DELIVERY_SIPI: u32 = 0x06;

/// ICR destination modes.
pub const DEST_PHYSICAL: u32 = 0;
pub const DEST_LOGICAL: u32 = 1;

/// ICR level.
pub const LEVEL_DEASSERT: u32 = 0;
pub const LEVEL_ASSERT: u32 = 1;

/// ICR trigger mode.
pub const TRIGGER_EDGE: u32 = 0;
pub const TRIGGER_LEVEL: u32 = 1;

/// Wait for ICR to be idle (bit 12 = Delivery Status).
unsafe fn wait_icr_idle() {
    let mut attempts = 0;
    while attempts < 100000 {
        let lo = apic::apic_read(apic::APIC_ICR_LO);
        if lo & (1 << 12) == 0 {
            return;
        }
        asm!("pause", options(nostack));
        attempts += 1;
    }
    // Timeout — ICR stuck. Log but continue.
    crate::dev::console::serial_write("[ipi] WARN: ICR busy timeout\n");
}

/// Write the ICR register (64-bit write as two 32-bit writes).
unsafe fn write_icr(high: u32, low: u32) {
    wait_icr_idle();
    apic::apic_write(apic::APIC_ICR_HI, high);
    apic::apic_write(apic::APIC_ICR_LO, low);
    wait_icr_idle();
}

/// Send INIT IPI to a specific APIC ID (edge-triggered, assert).
pub unsafe fn send_init_ipi(target_apic_id: u32) {
    let high = (target_apic_id & 0xFF) << 24;
    let low = (DELIVERY_INIT as u32) | (DEST_PHYSICAL << 11)
            | (LEVEL_ASSERT << 13) | (TRIGGER_EDGE << 14);
    write_icr(high, low);
}

/// Send INIT deassert IPI (used to clear the INIT state).
/// Must be edge-triggered and deassert level — bit 14 (trigger) = 0 (edge).
pub unsafe fn send_init_deinit_apic_ipi() {
    let low = (DELIVERY_INIT as u32) | (DEST_PHYSICAL << 11)
            | (LEVEL_DEASSERT << 13) | (TRIGGER_EDGE << 14);
    write_icr(0, low);
}

/// Send SIPI (Startup IPI) to a specific APIC ID.
/// `vector`: page number (physical address >> 12) where the AP starts.
pub unsafe fn send_sipi(target_apic_id: u32, vector: u8) {
    let high = (target_apic_id & 0xFF) << 24;
    let low = (vector as u32) | (DELIVERY_SIPI as u32)
            | (DEST_PHYSICAL << 11) | (LEVEL_ASSERT << 13)
            | (TRIGGER_EDGE << 14);
    write_icr(high, low);
}


