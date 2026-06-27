//! Thermal Monitoring (Ring 0 HAL).
//!
//! Monitors CPU temperature and takes action on overheat.
//! Uses AMD Zen3 thermal MSRs for temperature reading.
//!
//! AMD Zen3 thermal registers:
//!   - MSR_THERM_STATUS (0xC001029B): Current temperature, throttling
//!   - MSR_THERM_PROT (0xC001029B): Thermal protection config
//!   - MSR_F10H_THERMTRL: Thermal throttling control
//!
//! Temperature is read as the "current temperature" field in
//! THERM_STATUS. The trip point is configurable via BIOS/ACPI.

use core::sync::atomic::{AtomicBool, AtomicU32, Ordering};

/// Thermal MSR addresses (AMD Zen3).
const MSR_THERM_STATUS: u32 = 0xC001_029B;
const MSR_THERM_PROT: u32 = 0xC001_029B; // Same address, different field

/// Critical temperature threshold (Celsius). Ryzen 5600X max is 95°C.
const CRITICAL_TEMP_C: u32 = 90;

/// Warning temperature threshold.
const WARNING_TEMP_C: u32 = 80;

/// Current temperature (Celsius), updated periodically.
static CURRENT_TEMP: AtomicU32 = AtomicU32::new(0);

/// Overheat flag.
static OVERHEAT: AtomicBool = AtomicBool::new(false);

/// Thermal monitoring enabled.
static ENABLED: AtomicBool = AtomicBool::new(false);

/// Read the current CPU temperature from MSR.
///
/// AMD Zen3: MSR 0xC001029B bits [31:14] = temperature in Celsius.
fn read_temperature_msr() -> u32 {
    let (lo, hi): (u32, u32);
    unsafe {
        core::arch::asm!(
            "rdmsr",
            in("ecx") MSR_THERM_STATUS,
            out("eax") lo,
            out("edx") hi,
            options(nostack),
        );
    }
    let raw = ((hi as u64) << 32) | (lo as u64);
    // Temperature is bits [31:14], value / 8 = Celsius (for some AMD CPUs)
    // Or direct Celsius for others — simplified here
    ((raw >> 14) & 0x7FF) as u32
}

/// Initialize thermal monitoring.
pub fn init() {
    crate::dev::console::serial_write("[thermal] initializing\n");

    // Check if thermal MSRs are available (CPUID.06H: EAX[6] = thermal sensors)
    let (eax, _, _, _) = crate::cpu::cpuid(6, 0);
    if eax & (1 << 6) == 0 {
        crate::dev::console::serial_write("[thermal] no thermal sensors detected\n");
        return;
    }

    // Read initial temperature
    let temp = read_temperature_msr();
    CURRENT_TEMP.store(temp, Ordering::Relaxed);

    if temp >= CRITICAL_TEMP_C {
        OVERHEAT.store(true, Ordering::Relaxed);
        crate::dev::console::serial_write("[thermal] WARNING: initial temp=");
        crate::dev::console::serial_write_u64(temp as u64, 10);
        crate::dev::console::serial_write("C (CRITICAL)\n");
    } else {
        crate::dev::console::serial_write("[thermal] initial temp=");
        crate::dev::console::serial_write_u64(temp as u64, 10);
        crate::dev::console::serial_write("C\n");
    }

    ENABLED.store(true, Ordering::Relaxed);
}

/// Periodic thermal check. Called from timer tick.
pub fn check() {
    if !ENABLED.load(Ordering::Relaxed) { return; }

    let temp = read_temperature_msr();
    CURRENT_TEMP.store(temp, Ordering::Relaxed);

    if temp >= CRITICAL_TEMP_C {
        if !OVERHEAT.load(Ordering::Relaxed) {
            OVERHEAT.store(true, Ordering::Relaxed);
            crate::dev::console::serial_write("[thermal] OVERHEAT: ");
            crate::dev::console::serial_write_u64(temp as u64, 10);
            crate::dev::console::serial_write("C\n");
        }
        // TODO: Trigger thermal interrupt, reduce CPU frequency
    } else if temp < WARNING_TEMP_C {
        OVERHEAT.store(false, Ordering::Relaxed);
    }
}

/// Get current temperature in Celsius.
pub fn temperature() -> u32 {
    CURRENT_TEMP.load(Ordering::Relaxed)
}

/// Check if CPU is overheating.
pub fn is_overheating() -> bool {
    OVERHEAT.load(Ordering::Relaxed)
}
