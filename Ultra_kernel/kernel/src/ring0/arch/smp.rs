//! SMP — stub for the Ring 0 base.
//!
//! Multi-core startup (INIT-SIPI-SIPI, AP trampoline, per-CPU data) is
//! a future-phase feature. The stub exists so that the rest of the
//! kernel can call `arch::smp::init()` and compile.

pub fn init() {}
pub fn smp_enabler_init() {}
pub fn online_count() -> u32 { 1 }
pub fn total_count()  -> u32 { 1 }
