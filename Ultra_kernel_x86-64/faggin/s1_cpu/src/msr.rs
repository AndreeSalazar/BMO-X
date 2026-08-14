//! **MODEL-SPECIFIC REGISTERS AND `CPUID`** -- how the CPU is asked and told.
//!
//! === Why this is a file of its own ===
//!
//! It is the vocabulary every other module here speaks, and it contains no
//! policy: `rdmsr`/`wrmsr` move a 64-bit value in and out, `cpuid` asks a leaf.
//! Which registers to write, and what to conclude from the answers, belongs to
//! the CPU profile -- see `cpu/`.
//!
//! [!] `cpuid` preserves `RBX`. It is callee-saved in SysV and the instruction
//! clobbers it, so a wrapper that forgets returns correct numbers and corrupts
//! its caller -- a failure that appears far from its cause.

#[allow(unused_imports)]
use crate::*;

// ===================================================================
//  AMD-SPECIFIC MSR ADDRESSES (Zen 3 / Family 19h)
// ===================================================================

pub const MSR_TSC: u32                = 0x00000010;
pub const MSR_APIC_BASE: u32         = 0x0000001B;
pub const MSR_PLATFORM_INFO: u32     = 0x00000017;
pub const MSR_MTRR_CAP: u32          = 0x000000FE;
pub const MSR_PAT: u32               = 0x00000277;
pub const MSR_MTRR_FIX_64K_00000: u32 = 0x00000250;
pub const MSR_MTRR_VARIABLE_BASE: u32 = 0x00000200;
pub const MSR_MTRR_VARIABLE_MASK: u32 = 0x00000201;
pub const MSR_MTRR_DEF_TYPE: u32     = 0x000002FF;
pub const MSR_SYSENTER_CS: u32       = 0x00000174;
pub const MSR_SYSENTER_ESP: u32      = 0x00000175;
pub const MSR_SYSENTER_EIP: u32      = 0x00000176;
pub const MSR_TSC_AUX: u32           = 0xC0000103;

// AMD K8 SYSCALL MSRs (0xC0000080-0xC0000084)
pub const MSR_EFER: u32              = 0xC0000080;
pub const MSR_STAR: u32              = 0xC0000081;
pub const MSR_LSTAR: u32             = 0xC0000082;
pub const MSR_CSTAR: u32             = 0xC0000083;
pub const MSR_SFMASK: u32            = 0xC0000084;

// AMD segment base MSRs (0xC0000100-0xC0000102)
pub const MSR_FS_BASE: u32           = 0xC0000100;
pub const MSR_GS_BASE: u32           = 0xC0000101;
pub const MSR_KERNEL_GS_BASE: u32    = 0xC0000102;

// AMD-specific MSRs (Zen 3)
pub const MSR_SYSCFG: u32            = 0xC0000010;  // Zen 3 SYSCFG
pub const MSR_HWCR: u32              = 0xC0010015;  // Hardware Configuration
pub const MSR_NB_CFG1: u32           = 0xC001001E;  // Northbridge Config 1
pub const MSR_LS_CFG: u32            = 0xC0011020;  // Load-Store Configuration
pub const MSR_IC_CFG: u32            = 0xC0011021;  // Instruction Cache Configuration
pub const MSR_DC_CFG: u32            = 0xC0011022;  // Data Cache Configuration
pub const MSR_BU_CFG: u32            = 0xC0011023;  // Bus Unit Configuration
pub const MSR_DE_CFG: u32            = 0xC0011029;  // Decode Unit Configuration
pub const MSR_L2_CFG: u32            = 0xC001102D;  // L2 Cache Configuration
pub const MSR_CU_CFG: u32            = 0xC001102F;  // Compute Unit Configuration
pub const MSR_PF2_INSTR_CTL: u32     = 0xC0010100;  // Prefetch Configuration
pub const MSR_PF1_INSTR_CTL: u32     = 0xC0010102;

// EFER bits (AMD-specific bits marked)
pub const EFER_SCE: u64   = 1 << 0;   // SYSCALL enable
pub const EFER_LME: u64   = 1 << 8;   // Long mode enable
pub const EFER_LMA: u64   = 1 << 10;  // Long mode active
pub const EFER_NXE: u64   = 1 << 11;  // No-execute enable
pub const EFER_SVME: u64  = 1 << 12;  // Secure virtual machine (SVM) enable
pub const EFER_LMSLE: u64 = 1 << 13;  // Long mode segment limit enable
pub const EFER_FFXSR: u64 = 1 << 14;  // Fast FXSAVE/XRESTOR
pub const EFER_TCE: u64   = 1 << 15;  // Translation cache extension
pub const EFER_MCOMMIT: u64 = 1 << 17; // MCOMMIT instruction enable
pub const EFER_INTWB: u64 = 1 << 18;  // Interruptible WBINVD/WBNOINVD
pub const EFER_UAIE: u64  = 1 << 19;  // Upper address ignore enable (SEV)
pub const EFER_AIBRSE: u64 = 1 << 21; // Automatic IBRS enable

// SYSCFG bits (AMD Zen 3)
pub const SYSCFG_MFDM: u64   = 1 << 18;  // Memory disambiguation flush disable
pub const SYSCFG_TOM2: u64   = 1 << 21;  // TOM2 enable
pub const SYSCFG_FB_MODE: u64 = 1 << 25; // FSGSBASE enable (Zen 3)
pub const SYSCFG_FSGS: u64   = 1 << 18;  // FSGS (deprecated)

// HWCR bits (AMD)
pub const HWCR_FFDIS: u64    = 1 << 6;  // Flush filter disable


// ===================================================================
//  CPUID WRAPPER (preserves RBX per SysV ABI)
// ===================================================================

#[inline]
pub fn cpuid(leaf: u32, sub: u32) -> (u32, u32, u32, u32) {
    let (eax, ebx, ecx, edx);
    unsafe {
        asm!(
            "push rbx", "cpuid", "mov {ebx_out:e}, ebx", "pop rbx",
            inout("eax") leaf => eax, inout("ecx") sub => ecx,
            ebx_out = out(reg) ebx, out("edx") edx,
        );
    }
    (eax, ebx, ecx, edx)
}

#[inline]
pub unsafe fn wrmsr(msr: u32, val: u64) {
    asm!("wrmsr", in("ecx") msr, in("eax") val as u32, in("edx") (val >> 32) as u32);
}

#[inline]
pub unsafe fn rdmsr(msr: u32) -> u64 {
    let lo: u32; let hi: u32;
    asm!("rdmsr", in("ecx") msr, out("eax") lo, out("edx") hi);
    ((hi as u64) << 32) | (lo as u64)
}
