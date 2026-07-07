
//! Local APIC timer — preemptive scheduling tick.
//!
//! Vector 48 (0x30) is the timer interrupt. Phase 4 calls
//! `init_apic(tick_hz)` to start periodic ticks at the requested rate.
//!
//! v1.8.9: bug fix crítico de calibración. La versión anterior ponía
//! `INIT = 0xFFFF_FFFF` y luego leía `CUR` esperando que el timer
//! hubiera contado — pero el timer NO corre hasta que se programa el
//! LVT. Resultado: `elapsed = 0xFFFF_FFFF`, `initial_count = 4.3B`
//! → el APIC interrumpía una vez cada 43 segundos a 100 Hz, haciendo
//! que el watchdog (alimentado por APIC ticks) se disparara y
//! reseteaba la máquina.
//!
//! v1.8.9: programar LVT en modo one-shot **antes** de la calibración,
//! contar con PIT, luego cambiar a modo periódico y reprogramar INIT
//! con el valor calibrado.

use core::arch::asm;

// ── APIC register offsets (memory-mapped from APIC base) ─────────────

pub const APIC_ID:          u32 = 0x020;
const APIC_VERSION:        u32 = 0x030;
const APIC_TPR:            u32 = 0x080;
const APIC_EOI:            u32 = 0x0B0;
pub const APIC_SPURIOUS:       u32 = 0x0F0;
pub const APIC_ICR_LO:      u32 = 0x300;
pub const APIC_ICR_HI:      u32 = 0x310;
pub const APIC_TIMER_LVT:      u32 = 0x320;
pub const APIC_TIMER_INIT:     u32 = 0x380;
pub const APIC_TIMER_CUR:      u32 = 0x390;
pub const APIC_TIMER_DIV:      u32 = 0x3E0;

/// Timer interrupt vector — wired to IDT entry 48.
pub const APIC_TIMER_VECTOR: u8 = 48;

/// MSR holding the LAPIC base address.
const IA32_APIC_BASE_MSR: u32 = 0x1B;

/// Global LAPIC base (set during init, read-only after).
static mut APIC_BASE: u64 = 0;

// ── Low-level register access ────────────────────────────────────────

fn read_apic_base() -> u64 {
    let (lo, hi): (u32, u32);
    unsafe {
        asm!("rdmsr", in("ecx") IA32_APIC_BASE_MSR,
             out("eax") lo, out("edx") hi, options(nostack));
    }
    (((hi as u64) << 32) | (lo as u64)) & 0xFFFF_0000
}

#[inline]
pub unsafe fn apic_write(offset: u32, val: u32) {
    let ptr = (APIC_BASE + offset as u64) as *mut u32;
    core::ptr::write_volatile(ptr, val);
}

#[inline]
pub unsafe fn apic_read(offset: u32) -> u32 {
    let ptr = (APIC_BASE + offset as u64) as *const u32;
    core::ptr::read_volatile(ptr)
}

pub fn apic_eoi() {
    unsafe { apic_write(APIC_EOI, 0); }
}

// ── PIT-based delay ──────────────────────────────────────────────────

/// Wait ~10 ms using PIT channel 2. Used for APIC timer calibration.
/// Returns true on success, false if the PIT didn't respond.
fn pit_wait_10ms() -> bool {
    // PIT frequency = 1_193_182 Hz. 10 ms = 11_932 counts.
    const PIT_10MS: u16 = 11_932;
    const MAX_POLLS: u32 = 1_000_000;

    unsafe {
        // Channel 2, lobyte/hibyte, mode 0 (interrupt on terminal count).
        asm!("out 0x43, al", in("al") 0xB0u8, options(nostack));
        asm!("out 0x42, al", in("al") (PIT_10MS & 0xFF) as u8, options(nostack));
        asm!("out 0x42, al", in("al") (PIT_10MS >> 8) as u8, options(nostack));

        // Gate off, then gate on to start counting.
        let mut val: u8;
        asm!("in al, 0x61", out("al") val, options(nostack));
        asm!("out 0x61, al", in("al") val & !0x03, options(nostack)); // gate low
        asm!("out 0x61, al", in("al") (val & !0x03) | 0x01, options(nostack)); // gate high

        // Poll bit 5 of port 0x61 — set when PIT channel 2 reaches 0.
        for _ in 0..MAX_POLLS {
            asm!("in al, 0x61", out("al") val, options(nostack));
            if val & 0x20 != 0 { return true; }
        }
    }
    false
}

