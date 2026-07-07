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

// â”€â”€ CABINA: daemon + panels (omniscient diagnostic infrastructure) â”€â”€
pub use cabina_core;
pub use cabina_daemon;
pub use cabina_panels;
#[path = "../cabina/mod.rs"]
pub mod cabina;

// â”€â”€ BMO support dependencies â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€
#[path = "../bmo_gpu/mod.rs"]
pub mod bmo_gpu;


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

// â”€â”€ CPU-specific (AMD Ryzen 5 5600X / Zen 3) â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€
pub mod vendor;

// â”€â”€ Omniscient infrastructure â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€
pub mod omni;

// â”€â”€ Devour: PE/ELF â†’ BEF translation â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€
pub mod devour;

// â”€â”€ TrilogÃ­a subsystems (defense + timeback + userland) â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€
#[path = "../defense/mod.rs"]
pub mod defense;
#[path = "../timeback/mod.rs"]
pub mod timeback;
#[path = "../userland/mod.rs"]
pub mod userland;

// â”€â”€ Other Ring 0 modules â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€
mod panic;
pub mod profile;

pub use bmo_abi;

// Re-exports (BootInfo shared from bootloader)
pub use info::{BOOT_INFO, FB_ADDR, FB_WIDTH, FB_HEIGHT, FB_STRIDE, FB_PIXEL_FORMAT};

// ── Entry point ──────────────────────────────────────────

pub mod entry;
