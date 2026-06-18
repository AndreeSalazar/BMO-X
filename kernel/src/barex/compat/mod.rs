//! `barex::compat` — compatibility layer hooks (L4).
//!
//! Per the BareX spec, the heavy lifting of Win32 compatibility lives
//! in Ring 3 BEF shims. The kernel (Ring 0) only needs to know how to
//! detect binary formats and route them to the right loader.
//!
//! This module is now a **thin re-export** of the format registry in
//! `bmo_abi::interop::format`. The legacy `BinaryKind` enum and the
//! `FAKE_DLLS` list have been consolidated there.
//!
//! Kept here for backwards compatibility with any code that still
//! imports `crate::barex::compat::BinaryKind` etc.

#![allow(deprecated)]
#![allow(dead_code)]

pub use crate::bmo_abi::interop::format::{
    BinaryKind,
    detect_binary_kind,
};

/// DLLs that a Win32 PE binary may import and that the Ring 3 shim
/// (a BEF binary) must provide. This is the *contract* between the
/// BMO PE loader and the userland shim — it is **not** something the
/// kernel implements.
pub const FAKE_DLLS: &[&str] = &[
    // DirectX
    "d3d9.dll",
    "d3d10.dll",
    "d3d11.dll",
    "d3d12.dll",
    "dxgi.dll",
    // XInput / XAudio (game input + audio)
    "xinput1_4.dll",
    "xaudio2_9.dll",
    // Winsock
    "ws2_32.dll",
    "winhttp.dll",
    // Win32 core (handled natively by `bmo_abi::interop::win32`)
    "kernel32.dll",
    "ntdll.dll",
    // Win32 GUI (NOT handled by BMO; shim may provide or stub)
    "user32.dll",
    "gdi32.dll",
];

/// A Win32 PE import entry (used by the BMO PE loader in Ring 3).
#[derive(Debug, Clone, Copy)]
pub struct PeImport {
    pub dll_name_hash: u32,
    pub function_name_hash: u32,
}
