
//! Modular CPU initialization — single entry point for all CPU subsystems.
//!
//! Usage: `let info = crate::cpu::init();`
//!
//! This orchestrates: feature detection → CR0/CR4 → XCR0 → FPU → MTRR → PAT
//! → performance counters → TSC calibration → info display.

pub mod features;
pub mod msr;
pub mod regs;
pub mod cache;
pub mod fpu;
pub mod perf;
pub mod tsc;
pub mod info;

// Re-export key types for convenience
pub use features::CpuFeatures;

/// Complete CPU information after initialization.
pub struct CpuInfo {
    pub features: CpuFeatures,
    pub tsc_freq: u64,
}

/// Initialize all CPU subsystems in the correct order.
///
/// This is the single entry point for CPU initialization:
/// ```ignore
/// let cpu = crate::cpu::init();
/// ```
pub fn init() -> CpuInfo {
    crate::boot_phase::write_crash_marker(2031);
    crate::uefi_rt::write_boot_stage("cpu_step1_feat");

    // 1. Detect features via CPUID
    let features = features::detect();

    // 2. Configure CR0/CR4 (FPU, SSE, AVX, SMEP, SMAP, etc.)
    crate::boot_phase::write_crash_marker(2032);
    crate::uefi_rt::write_boot_stage("cpu_step2_regs");
    regs::init(&features);

    // 3. Configure XCR0 safely (x87 + SSE + AVX state management)
    crate::boot_phase::write_crash_marker(2033);
    crate::uefi_rt::write_boot_stage("cpu_step3_xcr0");
    regs::init_xcr0(&features);

    // 4. Initialize FPU clean state
    crate::boot_phase::write_crash_marker(2034);
    crate::uefi_rt::write_boot_stage("cpu_step4_fpu");
    crate::cpu::fpu::init_fpu();

    // 5. Configure MTRRs (default WB, VRAM as WC) + PAT
    crate::boot_phase::write_crash_marker(2035);
    crate::uefi_rt::write_boot_stage("cpu_step5_cache");
    // Pass framebuffer info so MTRR marks it as Write-Combining (WC).
    // Without WC, scattered writes (like font glyph pixels) stay in
    // CPU cache and are invisible to the display controller.
    //
    // SAFETY: We read static mut globals that were written by the
    // bootloader and stored in boot::info during store_boot_info().
    // This runs single-threaded during early boot.
    let (fb_addr, fb_size) = unsafe {
        let addr = crate::info::FB_ADDR;
        let size = if addr != 0 {
            crate::info::FB_WIDTH as u64
                * crate::info::FB_HEIGHT as u64
                * 4
        } else {
            0
        };
        (addr, size)
    };
    cache::init(&features, fb_addr, fb_size);

    // 6. Performance counters — SKIPPED on AMD Zen 3
    // MSR 0x38D/0x38F (IA32_FIXED_CTR_CTRL/IA32_PERF_GLOBAL_CTRL) are
    // Intel-architecture MSRs. On AMD they may not exist or have different
    // reserved bits, causing #GP → red screen crash. Fixed counters are
    // not essential for boot. Enable when AMD perfmon driver is mature.
    // (See kernel/src/ring0/cpu/perf.rs for the AMD-adapted version.)
    crate::boot_phase::write_crash_marker(2036);
    crate::uefi_rt::write_boot_stage("cpu_step6_perf");
    // perf::init(&features);  // DISABLED — causes #GP on Ryzen 5600X

    // 7. Enable lazy FPU switching (CR0.TS) - DISABLED
    crate::boot_phase::write_crash_marker(2037);
    crate::uefi_rt::write_boot_stage("cpu_step7_fpu");

    // 8. Calibrate TSC frequency
    crate::boot_phase::write_crash_marker(2038);
    crate::uefi_rt::write_boot_stage("cpu_step8_tsc");
    let tsc_freq = tsc::calibrate();

    // 9. Print CPU info
    crate::boot_phase::write_crash_marker(2039);
    crate::uefi_rt::write_boot_stage("cpu_step9_info");
    info::print();

    crate::boot_phase::write_crash_marker(2040);
    crate::uefi_rt::write_boot_stage("cpu_init_done");

    CpuInfo { features, tsc_freq }
}

// ── Low-level helpers (used by other arch modules) ─────────────────

/// CPUID with leaf/subleaf.
#[inline]
pub fn cpuid(leaf: u32, sub: u32) -> (u32, u32, u32, u32) {
    let (eax, ebx, ecx, edx);
    unsafe {
        core::arch::asm!(
            "push rbx",
            "cpuid",
            "mov {ebx_out:e}, ebx",
            "pop rbx",
            inout("eax") leaf => eax,
            inout("ecx") sub => ecx,
            ebx_out = out(reg) ebx,
            out("edx") edx,
        );
    }
    (eax, ebx, ecx, edx)
}

/// Read TSC (no serialization).
#[inline]
pub fn rdtsc() -> u64 {
    let low: u32;
    let high: u32;
    unsafe { core::arch::asm!("rdtsc", out("eax") low, out("edx") high); }
    ((high as u64) << 32) | low as u64
}

/// Global TSC frequency in Hz (set by `tsc::calibrate()`).
/// Used by watchdog, sleep helpers, and bmo_abi::time::Instant.
static mut TSC_FREQ_HZ: u64 = 0;

/// Set the global TSC frequency (called from `tsc::calibrate`).
pub fn set_tsc_freq(freq: u64) {
    unsafe { TSC_FREQ_HZ = freq; }
}

/// Get the global TSC frequency in Hz.
/// Returns 0 if TSC hasn't been calibrated yet.
#[inline]
pub fn tsc_per_sec() -> u64 {
    unsafe { TSC_FREQ_HZ }
}

/// Read TSC with processor ID.
#[inline]
pub fn rdtscp() -> (u64, u32) {
    let low: u32;
    let high: u32;
    let aux: u32;
    unsafe { core::arch::asm!("rdtscp", out("eax") low, out("edx") high, out("ecx") aux); }
    (((high as u64) << 32) | low as u64, aux)
}

/// Halt the CPU until the next interrupt.
#[inline]
pub fn halt() {
    unsafe { core::arch::asm!("sti; hlt"); }
}

/// Enable interrupts (STI).
#[inline]
pub fn sti() {
    unsafe { core::arch::asm!("sti"); }
}

/// Disable interrupts (CLI).
#[inline]
pub fn cli() {
    unsafe { core::arch::asm!("cli"); }
}

/// Check if interrupts are enabled (RFLAGS.IF = 1).
#[inline]
pub fn irqs_enabled() -> bool {
    let flags: u64;
    unsafe { core::arch::asm!("pushfq; pop {}", out(reg) flags, options(nostack)); }
    flags & 0x200 != 0
}

/// Busy-wait for approximately `ms` milliseconds using a simple TSC loop.
pub fn busy_wait_ms(ms: u64) {
    let freq = tsc_per_sec();
    if freq > 0 {
        tsc::busy_wait_ms(ms, freq);
    } else {
        // Fallback: ~3.7 GHz for Ryzen 5 5600X
        let start = rdtsc();
        let target = 3_700_000u64 * ms;
        while rdtsc().wrapping_sub(start) < target {
            core::hint::spin_loop();
        }
    }
}

