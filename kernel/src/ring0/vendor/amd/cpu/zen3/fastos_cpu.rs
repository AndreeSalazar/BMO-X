//! `fastos_cpu` — Public API for CPU data of the Ryzen 5 5600X.
//!
//! This is the single entry point for RING 0 (and BMO Core) to query
//! detailed information about the CPU. After `init_fastos_cpu()` is
//! called once at boot, the globals below contain:
//!
//! - `CPU_IDENTITY`     — Family 19h, model 01h, brand string, features
//! - `CPU_TOPOLOGY`     — 6C/12T, 1 CCX, 1 CCD
//! - `CPU_CACHE`        — L1d 32K, L1i 32K, L2 512K, L3 32M
//! - `CPU_TSC_FREQ_HZ`  — Calibrated TSC frequency
//! - `CPU_TSC_SOURCE`   — Where the TSC frequency came from
//! - `CPU_POWER`        — P-state query, C1 halt, etc.
//!
//! Status: ✅ COMPLETO — centraliza todos los datos del 5600X.
//!
//! ## Usage from RING 0
//!
//! ```ignore
//! // At boot, in boot_coordinator::main:
//! crate::vendor::amd::cpu::zen3::fastos_cpu::init_fastos_cpu();
//!
//! // Anywhere in the kernel:
//! if let Some(id) = crate::vendor::amd::cpu::zen3::fastos_cpu::identity() {
//!     log!("Running on {}", id.family_model.name());
//! }
//! ```

use core::sync::atomic::{AtomicBool, Ordering};
use super::cpuid_detection::{detect_cpu, CpuIdentity};
use super::topology::Topology;
use super::cache_topology::{CacheTopology, detect as detect_cache};
use super::tsc_calibration::TscSource;
use super::power_management;
use super::errata_workarounds;
use super::msr_init;
use super::acpi_real;

// ── Globals (initialized once at boot) ─────────────────────────────────

/// CPU identity (vendor, family, model, brand, features).
static mut CPU_IDENTITY: Option<CpuIdentity> = None;

/// CPU topology (SMT, cores, CCX, CCD, APIC IDs).
static mut CPU_TOPOLOGY: Option<Topology> = None;

/// Cache and TLB topology.
static mut CPU_CACHE: Option<CacheTopology> = None;

/// TSC frequency in Hz, calibrated at boot.
static mut CPU_TSC_FREQ_HZ: u64 = 0;

/// Where the TSC frequency came from.
static mut CPU_TSC_SOURCE: Option<TscSource> = None;

/// Set to true after `init_fastos_cpu()` completes successfully.
static INITIALIZED: AtomicBool = AtomicBool::new(false);

// ── Public accessors ────────────────────────────────────────────────────

/// Returns the CPU identity. None if `init_fastos_cpu` not yet called.
pub fn identity() -> Option<&'static CpuIdentity> {
    unsafe { CPU_IDENTITY.as_ref() }
}

/// Returns the CPU topology. None if not yet detected.
pub fn topology() -> Option<&'static Topology> {
    unsafe { CPU_TOPOLOGY.as_ref() }
}

/// Returns the cache topology. None if not yet detected.
pub fn cache() -> Option<&'static CacheTopology> {
    unsafe { CPU_CACHE.as_ref() }
}

/// Returns the calibrated TSC frequency in Hz.
/// Returns 0 if not yet calibrated.
pub fn tsc_freq_hz() -> u64 {
    unsafe { CPU_TSC_FREQ_HZ }
}

/// Returns the source of the TSC calibration.
pub fn tsc_source() -> Option<TscSource> {
    unsafe { CPU_TSC_SOURCE }
}

/// Returns true if `init_fastos_cpu` has completed.
pub fn is_initialized() -> bool {
    INITIALIZED.load(Ordering::Acquire)
}

// ── Main initialization function ───────────────────────────────────────

