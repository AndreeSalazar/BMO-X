//! `barex::compat` — L4 capa de compatibilidad para binarios Windows.
//!
//! Spec: `BareX_Compat_Shim_Spec.md`. PE loader + COM thunks para
//! `d3d9/10/11/12.dll`, `dxgi.dll`, `xinput`, `xaudio2`, `winsock`, etc.
//! Vive en Ring 3 dentro de un sandbox BEF — este módulo del kernel sólo
//! provee los *hooks* de carga PE y la tabla de syscalls NT mapeadas.
//!
//! ⚠️ Nada del shim Win32 entra en Ring 0. Aquí sólo declaramos los tipos
//! que el loader BEF necesita para reconocer un binario Windows.

use crate::barex::{BxError, BxResult};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BinaryKind {
    /// BEF nativo de FastOS.
    Bef,
    /// PE64 de Windows (game.exe). Requiere shim L4 en Ring 3.
    PeWindows64,
    /// ELF binario nativo Linux.
    Elf,
    /// No se reconoció el formato.
    Unknown,
}

#[derive(Debug, Clone, Copy)]
pub struct PeImport {
    pub dll_name_hash: u32,
    pub function_name_hash: u32,
}

/// Tabla de DLLs falsas que el loader Ring 3 debe proveer cuando carga PE.
pub const FAKE_DLLS: &[&str] = &[
    "d3d9.dll",
    "d3d10.dll",
    "d3d11.dll",
    "d3d12.dll",
    "dxgi.dll",
    "xinput1_4.dll",
    "xaudio2_9.dll",
    "ws2_32.dll",
    "winhttp.dll",
    "kernel32.dll",
    "user32.dll",
    "ntdll.dll",
];

pub fn detect_binary_kind(bytes: &[u8]) -> BxResult<BinaryKind> {
    if bytes.len() >= 4 && bytes[0..4] == *b"BEF\0" {
        return Ok(BinaryKind::Bef);
    }
    if bytes.len() >= 0x40 {
        let lfanew = u32::from_le_bytes([bytes[0x3C], bytes[0x3D], bytes[0x3E], bytes[0x3F]]) as usize;
        if lfanew + 4 <= bytes.len()
            && bytes[lfanew..lfanew + 4] == *b"PE\0\0"
        {
            return Ok(BinaryKind::PeWindows64);
        }
    }
    if bytes.len() >= 4 && bytes[0..4] == *b"\x7fELF" {
        return Ok(BinaryKind::Elf);
    }
    Ok(BinaryKind::Unknown)
}
