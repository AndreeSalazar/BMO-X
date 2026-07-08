//! BMO kernel — entry point.
//!
//! ## Boot order
//!
//!   1. `_start` (ring0/entry.rs): BSS zero, save boot_info_ptr
//!   2. `kernel_main_real`: early NVRAM breadcrumbs
//!   3. `boot_phase::main`: CPU, memory, devices, display
//!   4. Desktop / Welcome shell (bmo_core)
//!
//! ## Architecture
//!
//! The kernel only wires crates together. All real logic lives in
//! `crates_Personal/`. Ring 0 owns: entry, boot sequence, GDT/IDT,
//! page table bootstrap, scheduler, fault handler.

#![no_std]
#![no_main]
#![allow(static_mut_refs)]

extern crate alloc;

// ═══════════════════════════════════════════════════════════════════
//  Ring 0 — Hardware Abstraction Layer
// ═══════════════════════════════════════════════════════════════════

pub mod ring0;

// ═══════════════════════════════════════════════════════════════════
//  Ring 0 internals (module declarations without re-export)
//  arch, mm, dev, proc, cpu, info, context, uefi_rt, serial, visual,
//  font, log, boot_phase, entry, omni, devour, panic, hal_init
// ═══════════════════════════════════════════════════════════════════

// ═══════════════════════════════════════════════════════════════════
//  Ring 3 — Userland entry
// ═══════════════════════════════════════════════════════════════════

pub mod ring3;

// ═══════════════════════════════════════════════════════════════════
//  Re-exports — legacy crate::<mod> paths
//
//  ring0/mod.rs was originally the crate root. All code uses paths
//  like crate::arch, crate::mm, etc. We re-export for backward
//  compatibility (~293 files).
//
//  TODO(v2.0): migrate to explicit crate::ring0::* paths, delete
//  this block. Internal modules (arch, mm, dev, cpu, proc) first.
// ═══════════════════════════════════════════════════════════════════

// ── Core ring0 modules ─────────────────────────────────────────
pub use ring0::{arch, mm, dev, proc, cpu};
pub use ring0::{info, context, uefi_rt, serial, log};
pub use ring0::{boot_phase, entry, mod_loader};

// ── Crates (pure Ring 0) ───────────────────────────────────────
pub use cpu_vendor_profile as vendor;


