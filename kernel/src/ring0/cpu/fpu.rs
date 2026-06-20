#![allow(dead_code)]

//! FPU/SSE/AVX context save/restore — XSAVE/XRSTOR for Ryzen 5 5600X.
//!
//! XSAVE area layout (Intel SDM Vol 1, Ch 13):
//!   Bytes 0-511:   Legacy region (FXSAVE format)
//!     0-7:    FCW (x87 control word)
//!     8-9:    FSW (x87 status word)
//!     10:     FTW (x87 tag word)
//!     11:     Reserved
//!     12-13:  FOP (x87 opcode)
//!     14-17:  FIP (x87 instruction pointer offset)
//!     18-19:  FCS (x87 instruction pointer selector)
//!     20-21:  Reserved
//!     22-23:  FDP (x87 data pointer offset)
//!     24-25:  FDS (x87 data pointer selector)
//!     26-27:  Reserved
//!     28-31:  MXCSR
//!     32-33:  MXCSR_MASK
//!     34-511: ST0-MM7 (128 bytes each) + XMM0-XMM15 (128 bytes each)
//!   Bytes 512+:    Extended state (AVX, MPX, etc.)
//!     512-575: XMM0-XMM3 (YMM low halves)
//!     576-639: XMM4-XMM7
//!     640-703: XMM8-XMM11
//!     704-767: XMM12-XMM15
//!     768-831: XMM_Hi256 (YMM upper halves for YMM0-YMM3)
//!     832-895: XMM_Hi256 (YMM upper halves for YMM4-YMM7)
//!
//! Total XSAVE area for x87+SSE+AVX: 832 bytes (rounded to 64-byte boundary = 896).
//! We allocate 1024 bytes per task for safety.

use core::arch::{asm, x86_64::{_xgetbv, _xsetbv}};

/// XSAVE area size per task (1024 bytes for x87+SSE+AVX on Zen 3).
pub const XSAVE_AREA_SIZE: usize = 1024;

/// XSAVE header starts at byte 512 of the XSAVE area.
/// XCOMP_BV at offset 8 in the header indicates which components are in compacted format.
const XSAVE_HEADER_OFFSET: usize = 512;
const XSAVE_HEADER_SIZE: usize = 64;

/// XCR0 bits we support.
const XCR0_X87: u64 = 1 << 0;
const XCR0_SSE: u64 = 1 << 1;
const XCR0_AVX: u64 = 1 << 2;

/// Get the current XCR0 value.
#[inline]
pub fn xgetbv() -> u64 {
    unsafe { _xgetbv(0) }
}

/// Set XCR0 value.
///
/// # Safety
/// Caller must ensure the XCR0 value is valid for the current CPU.
#[inline]
pub unsafe fn xsetbv(val: u64) {
    let low = (val & 0xFFFFFFFF) as u32;
    let high = (val >> 32) as u32;
    _xsetbv(0, low as u64 | ((high as u64) << 32));
}

/// Save x87+SSE+AVX state using FXSAVE (512 bytes, no XSAVE header).
///
/// # Safety
/// - The `area` pointer must be 16-byte aligned and at least 512 bytes.
/// - SSE/AVX must be enabled in CR4.OSFXSR and CR4.OSXSAVE.
pub unsafe fn fxsave(area: *mut u8) {
    asm!(
        "fxsave [{}]",
        in(reg) area,
        options(nostack),
    );
}

/// Restore x87+SSE state using FXRSTOR (512 bytes).
///
/// # Safety
/// - The `area` pointer must be 16-byte aligned and contain valid FXSAVE data.
/// - SSE must be enabled in CR4.OSFXSR.
pub unsafe fn fxrstor(area: *const u8) {
    asm!(
        "fxrstor [{}]",
        in(reg) area,
        options(nostack),
    );
}

/// Save full x87+SSE+AVX state using XSAVE (up to 896+ bytes).
///
/// Saves components enabled in XCR0 AND EDX:EAX mask.
///
/// # Safety
/// - The `area` pointer must be 64-byte aligned and at least `xsave_area_size()` bytes.
/// - XSAVE must be enabled in CR4.OSXSAVE.
pub unsafe fn xsave(area: *mut u8) {
    // EDX:EAX = mask of components to save (all enabled in XCR0)
    let mask_lo: u32 = 0x7; // x87 | SSE | AVX
    let mask_hi: u32 = 0;
    asm!(
        "xsave [{}]",
        in(reg) area,
        in("eax") mask_lo,
        in("edx") mask_hi,
        options(nostack),
    );
}

/// Save full x87+SSE+AVX state using XSAVEOPT (optimized, skips clean components).
///
/// # Safety
/// - Same requirements as `xsave`.
/// - CPU must support XSAVEOPT (CPUID.07H:ECX.XSAVEOPT[bit 27]).
pub unsafe fn xsaveopt(area: *mut u8) {
    let mask_lo: u32 = 0x7;
    let mask_hi: u32 = 0;
    asm!(
        "xsaveopt [{}]",
        in(reg) area,
        in("eax") mask_lo,
        in("edx") mask_hi,
        options(nostack),
    );
}

