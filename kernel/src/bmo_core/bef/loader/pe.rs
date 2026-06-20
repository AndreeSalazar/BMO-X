//! ⭐ DEVOUR PE — loader que come binarios Windows (.exe / .dll).
//!
//! Lee el formato PE/COFF de Microsoft y produce una `Image` BEF con
//! `format = BinaryFormat::PeDevoured`. Las secciones PE (`.text`, `.data`,
//! `.rdata`, `.rsrc`, etc.) se mapean a `SectionKind` BEF, los imports a
//! fake-DLLs (`d3d12.dll` → BareX, `xinput1_4.dll` → bx_input, etc.).

#![allow(dead_code)]

use super::{Image, LoadError, MappedSection, fake_provenance_image};
use crate::bmo_core::bef::manifest::Provenance;
use crate::bmo_core::bmo_abi::primitives::{bx_u16, bx_u32, bx_u64};

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
    pub size_of_image: bx_u32,
    pub size_of_headers: bx_u32,
    pub checksum: bx_u32,
    pub subsystem: bx_u16,
    pub dll_characteristics: bx_u16,
    pub number_of_rva_and_sizes: bx_u32,
    // DataDirectory[16] follows but we don't need all of them.
}

/// DataDirectory entry — 8 bytes.
#[repr(C, packed)]
#[derive(Debug, Clone, Copy)]
pub struct DataDirectory {
    pub virtual_address: bx_u32,
    pub size: bx_u32,
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
///
/// v1.3.0: reducido a una lista informativa de strings. Los nombres
/// de módulos (`barex::compat::dxvk11`, etc.) son solo **etiquetas**
/// — el loader no los usa como paths Rust, solo los imprime en
/// logs. La redirección real sucede en `pe_thunks.rs` (en el mismo
/// directorio) donde cada import se mapea a una función real.
pub const FAKE_DLLS: &[&str] = &[
    "d3d12.dll",
    "d3d11.dll",
    "d3d9.dll",
    "dxgi.dll",
    "xinput1_4.dll",
    "xaudio2_9.dll",
    "ws2_32.dll",
    "winhttp.dll",
    "kernel32.dll",
    "user32.dll",
    "ntdll.dll",
];

/// PE relocation type: IMAGE_REL_BASED_DIR64.
const IMAGE_REL_BASED_DIR64: u16 = 10;

/// PE data directory indices.
const IMAGE_DIRECTORY_ENTRY_IMPORT: usize = 1;
const IMAGE_DIRECTORY_ENTRY_BASERELOC: usize = 5;

pub fn load(bytes: &[u8]) -> Result<Image, LoadError> {
    if bytes.len() < core::mem::size_of::<DosHeader>() {
        return Err(LoadError::Truncated);
    }
    let dos = unsafe { &*(bytes.as_ptr() as *const DosHeader) };
    let dos_magic = dos.e_magic;
    if dos_magic != DOS_MAGIC {
        return Err(LoadError::InvalidHeader);
    }

    // Lectura del PE header en e_lfanew.
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

    let opt_off = pe_off + core::mem::size_of::<CoffFileHeader>();
    let opt_size = coff.size_of_optional_header as usize;
    if opt_off + core::mem::size_of::<OptionalHeader64>() > bytes.len() {
        return Err(LoadError::Truncated);
    }
    let opt = unsafe { &*(bytes.as_ptr().add(opt_off) as *const OptionalHeader64) };
    let entry = opt.address_of_entry_point as u64;
    let base  = opt.image_base;

    // Parse DataDirectory if present.
    let dd_off = opt_off + core::mem::size_of::<OptionalHeader64>();
    let dd_count = opt.number_of_rva_and_sizes as usize;
    let data_dirs = if dd_off + dd_count * 8 <= bytes.len() && dd_count > 0 {
        unsafe {
            core::slice::from_raw_parts(
                bytes.as_ptr().add(dd_off) as *const DataDirectory,
                dd_count.min(16),
            )
        }
    } else {
        &[]
    };

    // ─── Iterar section headers ────────────────────────────────────────
    let sections_off = opt_off + opt_size;
    let n_sections = coff.number_of_sections as usize;
    let needed = n_sections * core::mem::size_of::<PeSectionHeader>();
    if sections_off + needed > bytes.len() {
        return Err(LoadError::Truncated);
    }
    let sec_ptr = unsafe { bytes.as_ptr().add(sections_off) as *const PeSectionHeader };
    let sections = unsafe { core::slice::from_raw_parts(sec_ptr, n_sections) };

    let mut img = fake_provenance_image(Provenance::PeDevoured);
    img.entry_point = base.wrapping_add(entry);
    img.base_address = base;

    // Mapear cada sección PE a una `MappedSection` BEF con datos reales.
    for s in sections {
        let va = s.virtual_address as u64;
        let vsz = s.virtual_size as u64;
        let chr = s.characteristics;
        let mut flags = 0u32;
        if chr & 0x4000_0000 != 0 { flags |= 0x1; }   // R
        if chr & 0x8000_0000 != 0 { flags |= 0x2; }   // W
        if chr & 0x2000_0000 != 0 { flags |= 0x4; }   // X

        let kind = pick_section_kind(&s.name, chr);

        // Allocate and copy section data.
        let raw_data_size = s.size_of_raw_data as usize;
        let virt_size = s.virtual_size as usize;
        let alloc_size = virt_size.max(raw_data_size);
        let align = 4096usize;
        let aligned_size = (alloc_size + align - 1) & !(align - 1);

        let data_ptr = if aligned_size > 0 {
            let layout = core::alloc::Layout::from_size_align(aligned_size, align)
                .map_err(|_| LoadError::SectionOutOfRange)?;
            let ptr = unsafe { alloc::alloc::alloc_zeroed(layout) };
            if ptr.is_null() {
                return Err(LoadError::SectionOutOfRange);
            }

            // Copy raw data from file.
            if raw_data_size > 0 && s.pointer_to_raw_data as usize + raw_data_size <= bytes.len() {
                let src = &bytes[s.pointer_to_raw_data as usize..s.pointer_to_raw_data as usize + raw_data_size];
                unsafe { core::ptr::copy_nonoverlapping(src.as_ptr(), ptr, raw_data_size); }
            }

            ptr as u64
        } else {
            0
        };

        img.sections.push(MappedSection {
            kind,
            virt_addr: base.wrapping_add(va),
            size: vsz.max(raw_data_size as u64),
            flags,
            data_ptr,
        });
    }

    // ─── Apply PE relocations (IMAGE_REL_BASED_DIR64) ─────────────────
    if data_dirs.len() > IMAGE_DIRECTORY_ENTRY_BASERELOC {
        let reloc_dir = &data_dirs[IMAGE_DIRECTORY_ENTRY_BASERELOC];
        if reloc_dir.virtual_address != 0 && reloc_dir.size != 0 {
            apply_pe_relocations(bytes, &mut img, reloc_dir, base)?;
        }
    }

    // ─── Walk PE Import Directory ──────────────────────────────────────
    if data_dirs.len() > IMAGE_DIRECTORY_ENTRY_IMPORT {
        let import_dir = &data_dirs[IMAGE_DIRECTORY_ENTRY_IMPORT];
        if import_dir.virtual_address != 0 && import_dir.size != 0 {
            resolve_pe_imports(bytes, &img, sections, import_dir, base)?;
        }
    }

    Ok(img)
}

/// Apply PE base relocations (IMAGE_REL_BASED_DIR64).
fn apply_pe_relocations(
    bytes: &[u8],
    img: &mut Image,
    reloc_dir: &DataDirectory,
    base: u64,
) -> Result<(), LoadError> {
    let reloc_rva = reloc_dir.virtual_address;
    let reloc_size = reloc_dir.size as usize;

    // Find the file offset for the reloc RVA.
    let reloc_file_offset = rva_to_file_offset_pe(reloc_rva, &[]).ok_or(LoadError::SectionOutOfRange)?;
    if reloc_file_offset + reloc_size > bytes.len() {
        return Err(LoadError::Truncated);
    }

    // Parse PE relocation blocks.
    let mut pos = reloc_file_offset;
    let reloc_end = reloc_file_offset + reloc_size;
    while pos + 8 <= reloc_end {
        let block_rva = u32::from_le_bytes([
            bytes[pos], bytes[pos+1], bytes[pos+2], bytes[pos+3],
        ]);
        let block_size = u32::from_le_bytes([
            bytes[pos+4], bytes[pos+5], bytes[pos+6], bytes[pos+7],
        ]) as usize;
        if block_size < 8 || block_size % 4 != 0 {
            break;
        }

        let entries = (block_size - 8) / 2;
        for i in 0..entries {
            let entry_off = pos + 8 + i * 2;
            if entry_off + 2 > reloc_end { break; }
            let entry = u16::from_le_bytes([bytes[entry_off], bytes[entry_off + 1]]);
            let reloc_type = entry >> 12;
            let reloc_offset = (entry & 0x0FFF) as u64;

            if reloc_type == IMAGE_REL_BASED_DIR64 {
                let target_rva = block_rva as u64 + reloc_offset;
                // Find the section containing this RVA.
                for section in &img.sections {
                    let sec_rva = section.virt_addr - img.base_address;
                    if target_rva >= sec_rva && target_rva < sec_rva + section.size {
                        let offset_in_section = (target_rva - sec_rva) as usize;
                        if section.data_ptr != 0 && offset_in_section + 8 <= section.size as usize {
                            unsafe {
                                let ptr = (section.data_ptr as *mut u64).add(offset_in_section / 8);
                                let old_val = *ptr;
                                let delta = base - section.virt_addr + section.virt_addr; // base - original_base
                                *ptr = old_val.wrapping_add(delta);
                            }
                        }
                        break;
                    }
                }
            }
        }
        pos += block_size;
    }

    Ok(())
}

/// Walk PE Import Directory and register resolved imports.
fn resolve_pe_imports(
    bytes: &[u8],
    _img: &Image,
    sections: &[PeSectionHeader],
    import_dir: &DataDirectory,
    base: u64,
) -> Result<(), LoadError> {
    let import_rva = import_dir.virtual_address;
    let import_size = import_dir.size as usize;

    let import_file_offset = rva_to_file_offset_pe(import_rva, sections)
        .ok_or(LoadError::SectionOutOfRange)?;
    if import_file_offset + import_size > bytes.len() {
        return Err(LoadError::Truncated);
    }

    let desc_size = core::mem::size_of::<super::pe_imports::ImageImportDescriptor>();
    let mut pos = import_file_offset;

    loop {
        if pos + desc_size > bytes.len() { break; }
        let desc = unsafe {
            &*(bytes.as_ptr().add(pos) as *const super::pe_imports::ImageImportDescriptor)
        };
        if desc.is_terminator() { break; }

        // Get DLL name.
        let dll_name = if desc.name_rva != 0 {
            let dll_offset = rva_to_file_offset_pe(desc.name_rva, sections);
            if let Some(off) = dll_offset {
                super::pe_imports::read_cstr(bytes, off, 256).unwrap_or("???")
            } else {
                "???"
            }
        } else {
            "???"
        };

        // Walk INT (OriginalFirstThunk) to get function names.
        if desc.original_first_thunk != 0 {
            let int_offset = rva_to_file_offset_pe(desc.original_first_thunk, sections);
            if let Some(int_off) = int_offset {
                let iat_offset = rva_to_file_offset_pe(desc.first_thunk_iat, sections);
                if let Some(iat_off) = iat_offset {
                    walk_pe_import_thunks(bytes, dll_name, int_off, iat_off, sections, base);
                }
            }
        }

        pos += desc_size;
    }

    Ok(())
}

/// Walk PE import thunks and register each resolved symbol.
fn walk_pe_import_thunks(
    bytes: &[u8],
    dll_name: &str,
    int_file_offset: usize,
    iat_file_offset: usize,
    sections: &[PeSectionHeader],
    _base: u64,
) {
    let thunk_size = core::mem::size_of::<super::pe_imports::ImageThunk>();
    let mut i = 0;

    loop {
        let int_pos = int_file_offset + i * thunk_size;
        let iat_pos = iat_file_offset + i * thunk_size;
        if int_pos + thunk_size > bytes.len() || iat_pos + thunk_size > bytes.len() {
            break;
        }

        let int_thunk = super::pe_imports::ImageThunk(unsafe {
            let mut buf = [0u8; 8];
            core::ptr::copy_nonoverlapping(bytes.as_ptr().add(int_pos), buf.as_mut_ptr(), 8);
            u64::from_le_bytes(buf)
        });
        let _iat_thunk = super::pe_imports::ImageThunk(unsafe {
            let mut buf = [0u8; 8];
            core::ptr::copy_nonoverlapping(bytes.as_ptr().add(iat_pos), buf.as_mut_ptr(), 8);
            u64::from_le_bytes(buf)
        });

        if int_thunk.is_terminator() { break; }

        let fn_name = if let Some(name_rva) = int_thunk.name_rva() {
            let name_offset = rva_to_file_offset_pe(name_rva, sections);
            if let Some(off) = name_offset {
                // IMAGE_IMPORT_BY_NAME: skip 2-byte Hint, read name.
                let name_start = off + 2;
                super::pe_imports::read_cstr(bytes, name_start, 256).unwrap_or("???")
            } else {
                "???"
            }
        } else if let Some(ordinal) = int_thunk.ordinal() {
            // Import by ordinal — use ordinal as name.
            static ORD_BUF: [u8; 8] = [0; 8]; // Can't return static in no_std easily
            let _ = ordinal;
            "ordinal"
        } else {
            "???"
        };

        // Resolve via PE thunk table — now returns real function pointers.
        let (target, fn_ptr) = super::pe_thunks::resolve_fn(dll_name, fn_name);
        let addr = match target {
            super::pe_thunks::ThunkTarget::SilentStub => super::pe_thunks::silent_stub as *const () as u64,
            super::pe_thunks::ThunkTarget::LogStub => super::pe_thunks::log_stub as *const () as u64,
            _ => fn_ptr, // Real bmo_abi::interop::win32 function pointer
        };

        // Register in runtime symbol table.
        // Use a static string for the DLL name — leaks but acceptable for kernel.
        let static_dll: &'static str = leak_str(dll_name);
        let static_fn: &'static str = leak_str(fn_name);
        super::runtime::register_symbol(static_dll, static_fn, addr,
            super::runtime::SYM_PE_THUNK | super::runtime::SYM_EAGER);

        i += 1;
    }
}

/// Leak a string into a &'static str (acceptable in kernel context).
fn leak_str(s: &str) -> &'static str {
    let len = s.len();
    let layout = core::alloc::Layout::from_size_align(len, 1).unwrap();
    let ptr = unsafe { alloc::alloc::alloc(layout) };
    if ptr.is_null() {
        return "";
    }
    unsafe {
        core::ptr::copy_nonoverlapping(s.as_ptr(), ptr, len);
        core::str::from_utf8_unchecked(core::slice::from_raw_parts(ptr, len))
    }
}