// ── Public init API ──────────────────────────────────────────────────

/// Initialize the Local APIC and start the periodic timer at `tick_hz`.
///
/// `tick_hz`: desired ticks per second (e.g. 100 for 10ms ticks).
///
/// The actual bus clock is unknown at boot, so we calibrate by counting
/// APIC timer ticks during a known PIT delay.
pub fn init_apic(tick_hz: u32) {
    unsafe {
        APIC_BASE = read_apic_base();
        crate::dev::console::serial_write("[apic] base=0x");
        crate::serial::hex(APIC_BASE);
        crate::dev::console::serial_write("\n");

        // 1. Enable LAPIC: set spurious vector to 0xFF + bit 8 (enable).
        let spurious = apic_read(APIC_SPURIOUS);
        apic_write(APIC_SPURIOUS, spurious | 0x1FF);

        // 2. Accept all interrupts (Task Priority = 0).
        apic_write(APIC_TPR, 0);

        // 3. Configure timer LVT in ONE-SHOT mode BEFORE calibration.
        //    The timer only counts down when LVT is non-masked. Without
        //    this, `elapsed` would always be 0xFFFF_FFFF and the
        //    calibration would be wrong.
        //    Bit 17 = periodic, bit 16 = masked, bits 0..8 = vector.
        //    One-shot: vector only, no bit 17.
        apic_write(APIC_TIMER_LVT, APIC_TIMER_VECTOR as u32);

        // 4. Set divide config to 16.
        apic_write(APIC_TIMER_DIV, 0x03);

        // 5. Calibrate: write max, wait 10ms via PIT, read elapsed.
        apic_write(APIC_TIMER_INIT, 0xFFFF_FFFF);

        let pit_ok = pit_wait_10ms();
        if !pit_ok {
            crate::dev::console::serial_write("[apic] WARN: PIT did not respond, using fallback tick=1000\n");
            apic_write(APIC_TIMER_INIT, 1_000_000); // arbitrary
            apic_write(APIC_TIMER_LVT, APIC_TIMER_VECTOR as u32 | (1 << 17));
            return;
        }

        let elapsed = 0xFFFF_FFFFu32.wrapping_sub(apic_read(APIC_TIMER_CUR));
        // elapsed = ticks del bus clock / 16 que pasaron en 10 ms.
        // ticks/sec = elapsed * 100.
        let ticks_per_sec = (elapsed as u64).saturating_mul(100);
        if ticks_per_sec == 0 {
            crate::dev::console::serial_write("[apic] WARN: elapsed=0, using fallback\n");
            apic_write(APIC_TIMER_INIT, 1_000_000);
            apic_write(APIC_TIMER_LVT, APIC_TIMER_VECTOR as u32 | (1 << 17));
            return;
        }

        // initial_count for desired freq.
        let initial_count = (ticks_per_sec / tick_hz as u64) as u32;
        let initial_count = if initial_count == 0 { 1 } else { initial_count };

        crate::dev::console::serial_write("[apic] ticks/sec=");
        crate::serial::u64_dec(ticks_per_sec);
        crate::dev::console::serial_write(" init=");
        crate::serial::u64_dec(initial_count as u64);
        crate::dev::console::serial_write(" @ ");
        crate::dev::console::serial_write_u64(tick_hz as u64, 10);
        crate::dev::console::serial_write(" Hz\n");

        // 6. Switch to PERIODIC mode and start the timer.
        apic_write(APIC_TIMER_LVT, APIC_TIMER_VECTOR as u32 | (1 << 17));
        apic_write(APIC_TIMER_DIV, 0x03);
        apic_write(APIC_TIMER_INIT, initial_count);
    }
}


