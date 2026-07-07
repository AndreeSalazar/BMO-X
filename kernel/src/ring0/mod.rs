//! Ring 0 â€” Hardware Abstraction Layer.
//!
//! Boot order (desde main.rs):
//!   1. _start: BSS zero, save boot_info_ptr
//!   2. kernel_main_real: early NVRAM breadcrumb
//!   3. boot_phase::main: full hardware init
//!   4. Ring 0 ready screen + heartbeat loop

// â”€â”€ Core Ring 0 modules â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€
pub mod arch;
pub mod mm;
pub mod dev;
pub mod proc;
pub mod cpu;
pub mod hal_init;
pub mod storage_hal_impl;

// ── CABINA: daemon + panels (omniscient diagnostic infrastructure) ─
pub use cabina_core;
pub use cabina_daemon;
pub use cabina_panels;

// ── Boot infrastructure (moved from boot/) ──────────────────────
pub mod info;
pub mod context;
pub mod uefi_rt;
pub mod serial;
pub mod visual;
pub mod font;
pub mod log;



// ── Main coordinator ────────────────────────────────────
pub mod boot_phase;

// ── Omniscient infrastructure ───────────────────────────
pub mod omni;

// â”€â”€ Devour: PE/ELF â†’ BEF translation â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€
pub mod devour;

// â”€â”€ TrilogÃ­a subsystems (defense + timeback + userland) â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€
pub use byte_defender;
pub use timeback;
#[path = "../userland/mod.rs"]
pub mod userland;

// â”€â”€ Other Ring 0 modules â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€
mod panic;
pub use hw_profile;

pub use bmo_abi;

// Re-exports (BootInfo shared from bootloader)
pub use info::{BOOT_INFO, FB_ADDR, FB_WIDTH, FB_HEIGHT, FB_STRIDE, FB_PIXEL_FORMAT};

// ── Entry point ──────────────────────────────────────────

pub mod entry;
