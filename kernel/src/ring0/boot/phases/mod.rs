//! Phased boot orchestration.
//!
//! Each phase is a self-contained module under this folder. The functions
//! here are called in a fixed order from `crate::main::kernel_main_real`.
//!
//! Phase order is documented in the crate-level `main.rs` doc-comment and is
//! load-bearing: e.g. Phase 0 must install GDT/IDT before Phase 1 can fault
//! safely, and Phase 1 must bring up the heap before Phase 5 can use Vec.

pub mod phase0_cpu;
pub mod phase1_memory;
pub mod phase2_devices;
pub mod phase3_display;
pub mod phase4_scheduler;
pub mod phase5_desktop;
pub mod ring3_tests;

pub mod trait_def;
pub use trait_def::report as report_self_test;
