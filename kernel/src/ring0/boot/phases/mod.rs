//! Phased boot orchestration (v1.8.15).
//!
//! Each phase is a self-contained module under this folder.
//! The functions here are called in fixed order from
//! `crate::coordinator::main`.
//!
//! v1.8.15: Phase 0 corre DENTRO de run_phases_0_to_4.
//! init_fastos_cpu y init_acpi corren en coordinator entre las fases
//! y bmo_core::init, para no duplicar la inicialización de MSRs.
//!
//! Phase order is load-bearing:
//!   - Phase 0 (arch):    GDT + IDT + syscall + FPU (init_syscall llama init_msrs)
//!   - Phase 1 (mem):     frame allocator + heap before any Vec/Box
//!   - Phase 2 (dev):     ACPI/PCI discovery; fragile services deferred
//!   - Phase 3 (display): GOP framebuffer (con MTRR/PAT via cache::init)
//!   - Phase 4 (sched):   scheduler + APIC timer + interrupts
//!
//! NOTA v1.8.7: Phase 5 (BMO Core handoff) se mantiene como módulo
//! `p5_user` solo por compatibilidad con `bmo_core::desktop::welcome`
//! que consulta `p5_user::self_test()`. La fase 5 real está en
//! `coordinator::dispatch_phase5`, que llama a `bmo_core::coord::enter()`
//! y nunca retorna.

pub mod p0_arch;
pub mod p1_mem;
pub mod p2_dev;
pub mod p3_display;
pub mod p4_bmo;
pub mod p5_user;

pub mod trait_def;
pub use trait_def::report as report_self_test;

use super::BootContext;
use super::visual;

/// Run phases 0-4 in order. Returns the TSC timestamp of phase 4 end.
///
/// v1.8.16: NVRAM markers written BEFORE any framebuffer access.
/// The previous version wrote `p0_arch` AFTER `visual::begin_phase(0)`
/// which draws the full splash screen. If the splash crashed (bad fb
/// pointer, unmapped memory, font rendering bug), the crash.log would
/// only show `phase_0_to_4` from the coordinator — useless for diagnosis.
///
/// Now the NVRAM marker is the FIRST thing written in each phase, before
/// touching the framebuffer. If `visual::begin_phase(0)` crashes, the
/// crash.log will still show `phase_0_to_4`, but if p0_arch::run crashes
/// after the marker, we'll see `p0_arch` — and so on for each phase.
pub fn run_phases_0_to_4(ctx: &mut BootContext, t0: u64) -> u64 {
    // Phase 0: CPU init (GDT + IDT + syscall + FPU)
    crate::coordinator::write_crash_marker(20);
    crate::boot::uefi_rt::write_boot_stage("p0_arch");
    visual::begin_phase(0);
    visual::log("ring0", "p0_arch", visual::color::OK);
    let (_cpu, out0) = p0_arch::run(ctx, t0);
    visual::end_phase(0);

    // Phase 1: Memory (frame alloc + heap + high-mem)
    crate::coordinator::write_crash_marker(21);
    crate::boot::uefi_rt::write_boot_stage("p1_mem");
    visual::begin_phase(1);
    visual::log("ring0", "p1_mem", visual::color::OK);
    let (_mem, out1) = p1_mem::run(ctx, out0.prev_end);
    visual::end_phase(1);

    // Phase 2: Devices (PCI + AHCI + exFAT)
    crate::coordinator::write_crash_marker(22);
    crate::boot::uefi_rt::write_boot_stage("p2_dev");
    visual::begin_phase(2);
    visual::log("ring0", "p2_dev", visual::color::OK);
    let out2 = p2_dev::run(ctx, out1.prev_end);
    visual::end_phase(2);

    // Phase 3: Display (GOP framebuffer)
    crate::coordinator::write_crash_marker(23);
    crate::boot::uefi_rt::write_boot_stage("p3_display");
    visual::begin_phase(3);
    visual::log("ring0", "p3_display", visual::color::OK);
    let out3 = p3_display::run(ctx, out2.prev_end);
    visual::end_phase(3);

    // Phase 4: Scheduler (APIC timer + interrupts)
    crate::coordinator::write_crash_marker(24);
    crate::boot::uefi_rt::write_boot_stage("p4_bmo");
    visual::begin_phase(4);
    visual::log("ring0", "p4_bmo", visual::color::OK);
    let out4 = p4_bmo::run(out3.prev_end);
    visual::end_phase(4);

    out4.prev_end
}