/// Convert RVA to file offset using section headers.
fn rva_to_file_offset_pe(rva: u32, sections: &[PeSectionHeader]) -> Option<usize> {
    for s in sections {
        let va = s.virtual_address;
        let vsz = s.virtual_size;
        if rva >= va && rva < va.saturating_add(vsz) {
            let delta = rva - va;
            return Some((s.pointer_to_raw_data + delta) as usize);
        }
    }
    None
}

/// Elige un `SectionKind` BEF para una sección PE.
fn pick_section_kind(name: &[u8; 8], chr: u32) -> u8 {
    use crate::bmo_core::bef::sections::SectionKind;
    let n = core::str::from_utf8(name).unwrap_or("");
    if n.starts_with(".text") || (chr & 0x2000_0000) != 0 { return SectionKind::Code as u8; }
    if n.starts_with(".rdata") || n.starts_with(".rodata") { return SectionKind::RoData as u8; }
    if n.starts_with(".data") { return SectionKind::Data as u8; }
    if n.starts_with(".bss") { return SectionKind::Bss as u8; }
    if n.starts_with(".idata") { return SectionKind::Imports as u8; }
    if n.starts_with(".edata") { return SectionKind::Exports as u8; }
    if n.starts_with(".reloc") { return SectionKind::Relocs as u8; }
    if n.starts_with(".rsrc") { return SectionKind::Resources as u8; }
    if n.starts_with(".tls") { return SectionKind::Tls as u8; }
    if n.starts_with(".pdata") { return SectionKind::Unwind as u8; }
    if n.starts_with(".debug") { return SectionKind::Debug as u8; }
    SectionKind::Data as u8
}
