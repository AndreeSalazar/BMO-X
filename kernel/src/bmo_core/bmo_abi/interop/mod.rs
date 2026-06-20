//! BMO interop — how BMO speaks with other ABIs and devours foreign formats.
//!
//! This module is the BMO equivalent of Linux's binfmt_misc subsystem:
//! it registers handlers for binary formats and NT-style syscalls that BMO
//! can understand. The implementations are intentionally thin: BMO
//! integrates the **mechanism** (registry, dispatch) and delegates the
//! **implementation** (shims, translators) to Ring 3 ELF/BEF binaries.
//!
//! ## Submodules
//!
//! - [`win32`]   — NT syscalls + kernel32 wrappers. The minimum BMO needs
//!                to load a Windows PE binary that only uses memory, threads,
//!                processes, and files. No GUI, no GDI, no COM.
//! - [`format`]  — Binary format registry (BEF, PE, ELF, BSF). Modelled
//!                after Linux's `/proc/sys/fs/binfmt_misc`.
//! - [`lang_bridge`] — Language tag registry for cross-language calls.
//! - [`marshal`]     — Type marshallers for cross-language calls.
//! - [`compat`]      — Thunks between Win64/SysV ABIs and BMO ABI.
//!
//! ## What is NOT here
//!
//! - Win32 GUI (USER32, GDI32)        → not integrated; BMO has its own UI.
//! - C runtime (msvcrt)                → not integrated; BMO apps use BMO CRT.
//! - Win32 misc (shell, advapi, COM)  → not integrated; Ring 3 shim only.
//!
//! Those belong in Ring 3 BEF shims and are not part of the kernel ABI.

#![allow(dead_code)]

pub mod compat;
pub mod format;
pub mod lang_bridge;
pub mod marshal;
pub mod win32;

/// Initialize all BMO interop surfaces.
pub fn init() {
    crate::bmo_core::diag::info("bmo_abi::interop", "BMO interop surface initialized");
    format::init();
    win32::init();
}
