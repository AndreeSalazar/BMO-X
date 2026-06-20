//! Binary format registry — BMO's equivalent of Linux `binfmt_misc`.
//!
//! Linux lets users register arbitrary binary formats at runtime by writing
//! lines to `/proc/sys/fs/binfmt_misc/register`. BMO does the same in a
//! compile-time registry: any subsystem that wants to claim a binary
//! format adds an entry to [`FORMAT_REGISTRY`] and the [`detect`] function
//! will route the file to the right handler.
//!
//! ## Format
//!
//! Each entry is `(magic_bytes, offset, handler)`:
//! - `magic_bytes` — the bytes that identify the format at `offset` from
//!   the start of the file. For PE this is `"MZ"` at offset 0; for ELF it
//!   is `"\x7fELF"` at offset 0; for BEF it is `"BEF\0"` at offset 0.
//! - `offset` — usually 0; PE's PE-header is at `lfanew` (offset 0x3C),
//!   but the magic `"MZ"` is always at offset 0.
//! - `handler` — a string identifying the loader:
//!   - `"bmo_native"`     — BMO native executable (BEF magic).
//!   - `"win32_pe_loader"` — Windows PE; needs BMO PE loader + Win32 shim.
//!   - `"elf_loader"`     — ELF binary; needs ELF→BEF translator.
//!   - `"bsf_shader"`     — BareX shader (BSF); goes to shader pipeline.
//!
//! ## BMO PE loader flow (analogous to Proton's)
//!
//! ```text
//! PE file (DOS MZ + PE\0\0)
//!   ↓ BMO PE loader (parses imports/exports)
//!   ↓ BEF shim (Ring 3 ELF) provides d3d11.dll, user32.dll, etc.
//!   ↓ thunks call bmo_abi::interop::win32::{ntdll, kernel32}_*
//!   ↓ BMO syscalls execute natively
//! ```
//!
//! The shim DLLs are themselves BMO native binaries (BEF format) that
//! expose the Win32 API surface. The kernel just needs to know which
//! shim binary corresponds to each `dll` import.

use crate::bmo_core::barex::BxResult;

/// Loader kind for a detected binary format.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BinaryKind {
    /// BMO native BEF binary — runs directly.
    Bef,
    /// Windows PE binary — needs PE loader + Win32 shim.
    PeWindows64,
    /// ELF binary — needs ELF→BEF translator.
    Elf,
    /// BareX Shader Format — goes to shader pipeline.
    BsfShader,
    /// Format not recognized.
    Unknown,
}

/// Loader identifier string (used by the loader dispatcher).
pub type Loader = &'static str;

/// One entry in the format registry.
pub struct FormatEntry {
    /// Magic bytes at `offset` that identify the format.
    pub magic: &'static [u8],
    /// Offset of the magic from the start of the file.
    pub offset: usize,
    /// The kind of binary this magic identifies.
    pub kind: BinaryKind,
    /// The loader responsible for handling this format.
    pub loader: Loader,
}

/// BMO format registry. Modelled on Linux's `binfmt_misc` table.
///
/// First match wins. Keep magic bytes in **decreasing specificity** order:
/// the most specific magic (longest, most distinctive) goes first.
pub static FORMAT_REGISTRY: &[FormatEntry] = &[
    // BEF: 4 bytes, "BEF\0" — unambiguous, native.
    FormatEntry { magic: b"BEF\0",  offset: 0, kind: BinaryKind::Bef,         loader: "bmo_native" },

    // BSF: 4 bytes, "BSF\0" — BareX Shader Format.
    FormatEntry { magic: b"BSF\0",  offset: 0, kind: BinaryKind::BsfShader,   loader: "bsf_shader_pipeline" },

    // ELF: 4 bytes, "\x7fELF" — needs translator.
    FormatEntry { magic: b"\x7fELF", offset: 0, kind: BinaryKind::Elf,         loader: "elf_loader" },

    // PE: 2 bytes, "MZ" at offset 0 (PE header is at 0x3C but DOS MZ is at 0).
    // The full PE check is "MZ" at 0 AND "PE\0\0" at the lfanew offset;
    // the dispatcher does that extra check below.
    FormatEntry { magic: b"MZ",     offset: 0, kind: BinaryKind::PeWindows64,  loader: "win32_pe_loader" },
];

/// Detect the binary kind of the given bytes by walking the format registry.
///
/// Returns `BinaryKind::Unknown` if no entry matches.
pub fn detect(bytes: &[u8]) -> BinaryKind {
    for entry in FORMAT_REGISTRY {
        if entry.offset + entry.magic.len() <= bytes.len()
            && &bytes[entry.offset..entry.offset + entry.magic.len()] == entry.magic
        {
            // PE magic is ambiguous ("MZ" is also DOS .COM). Verify
            // the PE header is present at `lfanew` to confirm.
            if entry.kind == BinaryKind::PeWindows64 {
                if !verify_pe(bytes) {
                    continue;
                }
            }
            return entry.kind;
        }
    }
    BinaryKind::Unknown
}

/// Get the loader string for a given binary kind.
pub fn loader_for(kind: BinaryKind) -> Loader {
    for entry in FORMAT_REGISTRY {
        if entry.kind == kind {
            return entry.loader;
        }
    }
    "unknown"
}

/// Verify a PE binary by checking that the PE header is at `lfanew`.
///
/// `lfanew` is the 4-byte little-endian value at offset `0x3C` of a PE
/// file. At that offset there must be the bytes `"PE\0\0"`.
fn verify_pe(bytes: &[u8]) -> bool {
    if bytes.len() < 0x40 { return false; }
    let lfanew = u32::from_le_bytes([bytes[0x3C], bytes[0x3D], bytes[0x3E], bytes[0x3F]]) as usize;
    lfanew + 4 <= bytes.len() && bytes[lfanew..lfanew + 4] == *b"PE\0\0"
}

/// Backwards-compatible API: detect_binary_kind returns a BxResult.
pub fn detect_binary_kind(bytes: &[u8]) -> BxResult<BinaryKind> {
    Ok(detect(bytes))
}

/// Initialize the format registry (called from `bmo_abi::interop::init`).
pub fn init() {
    crate::bmo_core::diag::info_u64("bmo_abi::interop::format", "FORMAT_REGISTRY entries",
        FORMAT_REGISTRY.len() as u64);
}
