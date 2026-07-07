//! Ring 0 — Hardware Abstraction Layer.
//!
//! ## Boot order
//!
//!   1. `_start` (entry.rs): BSS zero, save boot_info_ptr
//!   2. `kernel_main_real` (entry.rs): early NVRAM breadcrumbs
//!   3. `boot_phase::main`: Phase 0→4 → CPU detect → ACPI → SMP
//!   4. Desktop / Welcome shell (bmo_core)

// ═══════════════════════════════════════════════════════════════════
//  Core — CPU architecture, memory bootstrap, devices, scheduler
// ═══════════════════════════════════════════════════════════════════

pub mod arch;       // GDT, IDT, APIC, syscall, SMP, context
pub mod mm;          // Frame allocator, slab heap, VMM, page tables
pub mod dev;         // Console, PCIe, framebuffer, watchdog, HDA, ACPI
pub mod proc;        // Process table, task scheduler (touches CR3)
pub mod cpu;         // CPU features, TSC, registers, cache, FPU

// ═══════════════════════════════════════════════════════════════════
//  Infrastructure — boot, HAL wiring, storage bridge
// ═══════════════════════════════════════════════════════════════════

pub mod boot_phase;
pub mod hal_init;
pub mod storage_hal_impl;
pub mod entry;

// ═══════════════════════════════════════════════════════════════════
//  Boot services — info, context, UEFI runtime, serial, display
// ═══════════════════════════════════════════════════════════════════

pub mod info;
pub mod context;
pub mod uefi_rt;
pub mod serial;
pub mod visual;
pub mod font;
pub mod log;

// ═══════════════════════════════════════════════════════════════════
//  Subsystems — Devour (PE/ELF→BEF), omni, panic
// ═══════════════════════════════════════════════════════════════════

pub mod devour;
pub mod omni;
mod panic;

// ═══════════════════════════════════════════════════════════════════
//  External crates (extracted to crates_Personal/)
// ═══════════════════════════════════════════════════════════════════

pub use cabina_core;
pub use cabina_daemon;
pub use cabina_panels;
pub use bmo_abi;

pub use byte_defender;
pub use timeback;
pub use hw_profile;

// ═══════════════════════════════════════════════════════════════════
//  Userland bridge — kernel-side Ring 3 process management
// ═══════════════════════════════════════════════════════════════════

#[path = "../userland/mod.rs"]
pub mod userland;

// ═══════════════════════════════════════════════════════════════════
//  Re-exports — framebuffer globals shared with bootloader
// ═══════════════════════════════════════════════════════════════════

pub use info::{BOOT_INFO, FB_ADDR, FB_WIDTH, FB_HEIGHT, FB_STRIDE, FB_PIXEL_FORMAT};
