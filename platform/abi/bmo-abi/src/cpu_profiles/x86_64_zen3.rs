//! BMO v1 CPU contract: AMD Zen 3 on x86-64.
//!
//! This is a binary compatibility contract, not a promise that every
//! application uses every instruction. A compiler may tune with
//! `target-cpu=znver3`; a BEF that relies on these instructions declares this
//! profile and the BMO loader validates it with CPUID before execution.

/// Stable CPU feature bits stored by a BEF manifest.
#[repr(transparent)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CpuFeatureSet(pub u64);

impl CpuFeatureSet {
    pub const SSE4_2: Self = Self(1 << 0);
    pub const AVX: Self = Self(1 << 1);
    pub const AVX2: Self = Self(1 << 2);
    pub const FMA: Self = Self(1 << 3);
    pub const BMI1: Self = Self(1 << 4);
    pub const BMI2: Self = Self(1 << 5);
    pub const AES: Self = Self(1 << 6);
    pub const PCLMULQDQ: Self = Self(1 << 7);
    pub const RDTSCP: Self = Self(1 << 8);
    pub const INVARIANT_TSC: Self = Self(1 << 9);

    pub const fn contains(self, required: Self) -> bool {
        (self.0 & required.0) == required.0
    }
}

impl core::ops::BitOr for CpuFeatureSet {
    type Output = Self;
    fn bitor(self, rhs: Self) -> Self::Output {
        Self(self.0 | rhs.0)
    }
}

/// CPU/machine profile selected by Cargo for a BMO build.
#[repr(u16)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CpuProfile {
    X86_64Zen3 = 0x0100,
    Ryzen5_5600X = 0x0101,
    EpycZen3 = 0x0102,
}

impl CpuProfile {
    pub const fn architecture(self) -> &'static str {
        "x86_64"
    }
    pub const fn required_features(self) -> CpuFeatureSet {
        X86_64_ZEN3
    }
    pub const fn target_cpu(self) -> &'static str {
        "znver3"
    }
    pub const fn pointer_width_bits(self) -> u8 {
        64
    }
    pub const fn little_endian(self) -> bool {
        true
    }
    pub const fn page_size(self) -> u32 {
        4096
    }
}

/// ISA baseline available to BMO v1 code generated for Zen 3.
pub const X86_64_ZEN3: CpuFeatureSet = CpuFeatureSet(
    CpuFeatureSet::SSE4_2.0
        | CpuFeatureSet::AVX.0
        | CpuFeatureSet::AVX2.0
        | CpuFeatureSet::FMA.0
        | CpuFeatureSet::BMI1.0
        | CpuFeatureSet::BMI2.0
        | CpuFeatureSet::AES.0
        | CpuFeatureSet::PCLMULQDQ.0
        | CpuFeatureSet::RDTSCP.0
        | CpuFeatureSet::INVARIANT_TSC.0,
);
