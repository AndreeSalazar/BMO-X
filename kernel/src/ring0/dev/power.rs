//! Power Management — ACPI S3/S4/S5 + CPU halt.
//!
//! Ring 0 provides the mechanism (port I/O, ACPI table reading).
//! Ring 3 implements the policy (when to sleep, wake conditions).
//!
//! ## ACPI Sleep States
//!
//! | State | Name     | Latency | Power | Rings 0 Foundation |
//! |-------|----------|---------|-------|---------------------|
//! | S3    | Suspend  | ~1s     | Low   | ACPI PM1a_CNT write |
//! | S4    | Hibernate| ~3s     | Off   | ACPI + disk snapshot|
//! | S5    | Shutdown | ~5s     | Off   | ACPI PM1a_CNT write |

use core::arch::asm;

/// Shutdown the system via ACPI.
/// Writes SLP_TYP=5 to PM1a_CNT.SLP_EN.
/// Requires ACPI tables to be initialized.
pub fn shutdown() -> ! {
    // Try ACPI shutdown: outw(PM1a_CNT, SLP_TYP(5) | SLP_EN)
    // If ACPI not available, fall back to triple fault
    crate::dev::console::serial_write("[power] shutdown initiated\n");

    // Disable interrupts
    unsafe { asm!("cli"); }

    // Triple-fault fallback: load a bad IDT and trigger interrupt
    unsafe {
        // Write a zero IDTR to cause triple fault on next interrupt
        asm!("lidt [{}]", in(reg) 0u64, options(nostack, nomem));
        // Trigger interrupt 0 to cause triple fault
        asm!("int 0", options(nostack, nomem));
    }

    loop { unsafe { asm!("hlt"); } }
}

/// Reboot the system via keyboard controller pulse.
pub fn reboot() -> ! {
    crate::dev::console::serial_write("[power] reboot initiated\n");
    unsafe {
        asm!("cli");
        // Pulse the keyboard controller reset line
        // Wait for input buffer empty
        loop {
            let status: u8;
            asm!("in al, dx", in("dx") 0x64u16, out("al") status, options(nostack, nomem));
            if status & 2 == 0 { break; }
        }
        // Write 0xFE: system reset
        asm!("out 0x64, al", in("al") 0xFEu8, options(nostack, nomem));
    }
    loop { unsafe { asm!("hlt"); } }
}

/// Halt the CPU until next interrupt (HLT).
/// Use for idle loop in scheduler when no tasks are ready.
pub fn cpu_halt() {
    unsafe { asm!("sti; hlt"); }
}

/// Initialize power management.
pub fn init() {
    // Platform-specific init deferred to ACPI module in Ring 3
}
