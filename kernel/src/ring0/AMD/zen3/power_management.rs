//! Power management for the Ryzen 5 5600X.
//!
//! Implements `AMD/ryzen_5_5600x.md` §13 (P-states, C-states y boost).
//!
//! Provides minimal stubs for the OS to:
//! - Halt the CPU until next interrupt (C1 state)
//! - Set P-state frequency hints (P0..Pn)
//! - Read current frequency
//!
//! Status: 🚧 WIP — solo C1 (HALT) implementado. P-states y boost
//! requieren driver ACPI/P-state que está pendiente.
//!
//! References:
//! - AMD Zen 3 Family 19h BKDG, §3.12 (Power Management)

use core::arch::asm;
use super::msr_definitions::{rdmsr, wrmsr};

/// P-state IDs (AMD-specific).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PState {
    P0,  // Max performance (3.7 GHz on 5600X)
    P1,  // Base
    P2,
    P3,
    Pn,  // Min performance
}

/// Halt the CPU until the next interrupt (enters C1 state).
/// This is the most common idle state and is the default for the
/// scheduler's idle loop.
#[inline]
pub fn halt() {
    unsafe {
        // STI; HLT — enable interrupts then halt. The CPU will resume
        // on the next interrupt.
        asm!("sti; hlt", options(nostack, preserves_flags));
    }
}

/// Halt the CPU without enabling interrupts. Use only in critical
/// sections (with interrupts disabled).
#[inline]
pub fn halt_no_sti() {
    unsafe {
        asm!("hlt", options(nostack, preserves_flags));
    }
}

/// Enable C1e (C1 enhanced) for lower idle power. On Zen 3, C1e
/// drops the CPU voltage/frequency slightly while idle.
pub fn enable_c1e() {
    unsafe {
        // MSR 0xC0010055 (PSTATE_CNTL) — bit 28 = C1eOnCmpHalt
        let pstate = rdmsr(0xC001_0055);
        wrmsr(0xC001_0055, pstate | (1u64 << 28));
    }
    crate::dev::console::serial_write("[pm] C1e enabled\n");
}

/// Set the current P-state (request a frequency).
/// Note: On modern systems, the OS sets POLICY (min/max) and the
/// hardware firmware chooses the actual frequency. Setting a specific
/// P-state is generally discouraged; use boost hints instead.
pub fn set_pstate(state: PState) {
    // Implementation deferred — see AMD Family 19h BKDG §3.12.
    // For now, we just log the request.
    let s = match state {
        PState::P0 => "P0",
        PState::P1 => "P1",
        PState::P2 => "P2",
        PState::P3 => "P3",
        PState::Pn => "Pn",
    };
    crate::dev::console::serial_write("[pm] P-state request: ");
    crate::dev::console::serial_write(s);
    crate::dev::console::serial_write(" (no-op in current driver)\n");
}

/// Get the current P-state.
pub fn current_pstate() -> PState {
    // Read MSR C0010061 (P-State Status)
    unsafe {
        let status = rdmsr(0xC001_0061);
        let pstate_num = status & 0x07;
        match pstate_num {
            0 => PState::P0,
            1 => PState::P1,
            2 => PState::P2,
            3 => PState::P3,
            _ => PState::Pn,
        }
    }
}

/// Initialize the power management subsystem.
pub fn init() {
    enable_c1e();
}
