//! BMO GPU Shims — compatibility with external binary formats.
//!
//! v1.7.9: thin shim registry. The actual Win32/PE/ELF translation
//! happens in Ring 3 (BMO BEF shims). The kernel (Ring 0) only
//! detects the format and routes to the right loader.

#![allow(dead_code)]

pub mod pe_imports;
pub mod pe_thunks;

/// DLLs that a Win32 PE binary may import and that the Ring 3 shim
/// (a BEF binary) must provide. This is the *contract* between the
/// BMO PE loader and the userland shim — it is **not** something the
/// kernel implements.
pub const FAKE_DLLS: &[&str] = &[
    // DirectX
    "d3d9.dll",  "d3d10.dll", "d3d11.dll", "d3d12.dll", "dxgi.dll",
    // XInput / XAudio
    "xinput1_4.dll", "xaudio2_9.dll",
    // Winsock
    "ws2_32.dll", "winhttp.dll",
    // Win32 core
    "kernel32.dll", "ntdll.dll",
    // Win32 GUI (shim may provide or stub)
    "user32.dll", "gdi32.dll",
];

/// A Win32 PE import entry.
#[derive(Debug, Clone, Copy)]
pub struct PeImport {
    pub dll_name_hash: u32,
    pub function_name_hash: u32,
}
