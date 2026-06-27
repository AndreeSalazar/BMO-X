//! ACPI Sleep (Ring 0 HAL).
//!
//! Handles system sleep states: S3 (suspend-to-RAM) and S5 (power off).
//!
//! Architecture:
//!   - ACPI S3: Save CPU state, PCI config, enter S3 via ACPI register
//!   - ACPI S5: Write SLP_TYP to PM1a/PM1b control registers
//!   - Wakeup: BIOS restores state on power button or RTC alarm
//!
//! ACPI sleep registers (from FADT):
//!   - PM1a_CNT: Sleep type + enable bits
//!   - PM1b_CNT: Secondary sleep control (for S3)
//!   - SLP_TYP_A/B: Sleep type values for each state

use core::sync::atomic::{AtomicBool, Ordering};

/// ACPI sleep register addresses (from FADT).
static mut PM1A_CNT: u16 = 0;
static mut PM1B_CNT: u16 = 0;
static mut SLP_TYP_S3_A: u16 = 0;
static mut SLP_TYP_S3_B: u16 = 0;
static mut SLP_TYP_S5_A: u16 = 0;
static mut SLP_TYP_S5_B: u16 = 0;
static mut SLEEP_ENABLED: bool = false;

/// Initialize ACPI sleep from FADT data.
pub fn init_from_fadt(
    pm1a_cnt: u16,
    pm1b_cnt: u16,
    slp_typ_s3_a: u16,
    slp_typ_s3_b: u16,
    slp_typ_s5_a: u16,
    slp_typ_s5_b: u16,
) {
    unsafe {
        PM1A_CNT = pm1a_cnt;
        PM1B_CNT = pm1b_cnt;
        SLP_TYP_S3_A = slp_typ_s3_a;
        SLP_TYP_S3_B = slp_typ_s3_b;
        SLP_TYP_S5_A = slp_typ_s5_a;
        SLP_TYP_S5_B = slp_typ_s5_b;
        SLEEP_ENABLED = true;
    }

    crate::dev::console::serial_write("[sleep] ACPI sleep initialized\n");
    crate::dev::console::serial_write("[sleep] PM1a_CNT=0x");
    crate::dev::console::serial_write_u64(pm1a_cnt as u64, 16);
    crate::dev::console::serial_write(" PM1b_CNT=0x");
    crate::dev::console::serial_write_u64(pm1b_cnt as u64, 16);
    crate::dev::console::serial_write("\n");
}

/// Enter S3 state (suspend-to-RAM).
///
/// # Safety
/// This function does not return. The system will resume
/// from S3 when the power button is pressed or RTC alarm fires.
pub unsafe fn enter_s3() -> ! {
    if !SLEEP_ENABLED {
        crate::dev::console::serial_write("[sleep] S3 not configured\n");
        loop { core::arch::asm!("hlt"); }
    }

    crate::dev::console::serial_write("[sleep] entering S3 (suspend-to-RAM)\n");

    // TODO: Save CPU state (registers, MSRs)
    // TODO: Save PCI config space
    // TODO: Disable interrupts

    // Write sleep type + SLP_EN to PM control registers
    let slp_en: u16 = 1 << 13; // SLP_EN bit
    let pm1a_val = (SLP_TYP_S3_A << 10) | slp_en;
    let pm1b_val = (SLP_TYP_S3_B << 10) | slp_en;

    // ACPI spec: PM1a_CNT written first, then PM1b_CNT
    if PM1A_CNT != 0 {
        core::ptr::write_volatile(PM1A_CNT as *mut u16, pm1a_val);
    }
    if PM1B_CNT != 0 {
        core::ptr::write_volatile(PM1B_CNT as *mut u16, pm1b_val);
    }

    // If we get here, S3 failed
    crate::dev::console::serial_write("[sleep] S3 FAILED — looping\n");
    loop { core::arch::asm!("hlt"); }
}

/// Enter S5 state (power off).
///
/// # Safety
/// This function does not return. The system will power off.
pub unsafe fn enter_s5() -> ! {
    if !SLEEP_ENABLED {
        crate::dev::console::serial_write("[sleep] S5 not configured\n");
        loop { core::arch::asm!("hlt"); }
    }

    crate::dev::console::serial_write("[sleep] entering S5 (power off)\n");

    let slp_en: u16 = 1 << 13;
    let pm1a_val = (SLP_TYP_S5_A << 10) | slp_en;
    let pm1b_val = (SLP_TYP_S5_B << 10) | slp_en;

    if PM1A_CNT != 0 {
        core::ptr::write_volatile(PM1A_CNT as *mut u16, pm1a_val);
    }
    if PM1B_CNT != 0 {
        core::ptr::write_volatile(PM1B_CNT as *mut u16, pm1b_val);
    }

    loop { core::arch::asm!("hlt"); }
}

/// Check if ACPI sleep is configured.
pub fn is_available() -> bool {
    unsafe { SLEEP_ENABLED }
}
