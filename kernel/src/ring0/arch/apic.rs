#![allow(dead_code)]

//! Local APIC timer — preemptive scheduling tick.
//!
//! Uses the Local APIC's periodic timer for ctx switch interrupts.
//! Vector 48 (0x30) is used for the timer interrupt.

use core::arch::asm;

/// APIC timer interrupt vector.
pub const APIC_TIMER_VECTOR: u8 = 48;

// Local APIC register offsets (from APIC base, memory-mapped)
pub const APIC_ID:           u32 = 0x020;
const APIC_VERSION:      u32 = 0x030;
const APIC_TPR:          u32 = 0x080;
const APIC_EOI:          u32 = 0x0B0;
const APIC_SPURIOUS:     u32 = 0x0F0;
pub const APIC_ICR_LO:      u32 = 0x300;
pub const APIC_ICR_HI:      u32 = 0x310;
const APIC_TIMER_LVT:   u32 = 0x320;
const APIC_TIMER_INIT:  u32 = 0x380;
const APIC_TIMER_CUR:   u32 = 0x390;
const APIC_TIMER_DIV:   u32 = 0x3E0;

/// MSR for APIC base address.
const IA32_APIC_BASE_MSR: u32 = 0x1B;

/// Global APIC base address (set during init).
static mut APIC_BASE: u64 = 0;

/// Read APIC base from MSR.
fn read_apic_base() -> u64 {
    let (lo, hi): (u32, u32);
    unsafe {
        asm!("rdmsr", in("ecx") IA32_APIC_BASE_MSR,
             out("eax") lo, out("edx") hi, options(nostack));
    }
    (((hi as u64) << 32) | (lo as u64)) & 0xFFFF_F000
}

/// Write to APIC register.
#[inline]
pub unsafe fn apic_write(offset: u32, val: u32) {
    let ptr = (APIC_BASE + offset as u64) as *mut u32;
    core::ptr::write_volatile(ptr, val);
}

/// Read from APIC register.
#[inline]
pub unsafe fn apic_read(offset: u32) -> u32 {
    let ptr = (APIC_BASE + offset as u64) as *const u32;
    core::ptr::read_volatile(ptr)
}

/// Read the LAPIC ID of the current core.
pub fn read_lapic_id() -> u32 {
    unsafe { apic_read(APIC_ID) >> 24 }
}

/// Send End-Of-Interrupt to APIC. Must be called at the end of every APIC interrupt handler.
pub fn apic_eoi() {
    unsafe { apic_write(APIC_EOI, 0); }
}

/// Get APIC ID.
pub fn apic_id() -> u32 {
    unsafe { (apic_read(APIC_ID) >> 24) & 0xFF }
}

/// Initialize the Local APIC and start the periodic timer.
///
/// `tick_hz` = approximate desired timer frequency (e.g., 1000 for 1ms ticks).
/// The actual frequency depends on the bus clock, so we calibrate against PIT.
pub fn init_apic(tick_hz: u32) {
    unsafe {
        APIC_BASE = read_apic_base();

        // Enable APIC: set bit 8 of spurious vector register + set spurious vector to 0xFF
        let spurious = apic_read(APIC_SPURIOUS);
        apic_write(APIC_SPURIOUS, spurious | 0x1FF); // Enable + vector 0xFF

        // Set Task Priority to 0 (accept all interrupts)
        apic_write(APIC_TPR, 0);

        // Calibrate APIC timer using a known delay
        // Divide by 16
        apic_write(APIC_TIMER_DIV, 0x03); // divide by 16

        // Set initial count to max for calibration
        apic_write(APIC_TIMER_INIT, 0xFFFF_FFFF);

        // Wait ~10ms using PIT channel 2 for calibration
        pit_wait_10ms();

        // Read how many ticks elapsed
        let elapsed = 0xFFFF_FFFF - apic_read(APIC_TIMER_CUR);

        // Calculate initial count for desired frequency
        // elapsed ticks in 10ms → ticks_per_second = elapsed * 100
        // For tick_hz: initial_count = ticks_per_second / tick_hz
        let ticks_per_sec = (elapsed as u64) * 100;
        let initial_count = (ticks_per_sec / tick_hz as u64) as u32;

        // Set up periodic timer on our vector
        apic_write(APIC_TIMER_LVT, APIC_TIMER_VECTOR as u32 | (1 << 17)); // Periodic mode
        apic_write(APIC_TIMER_DIV, 0x03); // divide by 16
        apic_write(APIC_TIMER_INIT, if initial_count > 0 { initial_count } else { 1000 });
    }
}

/// Disable the APIC timer (stop periodic interrupts).
#[allow(dead_code)]
pub fn stop_apic_timer() {
    unsafe {
        apic_write(APIC_TIMER_LVT, 1 << 16); // Mask the timer
    }
}

/// Use PIT channel 2 to wait approximately 10ms for APIC timer calibration.
fn pit_wait_10ms() {
    // PIT frequency = 1,193,182 Hz
    // 10ms = 11,932 counts
    const PIT_10MS: u16 = 11932;
    const MAX_RETRIES: u32 = 100;

    unsafe {
        // PIT channel 2 mode 0 (interrupt on terminal count)
        asm!("out 0x61, al", in("al") 0x00u8, options(nostack)); // gate off
        asm!("out 0x43, al", in("al") 0xB0u8, options(nostack)); // channel 2, lobyte/hibyte, mode 0
        asm!("out 0x42, al", in("al") (PIT_10MS & 0xFF) as u8, options(nostack));
        asm!("out 0x42, al", in("al") (PIT_10MS >> 8) as u8, options(nostack));

        // Reset channel 2 gate to start counting
        let mut val: u8;
        asm!("in al, 0x61", out("al") val, options(nostack));
        asm!("out 0x61, al", in("al") val & 0xFC, options(nostack)); // gate low
        asm!("out 0x61, al", in("al") (val & 0xFC) | 1, options(nostack)); // gate high (start)

        // Wait for PIT output (bit 5 of port 0x61) — with timeout
        let mut retries = 0u32;
        loop {
            asm!("in al, 0x61", out("al") val, options(nostack));
            if val & 0x20 != 0 { break; }
            retries += 1;
            if retries >= MAX_RETRIES {
                // PIT channel 2 not responding — use RDTSC fallback
                crate::dev::console::serial_write("[APIC] PIT ch2 timeout, using RDTSC fallback\n");
                // RDTSC fallback: spin for ~10ms at ~3.7GHz = ~37M ticks
                let start = crate::cpu::rdtsc();
                while crate::cpu::rdtsc().wrapping_sub(start) < 37_000_000 {
                    asm!("pause", options(nostack));
                }
                return;
            }
        }
    }
}
