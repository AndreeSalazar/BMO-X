//! Phased boot orchestration (v1.7.5).
//!
//! Each phase is a self-contained module under this folder.
//! The functions here are called in fixed order from
//! `crate::coordinator::main`.
//!
//! Phase order is load-bearing:
//!   - Phase 0 (arch): GDT + IDT + APIC before anything can fault
//!   - Phase 1 (mem):  heap + page_alloc before any Vec/Box
//!   - Phase 2 (dev):  console, framebuffer, pcie, watchdog
//!   - Phase 3 (proc): scheduler + idle task
//!   - Phase 4 (bmo): BMO Core init
//!   - Phase 5 (user): Ring 3 first process

pub mod p0_arch;
pub mod p1_mem;
pub mod p2_dev;
pub mod p3_proc;
pub mod p4_bmo;
pub mod p5_user;
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
    visual::log("ring0", "p0_arch", visual::color::OK);
    let (_cpu, out0) = p0_arch::run(ctx, t0);
    visual::end_phase(0);

    visual::begin_phase(1);
    visual::log("ring0", "p1_mem", visual::color::OK);
    let (_mem, out1) = p1_mem::run(ctx, out0.prev_end);
    visual::end_phase(1);

    visual::begin_phase(2);
    visual::log("ring0", "p2_dev", visual::color::OK);
    let out2 = p2_dev::run(ctx, out1.prev_end);
    visual::end_phase(2);

    visual::begin_phase(3);
    visual::log("ring0", "p3_proc", visual::color::OK);
    let out3 = p3_proc::run(ctx, out2.prev_end);
    visual::end_phase(3);

    visual::begin_phase(4);
    visual::log("ring0", "p4_bmo", visual::color::OK);
    let out4 = p4_bmo::run(out3.prev_end);
    visual::end_phase(4);

    out4.prev_end
}
