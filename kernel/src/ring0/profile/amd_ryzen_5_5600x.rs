//! `profile/amd_ryzen_5_5600x.rs` â€” Active hardware profile.
//!
//! v1.8.8: this is the ONLY hardware profile for BMO. It enables
//! the Ryzen 5 5600X (Zen 3) and prepares the build for future RDNA4
//! GPU support.
//!
//! When the build target changes (e.g. to a Ryzen 9000 + RDNA4 system),
//! this file is the single point of modification.

/// Hardware profile identifier string.
pub const HARDWARE_PROFILE: &str = "AMD-Ryzen-5-5600X";

// â”€â”€ CPU profile flags â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€

/// Whether the CPU has SSE support (always true on x86-64).
pub const ENABLE_SSE: bool = true;
/// Whether SSE2 is available (always true on x86-64).
pub const ENABLE_SSE2: bool = true;
/// Whether SSE3 is available.
pub const ENABLE_SSE3: bool = true;
/// Whether SSE4.1 is available.
pub const ENABLE_SSE41: bool = true;
/// Whether SSE4.2 is available.
pub const ENABLE_SSE42: bool = true;
/// Whether AVX is available.
pub const ENABLE_AVX: bool = true;
/// Whether AVX2 is available.
pub const ENABLE_AVX2: bool = true;
/// Whether FMA is available.
pub const ENABLE_FMA: bool = true;
/// Whether BMI1/BMI2 are available.
pub const ENABLE_BMI: bool = true;
/// Whether AES-NI is available.
pub const ENABLE_AES_NI: bool = true;
/// Whether SHA-NI is available.
pub const ENABLE_SHA_NI: bool = true;
/// Whether F16C is available.
pub const ENABLE_F16C: bool = true;
/// Whether POPCNT is available.
pub const ENABLE_POPCNT: bool = true;
/// Whether LZCNT is available.
pub const ENABLE_LZCNT: bool = true;
/// Whether RDRAND/RDSEED are available.
pub const ENABLE_RDRAND: bool = true;
/// Whether XSAVE is available.
pub const ENABLE_XSAVE: bool = true;
/// Whether FSGSBASE is available.
pub const ENABLE_FSGSBASE: bool = true;
/// Whether SMEP is available.
pub const ENABLE_SMEP: bool = true;
/// Whether SMAP is available.
pub const ENABLE_SMAP: bool = true;
/// Whether UMIP is available.
pub const ENABLE_UMIP: bool = true;
/// Whether RDTSCP is available.
pub const ENABLE_RDTSCP: bool = true;
/// Whether PCID is available.
pub const ENABLE_PCID: bool = true;
/// Whether INVPCID is available.
pub const ENABLE_INVPCID: bool = true;
/// Whether MTRR is available.
pub const ENABLE_MTRR: bool = true;
/// Whether PAT is available.
pub const ENABLE_PAT: bool = true;
/// Whether 1 GB huge pages are available.
pub const ENABLE_1GB_PAGES: bool = true;
/// Whether performance counters are available.
pub const ENABLE_PERFCTR: bool = true;
/// Whether AVX-512 is available (5600X: NO).
pub const ENABLE_AVX512: bool = false;
/// Whether 5-level paging (LA57) is available (5600X: NO).
pub const ENABLE_LA57: bool = false;

// â”€â”€ GPU profile flags â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€

/// Whether a discrete GPU is present (5600X: NO integrated GPU; would be
/// YES for a Ryzen 7000G APU like 8700G with Radeon 780M).
pub const ENABLE_INTEGRATED_GPU: bool = false;
/// Whether RDNA4 (RX 9060 XT) is the active discrete GPU driver.
/// v1.8.8: false because the driver skeleton is not ready yet.
pub const ENABLE_RDNA4_DRIVER: bool = false;
/// Whether RDNA3 (RX 7000 series) is the active discrete GPU driver.
pub const ENABLE_RDNA3_DRIVER: bool = false;
/// Whether RDNA2 (RX 6000 series) is the active discrete GPU driver.
pub const ENABLE_RDNA2_DRIVER: bool = false;
/// Whether RDNA1 (RX 5000 series) is the active discrete GPU driver.
pub const ENABLE_RDNA1_DRIVER: bool = false;
/// Whether NVIDIA drivers are active.
pub const ENABLE_NVIDIA_DRIVER: bool = false;
/// Whether Intel iGPU drivers are active.
pub const ENABLE_INTEL_GPU_DRIVER: bool = false;

