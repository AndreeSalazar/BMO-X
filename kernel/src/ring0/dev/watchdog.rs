//! PIT (Programmable Interval Timer) driver — used as hardware watchdog.
//!
//! Channel 2 is repurposed as a watchdog: the kernel must call
//!
//! v1.6.16: allow(dead_code) — `init` and `arm` are part of the public
//! watchdog API. Phase 4 starts the APIC timer and the PIT watchdog
//! is left dormant; it arms automatically on a future fault path.

#![allow(dead_code)]
//! `pet()` periodically; if it doesn't within `WATCHDOG_TIMEOUT_SECS`,
//! the system resets via keyboard controller (port 0x64, bit 0).
//!
//! Channel 0 is left alone (used by scheduler at 100 Hz).

use core::sync::atomic::{AtomicU64, Ordering};

/// Write to an I/O port. Implemented with direct asm to avoid the
/// `x86_64` crate dependency.
#[inline]
pub unsafe fn outb(port: u16, value: u8) {
    core::arch::asm!("out dx, al", in("dx") port, in("al") value, options(nomem, preserves_flags));
}

/// Watchdog timeout in seconds.
pub const WATCHDOG_TIMEOUT_SECS: u64 = 5;

/// Last time the watchdog was pet (TSC ticks).
static LAST_PET_TSC: AtomicU64 = AtomicU64::new(0);

/// Whether the watchdog is armed.
static ARMED: AtomicU64 = AtomicU64::new(0);

/// Initialize the watchdog (does not arm).
pub fn init() {
    let tsc = crate::cpu::rdtsc();
    LAST_PET_TSC.store(tsc, Ordering::Relaxed);
}

/// Arm the watchdog. After this, if pet() is not called within
/// `WATCHDOG_TIMEOUT_SECS`, the system resets.
pub fn arm() {
    let tsc = crate::cpu::rdtsc();
    LAST_PET_TSC.store(tsc, Ordering::Relaxed);
    ARMED.store(1, Ordering::Relaxed);
    crate::dev::console::serial_write("[watchdog] ARMED (5 sec timeout)\n");
}

/// Disarm the watchdog.
pub fn disarm() {
    ARMED.store(0, Ordering::Relaxed);
    crate::dev::console::serial_write("[watchdog] DISARMED\n");
}

/// Pet the watchdog (reset the timer). Call this periodically.
pub fn pet() {
    let tsc = crate::cpu::rdtsc();
    LAST_PET_TSC.store(tsc, Ordering::Relaxed);
}

/// Pet the AMD FCH hardware watchdog via MMIO.
///
/// Hardware: AMD B550 FCH (Fusion Controller Hub) on Zen 3.
/// Source: Linux kernel sp5100_tco.c, AMD BKDG for Family 17h.
///
/// # Register Map
///
/// ACPI MMIO base: `0xFED8_0000` (identity-mapped by UEFI)
///
///   PM_DECODEEN   (+0x00, u8):  bit 7 = WDT_TMREN
///     On Family 17h+, this bit enables BOTH the WDT MMIO decode
///     AND the watchdog hardware timer itself.
///
///   PM_ISACONTROL (+0x04, u8):  bit 1 = MMIOEN
///     Determines which MMIO base the WDT registers use:
///       MMIOEN=1 → WDT at 0xFED8_0B00 (ACPI_MMIO + 0xB00)
///       MMIOEN=0 → WDT at 0xFEB_0000 (fixed address)
///
/// WDT MMIO registers (u32 read-modify-write):
///
///   WDT_CONTROL (base+0x00):
///     bit 0 = START_STOP  (1 = start watchdog)
///     bit 1 = FIRED       (read-only, set after timeout)
///     bit 2 = ACTION_RESET (0 = reset, 1 = SMI/NMI)
///     bit 3 = DISABLE     (1 = watchdog disabled)
///     bit 7 = TRIGGER     (pet/ping — resets countdown)
///
///   WDT_COUNT (base+0x04, u16): countdown value
///
/// # Our Previous Bug
///
/// We wrote `0x01` (byte) to WDT_CONTROL, which set bit 0 (START_STOP)
/// — that STARTS the watchdog, it does NOT pet it. The correct pet is
/// bit 7 (TRIGGER) via u32 read-modify-write.
pub fn pet_fch_watchdog() {
    unsafe {
        const ACPI_MMIO: u64 = 0xFED8_0000;
        const PM_DECODEEN_OFF: u64 = 0x00;
        const PM_ISACONTROL_OFF: u64 = 0x04;
        const WDT_OFFSET: u64 = 0x0B00;
        const WDT_FIXED: u64 = 0xFEB0_0000;

        const WDT_TRIGGER: u32 = 1 << 7;
        const WDT_DISABLE: u32 = 1 << 3;

        // Step 1: Check PM_DECODEEN bit 7 (WDT_TMREN).
        // If not set, the watchdog hardware is not active — nothing to do.
        let pm_decodeen = core::ptr::read_volatile(
            (ACPI_MMIO + PM_DECODEEN_OFF) as *const u8
        );
        if pm_decodeen & (1 << 7) == 0 {
            return;
        }

        // Step 2: Determine WDT MMIO base from PM_ISACONTROL bit 1.
        let pm_isa = core::ptr::read_volatile(
            (ACPI_MMIO + PM_ISACONTROL_OFF) as *const u8
        );
        let wdt_base = if pm_isa & (1 << 1) != 0 {
            ACPI_MMIO + WDT_OFFSET   // 0xFED80B00
        } else {
            WDT_FIXED                // 0xFEB00000
        };

        // Step 3: Disable the watchdog (bit 3) — read-modify-write u32.
        let ctrl = wdt_base as *mut u32;
        let val = core::ptr::read_volatile(ctrl);
        core::ptr::write_volatile(ctrl, val | WDT_DISABLE);

        // Step 4: Pet as safety net (bit 7 = TRIGGER) — read-modify-write u32.
        let val = core::ptr::read_volatile(ctrl);
        core::ptr::write_volatile(ctrl, val | WDT_TRIGGER);
    }
}

/// Check if the watchdog has expired. Call this from the scheduler tick.
/// If expired, resets the system.
pub fn check() {
    if ARMED.load(Ordering::Relaxed) == 0 { return; }

    let tsc_now = crate::cpu::rdtsc();
    let tsc_per_sec = crate::cpu::tsc_per_sec();
    if tsc_per_sec == 0 { return; }

    let elapsed_secs = (tsc_now - LAST_PET_TSC.load(Ordering::Relaxed)) / tsc_per_sec;
    if elapsed_secs >= WATCHDOG_TIMEOUT_SECS {
        crate::dev::console::serial_write("\n!!! WATCHDOG TIMEOUT — REBOOTING !!!\n");
        // Reset via keyboard controller (port 0x64, bit 0)
        unsafe { outb(0x64, 0xFE); }
        // If reset fails, halt
        loop { unsafe { core::arch::asm!("hlt"); } }
    }
}
