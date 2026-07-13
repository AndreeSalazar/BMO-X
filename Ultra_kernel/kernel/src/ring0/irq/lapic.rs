//! Local APIC timer — calibration, periodic mode, EOI.
//!
//! Uses PIT channel 2 for calibration: programs APIC timer in one-shot
//! mode, waits 10ms via PIT, reads elapsed ticks, calculates the correct
//! initial count for the desired tick frequency.
//!
//! ## APIC register offsets (from APIC base)
//!
//! All offsets are 32-bit aligned. The APIC base is discovered via
//! MSR 0x1B (IA32_APIC_BASE), masked to page alignment (0xFFFF_0000).

use core::arch::asm;

// ── Register offsets ────────────────────────────────────────────────────

const APIC_ID:      u32 = 0x020;
#[allow(dead_code)]
const APIC_VERSION: u32 = 0x030;
const APIC_TPR:     u32 = 0x080;
const APIC_EOI:     u32 = 0x0B0;
const APIC_SPURIOUS: u32 = 0x0F0;
#[allow(dead_code)]
const APIC_ICR_LO:  u32 = 0x300;
#[allow(dead_code)]
const APIC_ICR_HI:  u32 = 0x310;
const APIC_LVT_TIMER:  u32 = 0x320;
const APIC_INIT_COUNT: u32 = 0x380;
const APIC_CUR_COUNT:  u32 = 0x390;
const APIC_DIV_CONF:   u32 = 0x3E0;

/// Timer interrupt vector — IDT entry 48.
pub const TIMER_VECTOR: u8 = 48;

/// MSR holding the LAPIC base address.
const IA32_APIC_BASE_MSR: u32 = 0x1B;

static mut BASE: u64 = 0;

// ── Low-level register I/O ──────────────────────────────────────────────

fn read_base() -> u64 {
    let (lo, hi): (u32, u32);
    unsafe {
        asm!("rdmsr", in("ecx") IA32_APIC_BASE_MSR,
             out("eax") lo, out("edx") hi, options(nostack));
    }
    (((hi as u64) << 32) | (lo as u64)) & 0xFFFF_0000
}

#[inline]
unsafe fn write_reg(offset: u32, val: u32) {
    core::ptr::write_volatile((BASE + offset as u64) as *mut u32, val);
}

#[inline]
unsafe fn read_reg(offset: u32) -> u32 {
    core::ptr::read_volatile((BASE + offset as u64) as *const u32)
}

// ── Public API ──────────────────────────────────────────────────────────

/// Get the APIC base address (for MSI message address).
pub fn base() -> u64 {
    unsafe { BASE }
}

/// Send End-Of-Interrupt.
pub fn eoi() {
    unsafe { write_reg(APIC_EOI, 0); }
}

/// Get LAPIC ID.
pub fn id() -> u32 {
    unsafe { read_reg(APIC_ID) >> 24 }
}

// ── PIT calibration ─────────────────────────────────────────────────────

/// Wait ~10ms using PIT channel 2. Returns success.
fn pit_wait_10ms() -> bool {
    const PIT_COUNT: u16 = 11_932; // 1_193_182 Hz × 10ms
    const MAX_POLLS: u32 = 1_000_000;

    unsafe {
        // Mode 0 (interrupt on terminal count), channel 2, lobyte/hibyte
        asm!("out 0x43, al", in("al") 0xB0u8, options(nostack));
        asm!("out 0x42, al", in("al") (PIT_COUNT & 0xFF) as u8, options(nostack));
        asm!("out 0x42, al", in("al") (PIT_COUNT >> 8) as u8, options(nostack));

        // Toggle gate on
        let mut val: u8;
        asm!("in al, 0x61", out("al") val, options(nostack));
        asm!("out 0x61, al", in("al") val & !0x03, options(nostack));
        asm!("out 0x61, al", in("al") (val & !0x03) | 0x01, options(nostack));

        // Poll output bit
        for _ in 0..MAX_POLLS {
            asm!("in al, 0x61", out("al") val, options(nostack));
            if val & 0x20 != 0 { return true; }
        }
        false
    }
}

// ── Initialization ──────────────────────────────────────────────────────

/// Initialize the LAPIC and start the periodic timer.
///
/// `tick_hz`: desired ticks per second (e.g., 100 for 10ms intervals).
pub fn init(tick_hz: u32) {
    unsafe {
        BASE = read_base();
        if BASE == 0 {
            crate::ring0::dev::console::serial_write("[lapic] FATAL: APIC base is 0 (no APIC?)\n");
            return;
        }
        crate::ring0::dev::console::serial_write("[lapic] base=0x");
        crate::ring0::dev::console::serial_write_u64(BASE, 16);
        crate::ring0::dev::console::serial_write(" id=0x");
        crate::ring0::dev::console::serial_write_u64(id() as u64, 16);
        crate::ring0::dev::console::serial_write(" target=");
        crate::ring0::dev::console::serial_write_u64(tick_hz as u64, 10);
        crate::ring0::dev::console::serial_write(" Hz\n");

        // 1. Enable LAPIC (spurious vector = 0xFF + enable bit 8)
        let sp = read_reg(APIC_SPURIOUS);
        write_reg(APIC_SPURIOUS, sp | 0x1FF);

        // 2. Task Priority = 0 (accept all interrupts)
        write_reg(APIC_TPR, 0);

        // 3. Timer LVT: one-shot, unmasked, vector 48
        write_reg(APIC_LVT_TIMER, TIMER_VECTOR as u32);

        // 4. Divide configuration = 16
        write_reg(APIC_DIV_CONF, 0x03);

        // 5. Calibrate: write max count, wait 10ms, read elapsed
        write_reg(APIC_INIT_COUNT, 0xFFFF_FFFF);

        if !pit_wait_10ms() {
            // Fallback: 1M ticks
            crate::ring0::dev::console::serial_write("[lapic] PIT calibration timeout, using fallback\n");
            write_reg(APIC_INIT_COUNT, 1_000_000);
            write_reg(APIC_LVT_TIMER, TIMER_VECTOR as u32 | (1 << 17));
            return;
        }

        let elapsed = 0xFFFF_FFFFu32.wrapping_sub(read_reg(APIC_CUR_COUNT)) as u64;
        let ticks_per_sec = elapsed.saturating_mul(100);
        if ticks_per_sec == 0 {
            crate::ring0::dev::console::serial_write("[lapic] zero ticks per second, using fallback\n");
            write_reg(APIC_INIT_COUNT, 1_000_000);
            write_reg(APIC_LVT_TIMER, TIMER_VECTOR as u32 | (1 << 17));
            return;
        }

        let count = ((ticks_per_sec / tick_hz as u64) as u32).max(1);

        crate::ring0::dev::console::serial_write("[lapic] calibrated: ");
        crate::ring0::dev::console::serial_write_u64(ticks_per_sec, 10);
        crate::ring0::dev::console::serial_write(" ticks/sec, count=");
        crate::ring0::dev::console::serial_write_u64(count as u64, 10);
        crate::ring0::dev::console::serial_write(" for ");
        crate::ring0::dev::console::serial_write_u64(tick_hz as u64, 10);
        crate::ring0::dev::console::serial_write(" Hz\n");

        // 6. Switch to periodic mode
        write_reg(APIC_LVT_TIMER, TIMER_VECTOR as u32 | (1 << 17));
        write_reg(APIC_DIV_CONF, 0x03);
        write_reg(APIC_INIT_COUNT, count);
    }
}
