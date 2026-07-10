//! Power Management ??? ACPI S3/S4/S5 + CPU halt.
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

/// Try to perform a clean ACPI shutdown by writing SLP_TYP|SLP_EN to
/// PM1a_CNT. The PM1a_CNT port is read from the FADT (cached by
/// `vendor::amd::cpu::zen3::acpi_real`).
///
/// Returns true if a clean shutdown was attempted; false if the
/// FADT did not provide a port (and we should fall back to triple
/// fault).
fn try_acpi_shutdown() -> bool {
    // PM1a_CNT port comes from the FADT. If the ACPI module was not
    // initialized, fall through to the legacy triple-fault path.
    let port = match crate::vendor::amd::cpu::zen3::acpi_real::pm1a_control_port() {
        Some(p) => p,
        None => return false,
    };
    unsafe {
        let val: u16 = (5u16 << 10) | (1u16 << 13); // SLP_TYP=5 (soft off) | SLP_EN
        asm!("out dx, ax", in("dx") port, in("ax") val, options(nostack, nomem));
    }
    true
}

/// Shutdown the system via ACPI. Falls back to triple fault if
/// the FADT does not provide PM1a_CNT.
pub fn shutdown() -> ! {
    crate::dev::console::serial_write("[power] shutdown initiated\n");

    unsafe { asm!("cli"); }

    if try_acpi_shutdown() {
        // Give the chipset ~500ms to power off
        for _ in 0..50u32 {
            unsafe { asm!("hlt"); }
        }
        // Some boards ignore the SLP_EN write; fall through to
        // keyboard controller reset as a last resort.
        crate::dev::console::serial_write("[power] ACPI shutdown did not take effect, falling back\n");
    }

    // Triple-fault fallback: write a bad IDT and trigger interrupt 0
    unsafe {
        asm!("lidt [{}]", in(reg) 0u64, options(nostack, nomem));
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
        // Wait for input buffer empty (with bounded loop)
        for _ in 0..1_000_000u32 {
            let status: u8;
            asm!("in al, dx", out("al") status, in("dx") 0x64u16, options(nostack, nomem));
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
    // PM1a_CNT is discovered from the FADT by the ACPI module.
    // Nothing to do here at this layer.
}