// â”€â”€ Memory & platform â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€

/// Total physical address bits (5600X: 40 bits = 1 TB).
pub const PHYS_ADDR_BITS: u8 = 40;
/// Total virtual address bits (5600X: 48 bits, no LA57).
pub const VIRT_ADDR_BITS: u8 = 48;
/// Cache line size in bytes.
pub const CACHE_LINE_BYTES: u8 = 64;
/// Number of physical cores.
pub const CORE_COUNT: u32 = 6;
/// Number of logical threads.
pub const THREAD_COUNT: u32 = 12;

// â”€â”€ Human-readable strings â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€

/// Vendor name.
pub const CPU_VENDOR: &str = "AMD";
/// CPU family name.
pub const CPU_FAMILY: &str = "Zen 3";
/// CPU codename.
pub const CPU_CODE_NAME: &str = "Vermeer";
/// CPU product name.
pub const CPU_NAME: &str = "Ryzen 5 5600X";
/// CPU uarch.
pub const CPU_UARCH: &str = "Zen 3";
/// Microcode signature (BKDG, Family 19h).
pub const MICROCODE_SIGNATURE: &str = "0x0A0011B3";

/// Returns true if the CPU has the given feature.
/// v1.8.8: not const because &str comparison is not const yet.
pub fn has_feature(name: &str) -> bool {
    let b = name.as_bytes();
    if b.eq_ignore_ascii_case(b"sse")       { return ENABLE_SSE; }
    if b.eq_ignore_ascii_case(b"sse2")      { return ENABLE_SSE2; }
    if b.eq_ignore_ascii_case(b"sse3")      { return ENABLE_SSE3; }
    if b.eq_ignore_ascii_case(b"sse4.1")    { return ENABLE_SSE41; }
    if b.eq_ignore_ascii_case(b"sse4.2")    { return ENABLE_SSE42; }
    if b.eq_ignore_ascii_case(b"avx")       { return ENABLE_AVX; }
    if b.eq_ignore_ascii_case(b"avx2")      { return ENABLE_AVX2; }
    if b.eq_ignore_ascii_case(b"avx512")    { return ENABLE_AVX512; }
    if b.eq_ignore_ascii_case(b"fma")       { return ENABLE_FMA; }
    if b.eq_ignore_ascii_case(b"bmi")       { return ENABLE_BMI; }
    if b.eq_ignore_ascii_case(b"aes-ni")    { return ENABLE_AES_NI; }
    if b.eq_ignore_ascii_case(b"sha-ni")    { return ENABLE_SHA_NI; }
    if b.eq_ignore_ascii_case(b"f16c")      { return ENABLE_F16C; }
    if b.eq_ignore_ascii_case(b"popcnt")    { return ENABLE_POPCNT; }
    if b.eq_ignore_ascii_case(b"lzcnt")     { return ENABLE_LZCNT; }
    if b.eq_ignore_ascii_case(b"rdrand")    { return ENABLE_RDRAND; }
    if b.eq_ignore_ascii_case(b"xsave")     { return ENABLE_XSAVE; }
    if b.eq_ignore_ascii_case(b"fsgsbase")  { return ENABLE_FSGSBASE; }
    if b.eq_ignore_ascii_case(b"smep")      { return ENABLE_SMEP; }
    if b.eq_ignore_ascii_case(b"smap")      { return ENABLE_SMAP; }
    if b.eq_ignore_ascii_case(b"umip")      { return ENABLE_UMIP; }
    if b.eq_ignore_ascii_case(b"rdtscp")    { return ENABLE_RDTSCP; }
    if b.eq_ignore_ascii_case(b"pcid")      { return ENABLE_PCID; }
    if b.eq_ignore_ascii_case(b"invpcid")   { return ENABLE_INVPCID; }
    if b.eq_ignore_ascii_case(b"mtrr")      { return ENABLE_MTRR; }
    if b.eq_ignore_ascii_case(b"pat")       { return ENABLE_PAT; }
    if b.eq_ignore_ascii_case(b"1gb_pages") { return ENABLE_1GB_PAGES; }
    if b.eq_ignore_ascii_case(b"perfctr")   { return ENABLE_PERFCTR; }
    if b.eq_ignore_ascii_case(b"la57")      { return ENABLE_LA57; }
    false
}
