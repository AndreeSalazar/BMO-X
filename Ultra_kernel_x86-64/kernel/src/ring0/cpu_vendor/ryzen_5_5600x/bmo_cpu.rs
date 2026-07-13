//! Consolidated BMO CPU API for the Ryzen 5 5600X.
//!
//! Recovers the legacy `bmo_cpu.rs` from the deleted
//! `crates_Personal/ring0/cpu_vendor_profile/.../bmo_cpu.rs`,
//! simplified to the bare minimum needed by the minimal Ring 0
//! base:
//! - one-shot `init_bmo_cpu()` runs CPUID detect, topology, cache,
//!   TSC, and errata mitigations, and stashes the results in
//!   `static`s.
//! - accessors: `identity()`, `topology()`, `cache()`,
//!   `tsc_freq_hz()`, `tsc_source()`, `is_initialized()`.
//!
//! This is the in-kernel analog of the legacy
//! `vendor::amd::cpu::zen3::bmo_cpu` module, with all of the
//! unused `power_management` / `msr_init` surface stripped out.

use core::sync::atomic::{AtomicBool, Ordering};
use super::cpuid::{self, CpuIdentity};
use super::topology::Topology;
use super::cache::{CacheTopology, detect_5600x};
use super::tsc::{self, TscSource};
use super::errata;

static mut CPU_IDENTITY: Option<CpuIdentity> = None;
static mut CPU_TOPOLOGY: Option<Topology> = None;
static mut CPU_CACHE:    Option<CacheTopology> = None;
static mut CPU_TSC_HZ:   u64 = 0;
static mut CPU_TSC_SRC:  Option<TscSource> = None;
static INITIALIZED: AtomicBool = AtomicBool::new(false);

pub fn identity()    -> Option<&'static CpuIdentity>   { unsafe { (&*core::ptr::addr_of!(CPU_IDENTITY)).as_ref() } }
pub fn topology()    -> Option<&'static Topology>        { unsafe { (&*core::ptr::addr_of!(CPU_TOPOLOGY)).as_ref() } }
pub fn cache()       -> Option<&'static CacheTopology>   { unsafe { (&*core::ptr::addr_of!(CPU_CACHE)).as_ref() } }
pub fn tsc_freq_hz() -> u64                            { unsafe { CPU_TSC_HZ } }
pub fn tsc_source()  -> Option<TscSource>               { unsafe { CPU_TSC_SRC } }
pub fn is_initialized() -> bool                         { INITIALIZED.load(Ordering::Acquire) }

/// One-shot init: detect everything, log it, apply errata.
/// Idempotent — second call is a no-op.
pub fn init_bmo_cpu() {
    if INITIALIZED.load(Ordering::Acquire) { return; }

    // 1. CPUID detect
    let id = cpuid::detect_cpu();
    unsafe { CPU_IDENTITY = Some(id); }

    crate::ring0::dev::console::serial_write("[bmo-cpu] ");
    crate::ring0::dev::console::serial_write(id.brand.as_str());
    crate::ring0::dev::console::serial_write(" (");
    crate::ring0::dev::console::serial_write(id.family_model.name());
    crate::ring0::dev::console::serial_write(")\n");

    // 2. Topology
    let topo = super::topology::detect_bsp();
    crate::ring0::dev::console::serial_write("[bmo-cpu] topology: ");
    crate::ring0::dev::console::serial_write_u64_dec(topo.total_threads as u64);
    crate::ring0::dev::console::serial_write(" threads / ");
    crate::ring0::dev::console::serial_write_u64_dec(topo.total_cores as u64);
    crate::ring0::dev::console::serial_write(" cores / 1 CCX / 1 CCD\n");
    unsafe { CPU_TOPOLOGY = Some(topo); }

    // 3. Cache
    let c = detect_5600x();
    crate::ring0::dev::console::serial_write("[bmo-cpu] cache: L1d 32K L1i 32K L2 512K L3 32M\n");
    unsafe { CPU_CACHE = Some(c); }

    // 4. TSC
    let (hz, src) = tsc::calibrate();
    crate::ring0::dev::console::serial_write("[bmo-cpu] TSC: ");
    crate::ring0::dev::console::serial_write_u64_dec(hz);
    crate::ring0::dev::console::serial_write(" Hz (");
    crate::ring0::dev::console::serial_write(src.name());
    crate::ring0::dev::console::serial_write(")\n");
    unsafe { CPU_TSC_HZ = hz; CPU_TSC_SRC = Some(src); }

    // 5. Errata workarounds
    let applied = errata::apply_all();
    crate::ring0::dev::console::serial_write("[bmo-cpu] errata: ");
    if applied & (1 << 0) != 0 { crate::ring0::dev::console::serial_write("IBRS "); }
    if applied & (1 << 1) != 0 { crate::ring0::dev::console::serial_write("STIBP "); }
    if applied & (1 << 2) != 0 { crate::ring0::dev::console::serial_write("SSBD "); }
    if applied & (1 << 3) != 0 { crate::ring0::dev::console::serial_write("IBPB "); }
    if applied & (1 << 4) != 0 { crate::ring0::dev::console::serial_write("TSX-OFF "); }
    crate::ring0::dev::console::serial_write("\n");

    INITIALIZED.store(true, Ordering::Release);
}
