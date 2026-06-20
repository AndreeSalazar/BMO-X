//! Phased boot orchestration.
//!
//! Each phase is a self-contained module under this folder. The functions
//! here are called in a fixed order from `crate::ring_0::ring_0::main`.
//!
//! Phase order is load-bearing: e.g. Phase 0 must install GDT/IDT before
//! Phase 1 can fault safely, and Phase 1 must bring up the heap before
//! Phase 5 can use Vec.

pub mod phase0_cpu;
pub mod phase1_memory;
pub mod phase2_devices;
pub mod phase3_display;
pub mod phase4_scheduler;
pub mod phase5_desktop;
pub mod ring3_tests;

pub mod trait_def;
pub use trait_def::report as report_self_test;

use super::BootContext;
use super::visual;

/// Run all boot phases in order. Returns the boot timestamp and the
/// result of phase 4 (the last phase that returns state — phase 5
/// consumes the welcome screen and does not return).
pub fn run_all(ctx: &mut BootContext, t0: u64) -> u64 {
    visual::begin_phase(0);
    visual::log("ring0", "phase0_cpu", visual::color::OK);
    let (_cpu, out0) = phase0_cpu::run(ctx, t0);
    visual::end_phase(0);

    visual::begin_phase(1);
    visual::log("ring0", "phase1_memory", visual::color::OK);
    let (_mem, out1) = phase1_memory::run(ctx, out0.prev_end);
    visual::end_phase(1);

    visual::begin_phase(2);
    visual::log("ring0", "phase2_devices", visual::color::OK);
    let out2 = phase2_devices::run(ctx, out1.prev_end);
    visual::end_phase(2);

    visual::begin_phase(3);
    visual::log("ring0", "phase3_display", visual::color::OK);
    let out3 = phase3_display::run(ctx, out2.prev_end);
    visual::end_phase(3);

    visual::begin_phase(4);
    visual::log("ring0", "phase4_scheduler", visual::color::OK);
    let out4 = phase4_scheduler::run(out3.prev_end);
    visual::end_phase(4);

    out4.prev_end
}
