//! BMO kernel â€” entry point Ãºnico.
//!
//! Boot order:
//!   1. `_start` (en ring0/entry.rs): BSS zero, guarda boot_info_ptr
//!   2. `kernel_main_real`: init temprano + breadcrumb NVRAM
//!   3. `boot_phase::main`: CPU, memoria, dispositivos, display
//!   4. Welcome shell / Desktop (bmo_core)

#![no_std]
#![no_main]
#![allow(static_mut_refs)]

extern crate alloc;

// â”€â”€â”€ Ring 0: HAL, boot, x86-64 â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€
// ring0 declara tambiÃ©n los submÃ³dulos: arch/, mm/, dev/, proc/, cpu/,
// cabina/, bmo_core/, y el _start asm + panic_handler.
pub mod ring0;

// ─── Re-exports: legacy crate::<mod> paths ─────────────────────────
// ring0/mod.rs was originally the crate root; all code uses paths like
// crate::arch, crate::mm, etc. without the ring0:: prefix. We re-export
// to maintain compatibility without touching 293 source files.
//
// TODO(v1.9): migrate all 293 files to use explicit crate::ring0::* paths,
// then delete this re-export block. Start with internal ring0 modules
// (arch, mm, dev, cpu, proc) — they're self-contained and don't break
// external consumers.
pub use ring0::arch;
pub use ring0::mm;
pub use ring0::dev;
pub use ring0::proc;
pub use ring0::cpu;
pub use ring0::info;
pub use ring0::context;
pub use ring0::uefi_rt;
pub use ring0::serial;
pub use ring0::visual;
pub mod ring3;
pub use ring0::font;
pub use ring0::log;
pub use bmo_core;
pub use ring0::boot_phase;
pub use ring0::entry;
pub use ring0::vendor;
pub use ring0::omni;
pub use ring0::devour;
pub use byte_defender;
pub use timeback;
pub use ring0::userland;
pub use hw_profile;
pub use ring0::cabina_core;
pub use ring0::cabina_daemon;
pub use ring0::cabina_panels;
pub use ring0::bmo_abi;
pub use ring0::hal_init;
