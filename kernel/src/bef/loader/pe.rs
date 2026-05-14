//! ⭐ DEVOUR PE — loader que come binarios Windows (.exe / .dll).
//!
//! Lee el formato PE/COFF de Microsoft y produce una `Image` BEF con
//! `format = BinaryFormat::PeDevoured`. Las secciones PE (`.text`, `.data`,
//! `.rdata`, `.rsrc`, etc.) se mapean a `SectionKind` BEF, los imports a
//! fake-DLLs (`d3d12.dll` → BareX, `xinput1_4.dll` → bx_input, etc.).

#![allow(dead_code)]

use super::{Image, LoadError, fake_provenance_image};
use crate::bef::manifest::Provenance;
use crate::barex::abi::primitives::{bx_u16, bx_u32, bx_u64};

// ─── DOS Header (64 bytes) ─────────────────────────────────────────────
#[repr(C, packed)]
#[derive(Debug, Clone, Copy)]
pub struct DosHeader {
    pub e_magic: bx_u16,    // 'MZ' (0x5A4D)
    pub e_cblp: bx_u16,
    pub e_cp: bx_u16,
    pub e_crlc: bx_u16,
    pub e_cparhdr: bx_u16,
    pub e_minalloc: bx_u16,
    pub e_maxalloc: bx_u16,
    pub e_ss: bx_u16,
    pub e_sp: bx_u16,
    pub e_csum: bx_u16,
    pub e_ip: bx_u16,
    pub e_cs: bx_u16,
    pub e_lfarlc: bx_u16,
    pub e_ovno: bx_u16,
    pub e_res: [bx_u16; 4],
    pub e_oemid: bx_u16,
    pub e_oeminfo: bx_u16,
    pub e_res2: [bx_u16; 10],
    pub e_lfanew: bx_u32,   // offset al PE header
}

pub const DOS_MAGIC: bx_u16 = 0x5A4D;
pub const PE_MAGIC: bx_u32  = 0x0000_4550;  // "PE\0\0"
pub const PE_MACHINE_AMD64: bx_u16 = 0x8664;

// ─── COFF File Header (24 bytes) ───────────────────────────────────────
#[repr(C, packed)]
#[derive(Debug, Clone, Copy)]
pub struct CoffFileHeader {
    pub signature: bx_u32,
    pub machine: bx_u16,
    pub number_of_sections: bx_u16,
    pub time_date_stamp: bx_u32,
    pub pointer_to_symbol_table: bx_u32,
    pub number_of_symbols: bx_u32,
    pub size_of_optional_header: bx_u16,
    pub characteristics: bx_u16,
}

// ─── Optional Header PE32+ (subset que usamos) ─────────────────────────
#[repr(C, packed)]
#[derive(Debug, Clone, Copy)]
pub struct OptionalHeader64 {
    pub magic: bx_u16,                 // 0x20B = PE32+
    pub major_linker_version: u8,
    pub minor_linker_version: u8,
    pub size_of_code: bx_u32,
    pub size_of_initialized_data: bx_u32,
    pub size_of_uninitialized_data: bx_u32,
    pub address_of_entry_point: bx_u32,
    pub base_of_code: bx_u32,
    pub image_base: bx_u64,
    pub section_alignment: bx_u32,
    pub file_alignment: bx_u32,
    // (resto omitido — no necesario para el devour mínimo)
}

// ─── Section Header (40 bytes) ─────────────────────────────────────────
#[repr(C, packed)]
#[derive(Debug, Clone, Copy)]
pub struct PeSectionHeader {
    pub name: [u8; 8],                 // ej. ".text\0\0\0"
    pub virtual_size: bx_u32,
    pub virtual_address: bx_u32,
    pub size_of_raw_data: bx_u32,
    pub pointer_to_raw_data: bx_u32,
    pub pointer_to_relocations: bx_u32,
    pub pointer_to_linenumbers: bx_u32,
    pub number_of_relocations: bx_u16,
    pub number_of_linenumbers: bx_u16,
    pub characteristics: bx_u32,
}

/// DLL falsas que el devour-loader provee a los binarios PE.
/// Cuando el PE importa de aquí, lo resolvemos a la API BareX.
pub const FAKE_DLLS_TO_BAREX: &[(&str, &str)] = &[
    ("d3d12.dll",       "barex::graphics"),
    ("d3d11.dll",       "barex::compat::dxvk11"),
    ("d3d9.dll",        "barex::compat::dxvk9"),
    ("dxgi.dll",        "barex::graphics::swapchain"),
    ("xinput1_4.dll",   "barex::input"),
    ("xaudio2_9.dll",   "barex::audio"),
    ("ws2_32.dll",      "barex::net"),
    ("winhttp.dll",     "barex::net::http"),
    ("kernel32.dll",    "barex::compat::kernel32_stub"),
    ("user32.dll",      "barex::compat::user32_stub"),
    ("ntdll.dll",       "syscall::dispatch"),
];

pub fn load(bytes: &[u8]) -> Result<Image, LoadError> {
    if bytes.len() < core::mem::size_of::<DosHeader>() {
        return Err(LoadError::Truncated);
    }
    let dos = unsafe { &*(bytes.as_ptr() as *const DosHeader) };
    let dos_magic = dos.e_magic;
    if dos_magic != DOS_MAGIC {
        return Err(LoadError::InvalidHeader);
    }

    // Lectura del PE header en e_lfanew. Importante: campos packed
    // requieren copia local antes de comparar.
    let pe_off = dos.e_lfanew as usize;
    if pe_off + core::mem::size_of::<CoffFileHeader>() > bytes.len() {
        return Err(LoadError::Truncated);
    }
    let coff = unsafe { &*(bytes.as_ptr().add(pe_off) as *const CoffFileHeader) };
    let sig = coff.signature;
    if sig != PE_MAGIC {
        return Err(LoadError::InvalidHeader);
    }
    let machine = coff.machine;
    if machine != PE_MACHINE_AMD64 {
        return Err(LoadError::UnsupportedArch);
    }

    // TODO: parsear OptionalHeader64, mapear secciones, parsear imports
    // (`IMAGE_DIRECTORY_ENTRY_IMPORT`), aplicar relocs `IMAGE_REL_BASED_DIR64`,
    // sintetizar Manifest con `Provenance::PeDevoured`.
    let opt_off = pe_off + core::mem::size_of::<CoffFileHeader>();
    if opt_off + core::mem::size_of::<OptionalHeader64>() > bytes.len() {
        return Err(LoadError::Truncated);
    }
    let opt = unsafe { &*(bytes.as_ptr().add(opt_off) as *const OptionalHeader64) };
    let entry = opt.address_of_entry_point as u64;
    let base  = opt.image_base;

    let mut img = fake_provenance_image(Provenance::PeDevoured);
    img.entry_point = base.wrapping_add(entry);
    img.base_address = base;
    Ok(img)
}
