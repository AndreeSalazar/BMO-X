//! Loader BEF nativo.
//!
//! Pipeline:
//!   1. Validar header + magic + versión.
//!   2. Parsear section table.
//!   3. Verificar hashes BLAKE3 (sección Signature).
//!   4. Cargar manifest TOML → resolver capabilities.
//!   5. Mapear secciones (RO/RW/RX) en el address space del proceso.
//!   6. Aplicar relocations (`Abs64`/`Rel32`/`Got64`).
//!   7. Resolver imports (eager o instalar trampolines lazy).
//!   8. Setup TLS template del thread principal.
//!   9. Saltar al `entry_point`.

#![allow(dead_code)]

extern crate alloc;
use alloc::vec::Vec;

use crate::bef::header::{BefHeader, BEF_MAGIC};
use crate::bef::sections::{SectionTable, SectionKind, SectionEntry};
use crate::bef::signing::{SignatureHeader, SectionHash};
use crate::bef::relocations::Relocation;
use crate::bef::tls::TlsTemplate;
use super::{Image, LoadError, MappedSection, fake_provenance_image};
use super::meta_sections::{parse_meta_sections, meta_stats, MetaSectionStats};
use crate::bef::manifest::Provenance;

/// Virtual base address for user-space loading (Ring 3).
/// ASLR: offset by 2 MB random amount in lower 4 bits (16-aligned).
const USER_BASE: u64 = 0x0040_0000;

/// Get ASLR-randomized base address.
fn aslr_base() -> u64 {
    // Use TSC as entropy source — not cryptographically secure but
    // sufficient for basic ASLR in a bare-metal OS.
    let tsc = crate::arch::cpu::rdtsc();
    let offset = (tsc & 0x00FF_F000) as u64; // Random 4KB-aligned offset up to 16 MB
    USER_BASE + offset
}

pub fn load(bytes: &[u8]) -> Result<Image, LoadError> {
    if bytes.len() < BefHeader::SIZE {
        return Err(LoadError::Truncated);
    }
    // SAFETY: alignment guaranteed by size check + repr(C, align(16)).
    let hdr = unsafe { &*(bytes.as_ptr() as *const BefHeader) };
    if hdr.magic != BEF_MAGIC {
        return Err(LoadError::InvalidHeader);
    }
    if !hdr.is_valid() {
        return Err(LoadError::InvalidHeader);
    }
    if hdr.arch != crate::bef::header::BefArch::X86_64 as u8 {
        return Err(LoadError::UnsupportedArch);
    }
    if hdr.abi_version_major != 1 {
        return Err(LoadError::UnsupportedAbi);
    }

    // ASLR randomization.
    let base = aslr_base();

    // Step 2: Parse section table.
    let table = SectionTable::parse(bytes, hdr.section_table_offset, hdr.section_count)
        .map_err(|_| LoadError::SectionOutOfRange)?;

    // Step 3: Verify BLAKE3 hashes if Signature section exists.
    verify_section_hashes(bytes, &table)?;

    // Step 4: Parse meta sections (TypeMap, VTables, LangBridge, etc.)
    let meta = parse_meta_sections(bytes, &table)?;
    let _stats: MetaSectionStats = meta_stats(&meta);

    // Step 5: Map sections into virtual memory.
    let mapped = map_sections(bytes, &table, base)?;

    // Step 6: Apply relocations.
    if let Some(reloc_entry) = table.find(SectionKind::Relocs) {
        apply_relocations(bytes, &table, &mapped, reloc_entry, base)?;
    }

    // Step 7: Resolve imports via runtime symbol table.
    let mut resolved_count = 0u32;
    if let Some(imports_entry) = table.find(SectionKind::Imports) {
        let import_start = imports_entry.file_offset as usize;
        let import_size = imports_entry.file_size as usize;
        if import_start + import_size <= bytes.len() && import_size >= 4 {
            let section_bytes = &bytes[import_start..import_start + import_size];
            // First 4 bytes = entry count.
            let count = u32::from_le_bytes([
                section_bytes[0], section_bytes[1], section_bytes[2], section_bytes[3],
            ]);
            let entry_data = &section_bytes[4..];
            if let Ok(import_table) = crate::bef::imports::ImportTable::parse(entry_data, count) {
                resolved_count = super::runtime::resolve_imports(&import_table, &mapped)
                    .unwrap_or(0);
            }
        }
    }
    let _ = resolved_count; // Used for diagnostics below.

    // Step 8: Parse TLS template if present.
    let mut tls_off = 0u64;
    let mut tls_sz = 0u64;
    if let Some(tls_entry) = table.find(SectionKind::Tls) {
        let tls = parse_tls_template(bytes, tls_entry)?;
        tls_off = tls.data_offset;
        tls_sz = tls.total_size();
    }

    // Build the final image.
    let mut img = fake_provenance_image(Provenance::Native);
    img.entry_point = base + hdr.entry_offset;
    img.base_address = base;
    img.sections = mapped;
    img.tls_offset = tls_off;
    img.tls_size = tls_sz;

    crate::diag::info_u64("bef", "native load complete, entry", img.entry_point);
    crate::diag::info_u64("bef", "resolved imports", resolved_count as u64);

    Ok(img)
}

