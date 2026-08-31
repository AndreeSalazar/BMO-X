//! TSC calibration for the Ryzen 5 5600X.
//!
//! [carril]  AMARILLO  calibra el TSC, y TODO lo que mide tiempo cuelga de el
//!
//! Recovers the legacy `tsc_calibration.rs` from the deleted
//! `crates_Personal/ring0/cpu_vendor_profile/.../tsc_calibration.rs`,
//! simplified: we use CPUID 0x15 (Core Crystal Clock) if available
//! and fall back to the known 5600X base frequency (3,700,000,000 Hz).
//!
//! References:
//! - AMD64 APM Vol. 2, section 13.1 (TSC)
//! - AMD Zen 3 Family 19h BKDG (Crystal Clock / PState accounting)

use super::cpuid::cpuid;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TscSource { Cpuid15, AcpiPmTimer, Hardcoded }

impl TscSource {
    pub fn name(&self) -> &'static str {
        match self {
            Self::Cpuid15 => "CPUID 0x15",
            Self::AcpiPmTimer => "ACPI PM Timer",
            Self::Hardcoded => "hardcoded 5600X constant",
        }
    }
}

const FALLBACK_HZ: u64 = 3_700_000_000; // 5600X base frequency

/// Try CPUID 0x15 first; fall back to a hardcoded value.
/// Returns (freq_hz, source).
pub fn calibrate() -> (u64, TscSource) {
    let (eax, ebx, ecx, _) = cpuid(0x15, 0);
    if eax != 0 && ecx != 0 {
        // TSC freq = ecx * ebx / eax (Core Crystal Clock * max_ratio / core_ratio)
        let denom = eax as u64;
        if denom != 0 {
            return (ecx as u64 * ebx as u64 / denom, TscSource::Cpuid15);
        }
    }
    (FALLBACK_HZ, TscSource::Hardcoded)
}
