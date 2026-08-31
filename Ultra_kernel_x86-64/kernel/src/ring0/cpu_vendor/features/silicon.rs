//! `features::silicon` -- what the CPU DECLARES. Facts, never opinions.
//!
//! [carril]  VERDE     lo que el CPU DECLARA: hechos, nunca opiniones
//!
//! Every bit here comes from `CPUID` or from `CR4` on the machine that is
//! running. **Nothing is read from the profile**, and that is the whole point:
//! `profile.rs` says what we EXPECT, this says what there IS, and a census that
//! took its facts from the expectation would only ever agree with itself.
//!
//! Rule 5 of the project, and this file is its half: *hardcode CONTRACTS, ask
//! the hardware for FACTS.*
//!
//! # Where each bit lives
//!
//! Written down because a wrong bit number does not fail: it reports a feature
//! that is not there, or hides one that is. AMD64 APM Vol.3, appendix E.
//!
//! ```text
//!    CPUID.1.EDX          SSE2
//!    CPUID.1.ECX          SSE3..SSE4.2, AVX, FMA, F16C, POPCNT, MOVBE,
//!                         AES, PCLMULQDQ, XSAVE, OSXSAVE, RDRAND, MONITOR
//!    CPUID.7.0.EBX        BMI1/2, AVX2, SMEP, SMAP, ERMS, RDSEED, ADX,
//!                         CLFLUSHOPT, CLWB, SHA
//!    CPUID.7.0.ECX        UMIP
//!    CPUID.D.1.EAX        XSAVEOPT, XSAVEC, XSAVES
//!    CPUID.80000001.EDX   NX, paginas de 1 GiB, RDTSCP
//!    CPUID.80000001.ECX   LZCNT (ABM), MONITORX
//!    CPUID.80000007.EDX   TSC invariante
//!    CPUID.80000008.EBX   CLZERO
//! ```
//!
//! [!] `CPUID.D` is also read by `xsave.rs`, for a different question -- the
//! size of the save area. Two readers of one leaf asking two things is fine;
//! what would not be fine is one of them deriving its answer from the other.
//!
//! [!] `CR4.OSXSAVE` is read here directly rather than through `xsave.rs`,
//! whose `cr4()` is private. Two lines of inline assembly are not a contract,
//! and coupling two modules to avoid duplicating them would be the worse trade.

use super::Feat;
use crate::ring0::cpu_vendor::ryzen_5_5600x::cpuid::cpuid;

/// The raw words, read once. Kept as words and not as a pile of booleans so the
/// report can show them if a row ever looks wrong -- a feature that answers "no"
/// when the datasheet says yes is either the wrong bit or the wrong leaf, and
/// the only way to tell them apart is seeing the word.
#[derive(Debug, Clone, Copy, Default)]
pub struct Silicon {
    pub leaf1_ecx: u32,
    pub leaf1_edx: u32,
    pub leaf7_ebx: u32,
    pub leaf7_ecx: u32,
    pub leaf_d1_eax: u32,
    pub ext1_ecx: u32,
    pub ext1_edx: u32,
    pub ext7_edx: u32,
    pub ext8_ebx: u32,
    pub cr4: u64,
    /// Highest basic leaf the CPU answers. Below 7 the whole leaf-7 column is
    /// unanswerable, and reading it anyway returns whatever the last valid leaf
    /// left behind -- which would look like a plausible set of features.
    pub max_basic: u32,
    /// Highest extended leaf.
    pub max_ext: u32,
}

fn cr4() -> u64 {
    let v: u64;
    unsafe {
        core::arch::asm!("mov {}, cr4", out(reg) v, options(nomem, nostack, preserves_flags));
    }
    v
}