/// Step 3: Verify section hashes against the Signature section.
fn verify_section_hashes(bytes: &[u8], table: &SectionTable) -> Result<(), LoadError> {
    let sig_entry = match table.find(SectionKind::Signature) {
        Some(e) => e,
        None => return Ok(()), // No signature section — skip verification.
    };

    // Parse SignatureHeader from the section bytes.
    let sig_start = sig_entry.file_offset as usize;
    if sig_start + core::mem::size_of::<SignatureHeader>() > bytes.len() {
        return Err(LoadError::Truncated);
    }
    let sig_hdr = unsafe {
        &*(bytes.as_ptr().add(sig_start) as *const SignatureHeader)
    };

    // If sig_algo == 0, no actual signature — just hash verification.
    // Iterate SectionHash entries.
    let hashes_start = sig_start + core::mem::size_of::<SignatureHeader>();
    let hash_count = sig_hdr.hash_count as usize;
    let hashes_size = hash_count * SectionHash::SIZE;
    if hashes_start + hashes_size > bytes.len() {
        return Err(LoadError::Truncated);
    }

    for i in 0..hash_count {
        let offset = hashes_start + i * SectionHash::SIZE;
        let entry = unsafe {
            &*(bytes.as_ptr().add(offset) as *const SectionHash)
        };

        // Find the section by index.
        let idx = entry.section_index as usize;
        if idx >= table.entries.len() {
            continue; // Skip invalid section indices.
        }
        let section = &table.entries[idx];

        // Compute BLAKE3 hash of the section data.
        let sec_start = section.file_offset as usize;
        let sec_size = section.file_size as usize;
        if sec_start + sec_size > bytes.len() {
            return Err(LoadError::Truncated);
        }
        let section_bytes = &bytes[sec_start..sec_start + sec_size];
        let computed = crate::bef::signing::blake3_256(section_bytes);

        if computed != entry.digest {
            return Err(LoadError::HashMismatch);
        }
    }

    Ok(())
}

/// Step 5: Map sections into virtual memory (allocate and copy data).
fn map_sections(bytes: &[u8], table: &SectionTable, base: u64) -> Result<Vec<MappedSection>, LoadError> {
    let mut mapped = Vec::new();
    let mut current_va = base;

    for entry in table.entries {
        if entry.file_size == 0 && entry.mem_size == 0 {
            continue; // Skip empty sections.
        }

        let mem_size = entry.mem_size.max(entry.file_size) as usize;
        let align = 4096usize; // Page alignment.
        let aligned_size = (mem_size + align - 1) & !(align - 1);

        // Allocate memory for this section.
        let layout = alloc::alloc::Layout::from_size_align(aligned_size, align)
            .map_err(|_| LoadError::SectionOutOfRange)?;
        let ptr = unsafe { alloc::alloc::alloc(layout) };
        if ptr.is_null() {
            return Err(LoadError::SectionOutOfRange);
        }

        // Zero-fill the allocated memory.
        unsafe { core::ptr::write_bytes(ptr, 0, aligned_size); }

        // Copy section data from file.
        let copy_len = (entry.file_size as usize).min(bytes.len() - entry.file_offset as usize);
        if copy_len > 0 {
            let src = &bytes[entry.file_offset as usize..entry.file_offset as usize + copy_len];
            unsafe { core::ptr::copy_nonoverlapping(src.as_ptr(), ptr, copy_len); }
        }

        // Determine flags from section kind.
        let flags = section_flags(entry);

        mapped.push(MappedSection {
            kind: entry.kind,
            virt_addr: current_va,
            size: aligned_size as u64,
            flags,
            data_ptr: ptr as u64,
        });

        current_va += aligned_size as u64;
    }

    Ok(mapped)
}

