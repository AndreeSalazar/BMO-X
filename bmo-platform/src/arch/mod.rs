//! # Architecture abstraction
//!
//! The `Arch` trait is the **only** thing that differs between CPU
//! ports. Every other `bmo-platform` type is built on top of it.
//!
//! Today only `x86_64` is implemented. To port:
//!
//! 1. Add `src/arch/<your_arch>.rs` implementing `Arch`.
//! 2. Add a `#[cfg(target_arch = "...")]` arm in `current()` below.
//! 3. Add a `arch-<your_arch>` feature in `Cargo.toml`.
//!
//! The trait surface intentionally has **no** per-arch methods. Every
//! method is expressed in CPU-agnostic terms (e.g. `idle` says "park
//! the core until the next interrupt", not `hlt`). The implementation
//! behind each method is what's CPU-specific.

use core::arch::asm;

/// The active architecture. Selected at compile time via `cfg`.
///
/// A future AArch64 port would add:
/// ```ignore
/// #[cfg(target_arch = "aarch64")]
/// pub use self::aarch64::AArch64 as Current;
/// ```
#[cfg(target_arch = "x86_64")]
pub use self::x86_64::X86_64 as Current;

#[cfg(not(any(target_arch = "x86_64")))]
compile_error!(
    "bmo-platform: no Arch implementation for this target_arch. \
     Only x86_64 is currently supported. See src/arch/mod.rs for the \
     porting matrix."
);

/// The platform abstraction. Implemented once per CPU architecture.
///
/// **All methods must be safe to call from any Ring 3 context.** The
/// implementation is responsible for ensuring the right privilege
/// level, the right memory mappings, and the right memory ordering
/// for each operation. Ring 3 code never has to think about any of
/// that — it just calls `Arch::syscall(SYS_DRAW_RECT, ...)`.
pub trait Arch {
    // ── Identity ─────────────────────────────────────────────────
    /// Architecture name (e.g. `"x86_64"`, `"aarch64"`).
    fn name(&self) -> &'static str;

    /// Pointer width in bits (64 on every supported arch today).
    fn bits(&self) -> u32 { 64 }

    /// Little-endian? True for x86_64, aarch64, riscv64.
    fn little_endian(&self) -> bool { true }

    // ── CPU primitives ───────────────────────────────────────────

    /// Read the CPU's cycle counter / time-base counter. The
    /// `tsc_freq_hz` value passed in is the calibration from the
    /// kernel — for x86_64 it's TSC, for aarch64 it's `CNTVCT_EL0`,
    /// etc. The implementation scales appropriately.
    fn monotonic_ns(&self, tsc_freq_hz: u64) -> u64;

    /// Spin-wait hint. x86_64 uses `pause`, aarch64 uses `yield`,
    /// riscv64 uses nothing. The point is to give the front-end
    /// a chance to schedule a sibling hardware thread.
    fn spin_hint(&self);

    /// Park the core until the next interrupt. x86_64 uses `hlt`,
    /// aarch64 uses `wfi`, riscv64 uses `wfi` too. This is what
    /// userland calls when it has no work to do.
    fn idle(&self) -> !;

    /// Memory fence. After this returns, all prior stores are
    /// visible to all other cores. Maps to `mfence` on x86_64,
    /// `dmb ish` on aarch64, `fence rw,w` on riscv64.
    fn full_fence(&self);

    // ── Inter-ring communication ─────────────────────────────────

    /// Issue a BMO syscall to Ring 0. `nr` is one of the constants
    /// in `bmo_abi::syscalls`, `args` is up to 6 u64s (matches
    /// the System V AMD64 calling convention and most RISC ABIs).
    ///
    /// On x86_64 this emits `syscall`; on aarch64 it would emit
    /// `svc #0`; on riscv64 it would emit `ecall`.
    ///
    /// Returns the kernel's `rax` (or equivalent return register).
    fn syscall(&self, nr: u64, args: &[u64; 6]) -> u64;

    // ── CPU information (filled in at boot) ──────────────────────

    /// Number of logical processors visible to this core.
    /// The kernel publishes this in `BootContextV1.logical_cores`.
    fn logical_cores(&self) -> u32;

    /// Vendor name (e.g. `"AuthenticAMD"`, `"GenuineIntel"`).
    fn vendor(&self) -> &str;

    /// Brand string (e.g. `"AMD Ryzen 5 5600X 6-Core Processor"`).
    /// Up to 48 bytes including the NUL terminator.
    fn brand(&self) -> &str;
}

// ── The active arch is exposed as a single static ─────────────────
//
// `Arch::current()` returns a `&'static dyn Arch`. The first call
// initializes a `Current` instance from `BootContextV1` (CPUID output,
// TSC calibration, etc.) and stashes it in a `static`. Subsequent
// calls return that same instance.
//
// Why `&'static dyn` and not a generic? Because userland code
// wants to call `with_arch(|a| a.syscall(...))` without generic
// parameters — the platform version is fixed at boot, not at
// compile time (the BEF binary doesn't know which arch it will
// run on; only the loader knows).

use core::sync::atomic::{AtomicUsize, Ordering};

const UNINIT: usize = 0;
const INIT:   usize = 1;

static STATE: AtomicUsize = AtomicUsize::new(UNINIT);

// We use a `MaybeUninit` because the active arch struct is not
// `Sync` until after init, and we cannot put it in a `static`
// directly. This works for x86_64; a future aarch64 / riscv64
// would need its own `MaybeUninit` block, or a generic over the
// arch type via a trait-object trick. For one arch at a time this
// is fine.
static mut STORAGE: core::mem::MaybeUninit<x86_64::X86_64> =
    core::mem::MaybeUninit::uninit();

/// Get a reference to the active `Arch` implementation.
///
/// # Panics
/// Panics if called before [`crate::runtime::boot`].
pub fn current() -> &'static dyn Arch {
    match STATE.load(Ordering::Acquire) {
        INIT => unsafe { &*STORAGE.as_ptr() },
        _ => panic!("bmo_platform::arch::current() called before boot"),
    }
}

/// Called once from [`crate::runtime::boot`]. Stores the active
/// `Arch` impl in the platform's `static` so future calls to
/// [`current`] are zero-cost.
pub(crate) unsafe fn install(arch: x86_64::X86_64) {
    STORAGE.as_mut_ptr().write(arch);
    STATE.store(INIT, Ordering::Release);
}

// ── x86_64 module declaration ────────────────────────────────────
#[cfg(target_arch = "x86_64")]
pub mod x86_64;
