//! Modular CPU initialization — single entry point for all CPU subsystems.
//!
//! Usage: `let info = crate::ring0::cpu::init();`
//!
//! This orchestrates: feature detection → CR0/CR4 → XCR0 → FPU → MTRR → PAT
//! → TSC calibration → info display.

pub mod features;
pub mod msr;
pub mod regs;
pub mod cache;
pub mod fpu;
pub mod tsc;
pub mod info;
pub mod vendor_shim;

pub use features::CpuFeatures;
pub use vendor_shim as vendor;

/// Complete CPU information after initialization.
pub struct CpuInfo {
    pub features: CpuFeatures,
    pub tsc_freq: u64,
}

/// Initialize all CPU subsystems in the correct order.
pub fn init() -> CpuInfo {
    let features = features::detect();
    regs::init(&features);
    regs::init_xcr0(&features);
    fpu::init_fpu();
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
    let tsc_freq = tsc::calibrate();
    info::print();
    CpuInfo { features, tsc_freq }
}

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

#[inline]
pub fn rdtsc() -> u64 {
    let low: u32;
    let high: u32;
    unsafe { core::arch::asm!("rdtsc", out("eax") low, out("edx") high); }
    ((high as u64) << 32) | low as u64
}

static mut TSC_FREQ_HZ: u64 = 0;

pub fn set_tsc_freq(freq: u64) {
    unsafe { TSC_FREQ_HZ = freq; }
}

#[inline]
pub fn tsc_per_sec() -> u64 {
    unsafe { TSC_FREQ_HZ }
}

#[inline]
pub fn rdtscp() -> (u64, u32) {
    let low: u32;
    let high: u32;
    let aux: u32;
    unsafe { core::arch::asm!("rdtscp", out("eax") low, out("edx") high, out("ecx") aux); }
    (((high as u64) << 32) | low as u64, aux)
}

#[inline]
pub fn halt() {
    unsafe { core::arch::asm!("sti; hlt"); }
}

#[inline]
pub fn sti() {
    unsafe { core::arch::asm!("sti"); }
}

#[inline]
pub fn cli() {
    unsafe { core::arch::asm!("cli"); }
}

#[inline]
pub fn irqs_enabled() -> bool {
    let flags: u64;
    unsafe { core::arch::asm!("pushfq; pop {}", out(reg) flags, options(nostack)); }
    flags & 0x200 != 0
}

pub fn busy_wait_ms(ms: u64) {
    let freq = tsc_per_sec();
    if freq > 0 {
        tsc::busy_wait_ms(ms, freq);
    } else {
        let start = rdtsc();
        let target = 3_700_000u64 * ms;
        while rdtsc().wrapping_sub(start) < target {
            core::hint::spin_loop();
        }
    }
}