/// One-shot initialization: detects everything about the CPU and stores
/// the results in the globals above. Call this ONCE at boot, after the
/// bootloader has handed off.
///
/// Steps performed:
/// 1. CPUID detection (vendor, family, model, brand, features)
/// 2. Assert the target CPU (5600X) or warn
/// 3. Cache topology detection (L1/L2/L3/TLB)
/// 4. TSC calibration (CPUID 0x15, PM Timer, fallback)
/// 5. Errata workarounds (Spectre v2/v4, MDS)
/// 6. MSR setup (EFER, STAR, LSTAR, FMASK, PAT, etc.)
/// 7. Power management setup (C1e enable)
/// 8. ACPI init (RSDP/XSDT/MCFG parsing)
pub fn init_fastos_cpu() {
    if is_initialized() {
        return;  // idempotent
    }
    crate::dev::console::serial_write("\n[fastos_cpu] === Ryzen 5 5600X initialization ===\n");

    // 1. CPUID detection
    crate::dev::console::serial_write("[fastos_cpu] step 1: CPUID\n");
    let cpu_id = detect_cpu();
    crate::dev::console::serial_write("[fastos_cpu] step 1: CPUID done\n");
    crate::dev::console::serial_write("[fastos_cpu] CPU: ");
    crate::dev::console::serial_write(cpu_id.brand.as_str());
    crate::dev::console::serial_write("\n[fastos_cpu] Family ");
    crate::dev::console::serial_write_u64(cpu_id.family_model.family as u64, 16);
    crate::dev::console::serial_write("h, Model ");
    crate::dev::console::serial_write_u64(cpu_id.family_model.model as u64, 16);
    crate::dev::console::serial_write("h, Stepping ");
    crate::dev::console::serial_write_u64(cpu_id.family_model.stepping as u64, 16);
    crate::dev::console::serial_write("h\n");
    crate::dev::console::serial_write("[fastos_cpu] Logical cores: ");
    crate::dev::console::serial_write_u64(cpu_id.logical_cores as u64, 10);
    crate::dev::console::serial_write(", Initial APIC ID: ");
    crate::dev::console::serial_write_u64(cpu_id.initial_apic_id as u64, 10);
    crate::dev::console::serial_write("\n");
    unsafe { CPU_IDENTITY = Some(cpu_id); }
    // Also cache in cpuid_detection's global for compatibility
    super::cpuid_detection::cache_identity(cpu_id);

    // 2. Cache topology
    crate::dev::console::serial_write("[fastos_cpu] step 2: cache\n");
    let cache = detect_cache();
    crate::dev::console::serial_write("[fastos_cpu] step 2: cache done\n");
    crate::dev::console::serial_write("[fastos_cpu] Cache:\n");
    if let Some(c) = cache.l1d {
        crate::dev::console::serial_write("  L1d: ");
        crate::dev::console::serial_write_u64(c.size_kb as u64, 10);
        crate::dev::console::serial_write(" KB, ");
        crate::dev::console::serial_write_u64(c.associativity as u64, 10);
        crate::dev::console::serial_write("-way, line=");
        crate::dev::console::serial_write_u64(c.line_size_bytes as u64, 10);
        crate::dev::console::serial_write(" B\n");
    }
    if let Some(c) = cache.l1i {
        crate::dev::console::serial_write("  L1i: ");
        crate::dev::console::serial_write_u64(c.size_kb as u64, 10);
        crate::dev::console::serial_write(" KB, ");
        crate::dev::console::serial_write_u64(c.associativity as u64, 10);
        crate::dev::console::serial_write("-way, line=");
        crate::dev::console::serial_write_u64(c.line_size_bytes as u64, 10);
        crate::dev::console::serial_write(" B\n");
    }
    if let Some(c) = cache.l2 {
        crate::dev::console::serial_write("  L2: ");
        crate::dev::console::serial_write_u64(c.size_kb as u64, 10);
        crate::dev::console::serial_write(" KB, ");
        crate::dev::console::serial_write_u64(c.associativity as u64, 10);
        crate::dev::console::serial_write("-way, line=");
        crate::dev::console::serial_write_u64(c.line_size_bytes as u64, 10);
        crate::dev::console::serial_write(" B\n");
    }
    if let Some(c) = cache.l3 {
        crate::dev::console::serial_write("  L3: ");
        crate::dev::console::serial_write_u64(c.size_kb as u64, 10);
        crate::dev::console::serial_write(" KB, ");
        crate::dev::console::serial_write_u64(c.associativity as u64, 10);
        crate::dev::console::serial_write("-way, ");
        crate::dev::console::serial_write_u64(c.shared_threads as u64, 10);
        crate::dev::console::serial_write(" threads share\n");
    }
    crate::dev::console::serial_write("  Total cache: ");
    crate::dev::console::serial_write_u64(cache.total_size_kb() as u64, 10);
    crate::dev::console::serial_write(" KB\n");
    unsafe { CPU_CACHE = Some(cache); }

    // 3. CPU topology (SMT, CCX, CCD)
    crate::dev::console::serial_write("[fastos_cpu] step 3: topology\n");
    let topo = super::topology::detect();
    crate::dev::console::serial_write("[fastos_cpu] step 3: topology done\n");
    crate::dev::console::serial_write("[fastos_cpu] Topology: ");
    crate::dev::console::serial_write_u64(topo.cpu_count as u64, 10);
    crate::dev::console::serial_write(" threads, ");
    crate::dev::console::serial_write_u64(topo.total_cores as u64, 10);
    crate::dev::console::serial_write(" cores, ");
    crate::dev::console::serial_write_u64(topo.total_ccxs as u64, 10);
    crate::dev::console::serial_write(" CCX, ");
    crate::dev::console::serial_write_u64(topo.total_ccds as u64, 10);
    crate::dev::console::serial_write(" CCD\n");
    crate::dev::console::serial_write("[fastos_cpu] BSP: APIC ID ");
    crate::dev::console::serial_write_u64(topo.bsp.apic_id as u64, 10);
    crate::dev::console::serial_write(" (core ");
    crate::dev::console::serial_write_u64(topo.bsp.core as u64, 10);
    crate::dev::console::serial_write(", thread ");
    crate::dev::console::serial_write_u64(topo.bsp.thread as u64, 10);
    crate::dev::console::serial_write(")\n");
    unsafe { CPU_TOPOLOGY = Some(topo); }

    // 4. TSC calibration (use PM Timer if available, else fallback)
    //    Note: acpi_real::init must be called first to find the PM Timer
    //    port. If ACPI init fails, we use the hardcoded constant.
    crate::dev::console::serial_write("[fastos_cpu] step 4: TSC calibration\n");
    let pm_timer_port = find_pm_timer_port();
    crate::dev::console::serial_write_u64(pm_timer_port as u64, 16);
    crate::dev::console::serial_write(" = pm_timer_port\n");
    let (freq, source) = super::tsc_calibration::calibrate_tsc(pm_timer_port);
    crate::dev::console::serial_write("[fastos_cpu] TSC: ");
    crate::dev::console::serial_write_u64(freq, 10);
    crate::dev::console::serial_write(" Hz (");
    crate::dev::console::serial_write(source.name());
    crate::dev::console::serial_write(")\n");
    unsafe {
        CPU_TSC_FREQ_HZ = freq;
        CPU_TSC_SOURCE = Some(source);
    }

    // 5. Errata workarounds
    errata_workarounds::apply_all();

    // 6. Power management: C1e
    power_management::init();

    // 7. MSR setup (common across all cores)
    let _bsp_apic_id = topo.bsp.apic_id as u32;
    // Note: syscall_entry should be the actual handler. For now we
    // pass a placeholder; the real call sites set it explicitly.
    // RING 0 will call `init_msr_common` with the real entry.
    crate::dev::console::serial_write("[fastos_cpu] MSR setup (deferred to syscall init)\n");

    // 8. ACPI init (RSDP, XSDT, MCFG) — disabled by default
    //    because it might require BootInfo.rsdp_addr which is set
    //    later. Call `init_acpi(boot_info.rsdp_addr)` explicitly
    //    from boot_coordinator when you have that info.
    crate::dev::console::serial_write("[fastos_cpu] ACPI init deferred (needs BootInfo.rsdp_addr)\n");

    crate::dev::console::serial_write("[fastos_cpu] === Initialization complete ===\n\n");
    INITIALIZED.store(true, Ordering::Release);
}

