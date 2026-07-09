#![no_std]
#![allow(dead_code, static_mut_refs)]
extern crate alloc;

pub mod hal;

pub mod port_io;
pub mod bmo_api;
pub mod desktop;
pub mod gateway;
pub mod ui;
pub mod bef;
pub mod fs;

pub mod dev;

// ── Plugin Loader — runtime symbol resolution ───────────────────
pub mod plugin_loader;
pub mod mm;
pub mod info;
pub mod cpu;
pub mod uefi_rt;
pub mod visual;
pub mod font;
pub mod log;
pub mod cabina;
pub mod serial;
pub mod profile;
pub mod context;
pub mod phase_1_RING_0;
pub mod defense;
pub mod timeback;
pub mod userland;
pub mod vendor;
pub mod bmo_audio;
pub mod omni;
pub mod arch;
pub mod ring3;

pub mod proc;

pub use bmo_abi;
pub use hal::init as hal_init;

#[path = "bmo_core.rs"]
pub mod coord;
