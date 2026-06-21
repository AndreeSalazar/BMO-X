#![allow(dead_code)]

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
pub mod delay;
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
    crate::dev::console::serial_write("[cpu] === Modular CPU Init ===\n");

    // 1. Detect features via CPUID
    crate::dev::console::serial_write("[cpu] step 1: features::detect\n");
    let features = features::detect();

    // 2. Configure CR0/CR4 (FPU, SSE, AVX, SMEP, SMAP, etc.)
    crate::dev::console::serial_write("[cpu] step 2: regs::init\n");
    regs::init(&features);

    // 3. Configure XCR0 safely (x87 + SSE + AVX state management)
    crate::dev::console::serial_write("[cpu] step 3: regs::init_xcr0\n");
    regs::init_xcr0(&features);

    // 4. Initialize FPU clean state
    crate::dev::console::serial_write("[cpu] step 4: fpu::init_fpu\n");
    crate::cpu::fpu::init_fpu();

    // 5. Configure MTRRs (default WB, VRAM as WC) + PAT
    crate::dev::console::serial_write("[cpu] step 5: cache::init\n");
    // Pass framebuffer info so MTRR marks it as Write-Combining (WC).
    // Without WC, scattered writes (like font glyph pixels) stay in
    // CPU cache and are invisible to the display controller.
    let (fb_addr, fb_size) = unsafe {
        let addr = crate::boot::info::FB_ADDR;
        let size = if addr != 0 {
            crate::boot::info::FB_WIDTH as u64
                * crate::boot::info::FB_HEIGHT as u64
                * 4
        } else {
            0
        };
        (addr, size)
    };
    if fb_addr != 0 && fb_size > 0 {
        crate::dev::console::serial_write("[cpu]   framebuffer WC: base=0x");
        crate::dev::console::serial_write_u64(fb_addr, 16);
        crate::dev::console::serial_write(" size=");
        crate::dev::console::serial_write_u64(fb_size, 10);
        crate::dev::console::serial_write("\n");
    }
    cache::init(&features, fb_addr, fb_size);

    // 6. Enable performance counters
    crate::dev::console::serial_write("[cpu] step 6: perf::init\n");
    perf::init(&features);

    // 7. Enable lazy FPU switching (CR0.TS)
    crate::dev::console::serial_write("[cpu] step 7: enable_lazy_fpu\n");
    crate::cpu::fpu::enable_lazy_fpu();

    // 8. Calibrate TSC frequency
    crate::dev::console::serial_write("[cpu] step 8: tsc::calibrate\n");
    let tsc_freq = tsc::calibrate();

    // 9. Print CPU info
    crate::dev::console::serial_write("[cpu] step 9: info::print\n");
    info::print();

    crate::dev::console::serial_write("[cpu] === Init Complete ===\n");

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

/// Read performance monitoring counter.
#[inline]
pub unsafe fn rdpmc(counter: u32) -> u64 {
    let low: u32;
    let high: u32;
    core::arch::asm!("rdpmc", in("ecx") counter, out("eax") low, out("edx") high);
    ((high as u64) << 32) | low as u64
}

/// Flush cache line.
#[inline]
pub fn clflush(addr: u64) {
    unsafe { core::arch::asm!("clflush [{}]", in(reg) addr, options(nostack)); }
}

/// Flush cache line optimized (CLFLUSHOPT).
#[inline]
pub fn clflushopt(addr: u64) {
    unsafe { core::arch::asm!("clflushopt [{}]", in(reg) addr, options(nostack)); }
}

/// Serialize instruction stream.
#[inline]
pub fn lfence() {
    unsafe { core::arch::asm!("lfence"); }
}

/// Memory fence.
#[inline]
pub fn mfence() {
    unsafe { core::arch::asm!("mfence"); }
}

/// CPUID with explicit ECX subleaf (alias for x2APIC enumeration).
#[inline]
pub fn cpuid_x2(leaf: u32, subleaf: u32) -> (u32, u32, u32, u32) {
    cpuid(leaf, subleaf)
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