/// Restore full x87+SSE+AVX state using XRSTOR.
///
/// # Safety
/// - The `area` pointer must be 64-byte aligned and contain valid XSAVE data.
/// - XSAVE must be enabled in CR4.OSXSAVE.
pub unsafe fn xrstor(area: *const u8) {
    let mask_lo: u32 = 0x7;
    let mask_hi: u32 = 0;
    asm!(
        "xrstor [{}]",
        in(reg) area,
        in("eax") mask_lo,
        in("edx") mask_hi,
        options(nostack),
    );
}

/// Get XSAVE area size for components enabled in XCR0.
///
/// # Safety
/// - XSAVE must be enabled in CR4.OSXSAVE.
pub unsafe fn xsave_area_size() -> u32 {
    let result: u32;
    core::arch::asm!(
        "push rbx",
        "cpuid",
        "mov {res:e}, ebx",
        "pop rbx",
        in("eax") 0x0Du32,
        in("ecx") 0u32,
        res = out(reg) result,
        out("edx") _,
    );
    result
}

pub unsafe fn xsave_component_info(comp: u32) -> (u32, u32) {
    let offset: u32;
    let size: u32;
    core::arch::asm!(
        "push rbx",
        "cpuid",
        "mov {off:e}, eax",
        "mov {sz:e}, ebx",
        "pop rbx",
        in("eax") 0x0Du32,
        in("ecx") comp,
        off = out(reg) offset,
        sz = out(reg) size,
        out("edx") _,
    );
    (offset & 0x7FFFFFFF, size)
}

/// Enable CR0.TS (Task Switched) for lazy FPU context switching.
///
/// When TS is set, any FPU/SSE/AVX instruction causes #NM (vector 7).
/// The ISR can then save the previous task's FPU state and restore the current task's.
#[inline]
pub fn enable_lazy_fpu() {
    unsafe {
        let mut cr0: u64;
        asm!("mov {}, cr0", out(reg) cr0);
        cr0 |= 1 << 3; // Set TS
        asm!("mov cr0, {}", in(reg) cr0);
    }
}

/// Clear CR0.TS — allow FPU/SSE/AVX instructions.
#[inline]
pub fn clear_lazy_fpu() {
    unsafe {
        let mut cr0: u64;
        asm!("mov {}, cr0", out(reg) cr0);
        cr0 &= !(1 << 3); // Clear TS
        asm!("mov cr0, {}", in(reg) cr0);
    }
}

/// Check if CR0.TS is set (FPU was not used by current task yet).
#[inline]
pub fn ts_is_set() -> bool {
    unsafe {
        let cr0: u64;
        asm!("mov {}, cr0", out(reg) cr0);
        cr0 & (1 << 3) != 0
    }
}

/// FXSAVE area size (always 512 bytes, 16-byte aligned).
pub const FXSAVE_AREA_SIZE: usize = 512;

/// Align a value up to the given alignment.
#[inline]
pub const fn align_up(val: usize, align: usize) -> usize {
    (val + align - 1) & !(align - 1)
}

/// Initialize FPU/SSE/AVX for the boot CPU.
///
/// Must be called after CR0/CR4/XCR0 are configured.
/// Sets up the initial FPU state with:
/// - x87 FPU: round to nearest, double precision
/// - MXCSR: default value (exceptions masked)
pub fn init_fpu() {
    unsafe {
        // Initialize x87 FPU state
        asm!(
            "fninit",
            options(nostack),
        );

        // Set MXCSR to default: all exceptions masked, round to nearest
        let mxcsr: u32 = 0x1F80; // Default MXCSR value
        asm!(
            "ldmxcsr [{addr}]",
            addr = in(reg) &mxcsr as *const u32,
            options(nostack),
        );

        crate::device::serial::serial_write("[FPU] x87 FPU + MXCSR initialized\n");
    }
}

/// Save FPU/SSE/AVX context for a task.
///
/// Uses XSAVE if available, falls back to FXSAVE.
/// Returns the number of bytes written.
pub unsafe fn save_context(area: *mut u8, use_xsave: bool, use_xsaveopt: bool) -> usize {
    if use_xsave {
        if use_xsaveopt {
            xsaveopt(area);
        } else {
            xsave(area);
        }
        xsave_area_size() as usize
    } else {
        fxsave(area);
        FXSAVE_AREA_SIZE
    }
}

/// Restore FPU/SSE/AVX context for a task.
///
/// Uses XRSTOR if available, falls back to FXRSTOR.
pub unsafe fn restore_context(area: *const u8, use_xsave: bool) {
    if use_xsave {
        xrstor(area);
    } else {
        fxrstor(area);
    }
}
