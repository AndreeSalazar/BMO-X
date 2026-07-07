//! `userland/` — Ring 3 userland subsystem.
//!
//! v1.8.8: active subsystem with desktop entry, BEF/ELF loader, and
//! window message dispatch. Manages Ring 3 processes and their lifecycle.
//!
//! ## Components
//!
//! - `ring_3`:    Ring 3 coordinator — process init, wnd_proc dispatch.
//! - `app`:       BEF/ELF binary loader — format detection, memory mapping, execution.
//!
//! ## Relationship with BMO Core
//!
//! In the Opus architecture, BMO Core is the "Ring 3 kernel":
//! - Receives control from Ring 0 at boot completion.
//! - Initializes windowing API, desktop, filesystem.
//! - userland::init() creates the desktop process.
//! - ring3::ring3_entry::enter() transitions to CPL=3.
//! - Apps use bmo_api syscalls (0x100..0x1FF) through the scheduler.

pub mod ring_3;
pub mod app;

pub use ring_3 as userland_impl;