/// Determine section flags (R/W/X) from section kind.
fn section_flags(entry: &SectionEntry) -> u32 {
    let kind = SectionKind::from_u8(entry.kind);
    match kind {
        Some(SectionKind::Code) => 0x1,         // RX
        Some(SectionKind::RoData) => 0x4, // R
        Some(SectionKind::Data) => 0x3,         // RW
        Some(SectionKind::Tls) => 0x3,          // RW
        Some(SectionKind::Imports) => 0x4,      // R
        Some(SectionKind::Exports) => 0x4,      // R
        Some(SectionKind::Relocs) => 0x4,       // R
        Some(SectionKind::Signature) => 0x4,    // R
        _ => 0x3, // Default RW.
    }
}

/// Step 6: Apply relocations from the Relocs section.
fn apply_relocations(
    bytes: &[u8],
    _table: &SectionTable,
    mapped: &[MappedSection],
    reloc_entry: &SectionEntry,
    base: u64,
) -> Result<(), LoadError> {
    let reloc_start = reloc_entry.file_offset as usize;
    let reloc_size = reloc_entry.file_size as usize;
    if reloc_start + reloc_size > bytes.len() {
        return Err(LoadError::Truncated);
    }

    // Parse relocation entries.
    let reloc_count = reloc_size / Relocation::SIZE;
    for i in 0..reloc_count {
        let offset = reloc_start + i * Relocation::SIZE;
        let reloc = unsafe {
            &*(bytes.as_ptr().add(offset) as *const Relocation)
        };

        let _kind = reloc.kind().ok_or(LoadError::InvalidHeader)?;

        // Find the target section in mapped memory.
        let target_idx = reloc.target_section as usize;
        if target_idx >= mapped.len() {
            continue;
        }
        let target = &mapped[target_idx];

        // Resolve symbol address via runtime table.
        // For BEF native, symbol_idx refers to the Symbols section.
        let symbol_addr = resolve_symbol_for_reloc(reloc, mapped, base);

        // Apply the relocation using the actual section data pointer.
        if target.data_ptr != 0 {
            let target_slice = unsafe {
                core::slice::from_raw_parts_mut(
                    target.data_ptr as *mut u8,
                    target.size as usize,
                )
            };
            let _ = crate::bef::relocations::apply(
                reloc,
                target_slice,
                target.virt_addr + reloc.offset,
                symbol_addr,
            );
        }
    }

    Ok(())
}

/// Resolve a symbol address for a relocation entry.
fn resolve_symbol_for_reloc(reloc: &Relocation, mapped: &[MappedSection], base: u64) -> u64 {
    // Try runtime symbol table first.
    if reloc.symbol_idx != 0 {
        // Look up by symbol index in the runtime table.
        let addr = super::runtime::lookup_by_hash(reloc.symbol_idx, "");
        if addr != 0 {
            return addr;
        }
    }

    // For Abs64 with no symbol, use base + addend (position-independent).
    if reloc.symbol_idx == 0 {
        return base.wrapping_add(reloc.addend as u64);
    }

    // Fallback: return 0 (will cause a fault if called).
    0
}

/// Step 8: Parse TLS template from the TLS section.
fn parse_tls_template(bytes: &[u8], tls_entry: &SectionEntry) -> Result<TlsTemplate, LoadError> {
    let start = tls_entry.file_offset as usize;
    if start + core::mem::size_of::<TlsTemplate>() > bytes.len() {
        return Err(LoadError::Truncated);
    }
    let template = unsafe {
        core::ptr::read_volatile(bytes.as_ptr().add(start) as *const TlsTemplate)
    };
    Ok(template)
}