/// Read every leaf the census needs, once.
pub fn leer() -> Silicon {
    let mut s = Silicon::default();

    let (max_basic, _, _, _) = cpuid(0, 0);
    s.max_basic = max_basic;
    let (max_ext, _, _, _) = cpuid(0x8000_0000, 0);
    s.max_ext = max_ext;

    if max_basic >= 1 {
        let (_, _, ecx, edx) = cpuid(1, 0);
        s.leaf1_ecx = ecx;
        s.leaf1_edx = edx;
    }
    // ** Guarded, and not out of politeness: CPUID with a leaf above the
    // maximum returns the highest supported leaf's values instead of zeros.
    // Reading leaf 7 on a CPU that stops at 1 would hand back leaf 1's words
    // and every bit of the second column would be a plausible lie.
    if max_basic >= 7 {
        let (_, ebx, ecx, _) = cpuid(7, 0);
        s.leaf7_ebx = ebx;
        s.leaf7_ecx = ecx;
    }
    if max_basic >= 0xD {
        let (eax, _, _, _) = cpuid(0xD, 1);
        s.leaf_d1_eax = eax;
    }
    if max_ext >= 0x8000_0001 {
        let (_, _, ecx, edx) = cpuid(0x8000_0001, 0);
        s.ext1_ecx = ecx;
        s.ext1_edx = edx;
    }
    if max_ext >= 0x8000_0007 {
        let (_, _, _, edx) = cpuid(0x8000_0007, 0);
        s.ext7_edx = edx;
    }
    if max_ext >= 0x8000_0008 {
        let (_, ebx, _, _) = cpuid(0x8000_0008, 0);
        s.ext8_ebx = ebx;
    }

    s.cr4 = cr4();
    s
}

const fn bit(word: u32, n: u32) -> bool {
    word & (1u32 << n) != 0
}

/// Does this silicon declare `f`?
///
/// The `match` is exhaustive on purpose: adding a variant to [`Feat`] breaks
/// this file until somebody says where its bit lives. That is the only reason
/// the enum exists instead of a table of strings.
pub fn has(f: Feat, s: &Silicon) -> bool {
    match f {
        // -- CPUID.1.EDX --
        Feat::Sse2 => bit(s.leaf1_edx, 26),

        // -- CPUID.1.ECX --
        Feat::Sse41 => bit(s.leaf1_ecx, 19),
        Feat::Sse42 => bit(s.leaf1_ecx, 20),
        Feat::Fma => bit(s.leaf1_ecx, 12),
        Feat::Movbe => bit(s.leaf1_ecx, 22),
        Feat::Popcnt => bit(s.leaf1_ecx, 23),
        Feat::Aes => bit(s.leaf1_ecx, 25),
        Feat::Pclmul => bit(s.leaf1_ecx, 1),
        Feat::Xsave => bit(s.leaf1_ecx, 26),
        Feat::Avx => bit(s.leaf1_ecx, 28),
        Feat::F16c => bit(s.leaf1_ecx, 29),
        Feat::Rdrand => bit(s.leaf1_ecx, 30),
        Feat::Monitor => bit(s.leaf1_ecx, 3),

        // -- CPUID.7.0.EBX --
        Feat::Bmi1 => bit(s.leaf7_ebx, 3),
        Feat::Avx2 => bit(s.leaf7_ebx, 5),
        Feat::Smep => bit(s.leaf7_ebx, 7),
        Feat::Bmi2 => bit(s.leaf7_ebx, 8),
        Feat::Erms => bit(s.leaf7_ebx, 9),
        Feat::Rdseed => bit(s.leaf7_ebx, 18),
        Feat::Adx => bit(s.leaf7_ebx, 19),
        Feat::Smap => bit(s.leaf7_ebx, 20),
        Feat::Clflushopt => bit(s.leaf7_ebx, 23),
        Feat::Clwb => bit(s.leaf7_ebx, 24),
        Feat::Sha => bit(s.leaf7_ebx, 29),

        // -- CPUID.7.0.ECX --
        Feat::Umip => bit(s.leaf7_ecx, 2),

        // -- CPUID.D.1.EAX --
        Feat::Xsaveopt => bit(s.leaf_d1_eax, 0),
        Feat::Xsavec => bit(s.leaf_d1_eax, 1),
        Feat::Xsaves => bit(s.leaf_d1_eax, 3),

        // -- CPUID.80000001 --
        Feat::Lzcnt => bit(s.ext1_ecx, 5),
        Feat::Monitorx => bit(s.ext1_ecx, 29),
        Feat::Nx => bit(s.ext1_edx, 20),
        Feat::Pdpe1gb => bit(s.ext1_edx, 26),
        Feat::Rdtscp => bit(s.ext1_edx, 27),

        // -- CPUID.80000007 / 80000008 --
        Feat::InvariantTsc => bit(s.ext7_edx, 8),
        Feat::Clzero => bit(s.ext8_ebx, 0),

        // ** NOT a CPUID bit: a bit the SYSTEM has to set. It is in this census
        // because the syscall stub runs `xsave64` on every single door, and
        // `xsave64` is `#UD` unless this is on. See its row in `usage.rs`.
        Feat::Osxsave => s.cr4 & (1 << 18) != 0,
    }
}
