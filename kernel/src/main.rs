//! BMO kernel â€” entry point Ãºnico.
//!
//! Boot order:
//!   1. `_start` (en ring0/mod.rs): BSS zero, guarda boot_info_ptr
//!   2. `kernel_main_real`: init temprano + breadcrumb NVRAM
//!   3. `phase_1_RING_0::main`: CPU, memoria, dispositivos, display
//!   4. Welcome shell / Desktop (bmo_core)

#![no_std]
#![no_main]
#![allow(static_mut_refs)]

extern crate alloc;

// â”€â”€â”€ Ring 0: HAL, boot, x86-64 â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€
// ring0 declara tambiÃ©n los submÃ³dulos: arch/, mm/, dev/, proc/, cpu/,
// cabina/, bmo_core/, y el _start asm + panic_handler.
pub mod ring0;

// â”€â”€â”€ Re-exports: legacy crate::<mod> paths â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€
// ring0/mod.rs era antes la crate root; todo el cÃ³digo usa paths como
// crate::arch, crate::mm, etc. sin prefijo "ring0::". Re-exportamos
// para mantener compatibilidad sin tocar 293 archivos fuente.
pub use ring0::arch;
pub use ring0::mm;
pub use ring0::dev;
pub use ring0::proc;
pub use ring0::cpu;
pub use ring0::cabina;
pub use ring0::bmo_gpu;
pub use ring0::info;
pub use ring0::context;
pub use ring0::uefi_rt;
pub use ring0::serial;
pub use ring0::visual;
pub use ring0::font;
pub use ring0::log;
pub use ring0::bmo_core;
pub use ring0::phase_1_RING_0;
pub use ring0::vendor;
pub use ring0::omni;
pub use ring0::devour;
pub use ring0::defense;
pub use ring0::timeback;
pub use ring0::userland;
pub use ring0::profile;
pub use ring0::cabina_core;
pub use ring0::cabina_daemon;
pub use ring0::cabina_panels;
pub use ring0::bmo_abi;