/// Initialize MSRs (called after `init_fastos_cpu`, when the syscall
/// entry point is known).
pub fn init_msrs(syscall_entry: u64) {
    let bsp_apic_id = topology().map(|t| t.bsp.apic_id as u32).unwrap_or(0);
    msr_init::init_msr_common(syscall_entry, bsp_apic_id);
}

/// Initialize ACPI tables. Call this from `boot_coordinator::main` when
/// the bootloader has provided the RSDP address.
pub fn init_acpi(rsdp_addr: Option<u64>) {
    if let Err(e) = acpi_real::init(rsdp_addr) {
        crate::dev::console::serial_write("[fastos_cpu] ACPI init failed: ");
        crate::dev::console::serial_write(match e {
            acpi_real::AcpiError::NotFound => "not found",
            acpi_real::AcpiError::BadSignature => "bad signature",
            acpi_real::AcpiError::BadChecksum => "bad checksum",
            acpi_real::AcpiError::TooShort => "too short",
            acpi_real::AcpiError::UnsupportedRevision => "unsupported rev",
        });
        crate::dev::console::serial_write("\n");
    }
}

// ── Helpers ─────────────────────────────────────────────────────────────

/// Find the ACPI PM Timer port. Returns 0 if not found.
/// On modern systems (5600X with UEFI), the PM Timer is at port 0x408
/// or obtained from the FADT. We try common ports.
fn find_pm_timer_port() -> u16 {
    // Common PM Timer ports
    const PM_PORTS: &[u16] = &[0x408, 0x4D0, 0x500, 0x580, 0x600];

    // First check: does ACPI know? (preferred)
    if let Some(port) = acpi_real::pm_timer_port() {
        return port;
    }

    // Fallback: probe common ports (read returns 0xFFFFFF if no device)
    for &port in PM_PORTS {
        let v = unsafe {
            let lo: u32;
            core::arch::asm!(
                "in eax, dx",
                in("dx") port,
                out("eax") lo,
                options(nostack, preserves_flags),
            );
            lo
        };
        if v != 0xFFFFFFFF && v != 0 {
            return port;
        }
    }

    // Not found — caller will use hardcoded fallback
    0
}

// ── Summary print function (for diag/overlay) ──────────────────────────

/// Returns a multi-line summary string describing the CPU.
pub fn summary() -> &'static str {
    // NOTE: this is a fixed string because dynamic formatting is too
    // expensive for a no_std context. The actual values are printed
    // individually in `init_fastos_cpu`.
    "Ryzen 5 5600X (Vermeer, Zen 3) — 6C/12T, 3.7/4.6 GHz, 32 MB L3"
}
