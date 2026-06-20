//! ⭐ DEVOUR ELF — loader que come binarios Linux/Unix (.elf / .so).
//!
//! Lee el formato ELF64 (x86_64) y produce una `Image` BEF con
//!
//! v1.6.16: allow(dead_code, unused_assignments) on the `dyn_*` offset
//! fields — they're parsed for completeness but the loader currently
//! only handles statically-linked ELFs (no PT_DYNAMIC). Future work.

#![allow(dead_code, unused_assignments, unused_variables)]
//! `format = BinaryFormat::ElfDevoured`. Los segments de programa (LOAD,
//! TLS, DYNAMIC) se mapean a `SectionKind` BEF; las relocs ELF
//! (`R_X86_64_*`) se canonicalizan a las 3 de BEF; los `DT_NEEDED` se
//! re-resuelven a libc-shim BMO.

use super::{Image, LoadError, MappedSection, fake_provenance_image};
use crate::bmo_core::bef::manifest::Provenance;
use crate::bmo_core::bmo_abi::primitives::{bx_u8, bx_u16, bx_u32, bx_u64, bx_i64};

pub const ELF_MAGIC: [bx_u8; 4] = [0x7F, b'E', b'L', b'F'];

// ─── ELF64 Identification (16 bytes) ───────────────────────────────────
#[repr(C, packed)]
#[derive(Debug, Clone, Copy)]
pub struct ElfIdent {
    pub ei_magic: [bx_u8; 4],
    pub ei_class: bx_u8,        // 1=ELF32, 2=ELF64
    pub ei_data: bx_u8,         // 1=LE, 2=BE
    pub ei_version: bx_u8,
    pub ei_osabi: bx_u8,        // 0=SysV, 3=Linux, etc.
    pub ei_abiversion: bx_u8,
    pub ei_pad: [bx_u8; 7],
}

// ─── ELF64 Header (64 bytes) ───────────────────────────────────────────
#[repr(C, packed)]
#[derive(Debug, Clone, Copy)]
pub struct Elf64Ehdr {
    pub e_ident: ElfIdent,
    pub e_type: bx_u16,         // 2=ET_EXEC, 3=ET_DYN
    pub e_machine: bx_u16,      // 0x3E = EM_X86_64
    pub e_version: bx_u32,
    pub e_entry: bx_u64,
    pub e_phoff: bx_u64,
    pub e_shoff: bx_u64,
    pub e_flags: bx_u32,
    pub e_ehsize: bx_u16,
    pub e_phentsize: bx_u16,
    pub e_phnum: bx_u16,
    pub e_shentsize: bx_u16,
    pub e_shnum: bx_u16,
    pub e_shstrndx: bx_u16,
}

pub const ELF_MACHINE_X86_64: bx_u16 = 0x3E;
pub const ET_EXEC: bx_u16 = 2;
pub const ET_DYN: bx_u16 = 3;

// ─── Program Header (56 bytes) ─────────────────────────────────────────
#[repr(C, packed)]
#[derive(Debug, Clone, Copy)]
pub struct Elf64Phdr {
    pub p_type: bx_u32,
    pub p_flags: bx_u32,        // PF_X=1, PF_W=2, PF_R=4
    pub p_offset: bx_u64,
    pub p_vaddr: bx_u64,
    pub p_paddr: bx_u64,
    pub p_filesz: bx_u64,
    pub p_memsz: bx_u64,
    pub p_align: bx_u64,
}

pub const PT_LOAD:    bx_u32 = 1;
pub const PT_DYNAMIC: bx_u32 = 2;
pub const PT_TLS:     bx_u32 = 7;
pub const PT_GNU_RELRO: bx_u32 = 0x6474E552;

// ─── ELF64 Section Header (64 bytes) ───────────────────────────────────
#[repr(C, packed)]
#[derive(Debug, Clone, Copy)]
pub struct Elf64Shdr {
    pub sh_name: bx_u32,
    pub sh_type: bx_u32,
    pub sh_flags: bx_u64,
    pub sh_addr: bx_u64,
    pub sh_offset: bx_u64,
    pub sh_size: bx_u64,
    pub sh_link: bx_u32,
    pub sh_info: bx_u32,
    pub sh_addralign: bx_u64,
    pub sh_entsize: bx_u64,
}

pub const SHT_RELA: bx_u32 = 4;

// ─── Relocations canónicas x86_64 ──────────────────────────────────────
pub const R_X86_64_64:        bx_u32 = 1;
pub const R_X86_64_PC32:      bx_u32 = 2;
pub const R_X86_64_PLT32:     bx_u32 = 4;
pub const R_X86_64_GLOB_DAT:  bx_u32 = 6;
pub const R_X86_64_JUMP_SLOT: bx_u32 = 7;
pub const R_X86_64_RELATIVE:  bx_u32 = 8;

#[repr(C, packed)]
#[derive(Debug, Clone, Copy)]
pub struct Elf64Rela {
    pub r_offset: bx_u64,
    pub r_info: bx_u64,         // (sym_idx << 32) | type
    pub r_addend: bx_i64,
}

pub fn load(bytes: &[u8]) -> Result<Image, LoadError> {
    if bytes.len() < core::mem::size_of::<Elf64Ehdr>() {
        return Err(LoadError::Truncated);
    }
    let ehdr = unsafe { &*(bytes.as_ptr() as *const Elf64Ehdr) };
    if ehdr.e_ident.ei_magic != ELF_MAGIC {
        return Err(LoadError::InvalidHeader);
    }
    if ehdr.e_ident.ei_class != 2 {
        return Err(LoadError::UnsupportedArch);
    }
    if ehdr.e_ident.ei_data != 1 {
        return Err(LoadError::UnsupportedArch);
    }
    let machine = ehdr.e_machine;
    if machine != ELF_MACHINE_X86_64 {
        return Err(LoadError::UnsupportedArch);
    }

    // ─── Parse section headers (for relocations) ─────────────────────
    let shoff = ehdr.e_shoff as usize;
    let shnum = ehdr.e_shnum as usize;
    let shent = ehdr.e_shentsize as usize;
    let shstrndx = ehdr.e_shstrndx as usize;

    let mut dyn_symtab_offset: u64 = 0;
    let mut dyn_symtab_entry_size: u64 = 0;
    let mut dyn_strtab_offset: u64 = 0;
    let mut dyn_strtab_size: u64 = 0;

    // Parse .dynsym and .dynstr section headers.
    if shoff + shnum * shent <= bytes.len() && shnum > 0 {
        for i in 0..shnum {
            let off = shoff + i * shent;
            let shdr = unsafe { &*(bytes.as_ptr().add(off) as *const Elf64Shdr) };
            // SHT_DYNSYM = 11
            if shdr.sh_type == 11 {
                dyn_symtab_offset = shdr.sh_offset;
                dyn_symtab_entry_size = shdr.sh_entsize.max(24);
            }
            // SHT_STRTAB = 3 — we need the one linked from .dynsym
            if shdr.sh_type == 3 && i != shstrndx {
                // Heuristic: if this strtab is large, it's likely .dynstr
                if shdr.sh_size > dyn_strtab_size {
                    dyn_strtab_offset = shdr.sh_offset;
                    dyn_strtab_size = shdr.sh_size;
                }
            }
        }
    }

    // ─── Iterate program headers ─────────────────────────────────────
    let phoff = ehdr.e_phoff as usize;
    let phnum = ehdr.e_phnum as usize;
    let phent = ehdr.e_phentsize as usize;
    if phent < core::mem::size_of::<Elf64Phdr>() {
        return Err(LoadError::InvalidHeader);
    }
    if phoff + phnum * phent > bytes.len() {
        return Err(LoadError::Truncated);
    }

    let mut img = fake_provenance_image(Provenance::ElfDevoured);
    img.entry_point = ehdr.e_entry;

    let mut pt_dynamic_offset: Option<(u64, u64)> = None;

    for i in 0..phnum {
        let off = phoff + i * phent;
        let phdr = unsafe { &*(bytes.as_ptr().add(off) as *const Elf64Phdr) };
        let p_type = phdr.p_type;
        let p_flags = phdr.p_flags;
        let p_offset = phdr.p_offset;
        let p_vaddr = phdr.p_vaddr;
        let p_filesz = phdr.p_filesz;
        let p_memsz = phdr.p_memsz;

        match p_type {
            PT_LOAD => {
                let mut flags = 0u32;
                if p_flags & 4 != 0 { flags |= 0x1; }   // R
                if p_flags & 2 != 0 { flags |= 0x2; }   // W
                if p_flags & 1 != 0 { flags |= 0x4; }   // X
                let kind = pick_kind_from_flags(p_flags);

                // Allocate and copy segment data into memory.
                let alloc_size = p_memsz as usize;
                let align = 4096usize;
                let aligned_size = (alloc_size + align - 1) & !(align - 1);

                let data_ptr = if aligned_size > 0 {
                    let layout = core::alloc::Layout::from_size_align(aligned_size, align)
                        .map_err(|_| LoadError::SectionOutOfRange)?;
                    let ptr = unsafe { alloc::alloc::alloc_zeroed(layout) };
                    if ptr.is_null() {
                        return Err(LoadError::SectionOutOfRange);
                    }

                    // Copy from file.
                    let copy_len = (p_filesz as usize).min(bytes.len() - p_offset as usize);
                    if copy_len > 0 && p_offset as usize + copy_len <= bytes.len() {
                        let src = &bytes[p_offset as usize..p_offset as usize + copy_len];
                        unsafe { core::ptr::copy_nonoverlapping(src.as_ptr(), ptr, copy_len); }
                    }

                    ptr as u64
                } else {
                    0
                };

                img.sections.push(MappedSection {
                    kind,
                    virt_addr: p_vaddr,
                    size: p_memsz,
                    flags,
                    data_ptr,
                });
            }
            PT_DYNAMIC => {
                pt_dynamic_offset = Some((p_offset, p_filesz));
            }
            PT_TLS => {
                use crate::bmo_core::bef::sections::SectionKind;
                img.sections.push(MappedSection {
                    kind: SectionKind::Tls as u8,
                    virt_addr: p_vaddr,
                    size: p_memsz,
                    flags: 0x1,
                    data_ptr: 0,
                });
            }
            _ => {}
        }
    }

    // ─── Process PT_DYNAMIC if present ────────────────────────────────
    if let Some((dyn_off, dyn_size)) = pt_dynamic_offset {
        let start = dyn_off as usize;
        let end = start + dyn_size as usize;
        if end <= bytes.len() {
            let dyn_bytes = &bytes[start..end];
            let info = super::elf_dynamic::parse(dyn_bytes);

            // Register needed libraries from DT_NEEDED.
            for &needed_off in &info.needed_offsets {
                if let Some(lib_name) = super::elf_dynamic::read_dynstr(dyn_bytes, needed_off) {
                    if !lib_name.is_empty() {
                        let _normalized = normalize_elf_lib(lib_name);
                        crate::bmo_core::diag::info("elf", "registering ELF lib");
                    }
                }
            }
        }
    }

    // ─── Apply ELF relocations (from .rela sections) ─────────────────
    apply_elf_relocations(bytes, &mut img, shoff, shnum, shent, ehdr)?;

    Ok(img)
}

/// Apply ELF relocations from SHT_RELA sections.
fn apply_elf_relocations(
    bytes: &[u8],
    img: &mut Image,
    shoff: usize,
    shnum: usize,
    shent: usize,
    ehdr: &Elf64Ehdr,
) -> Result<(), LoadError> {
    let _shstrndx = ehdr.e_shstrndx as usize;
    if shoff + shnum * shent > bytes.len() {
        return Ok(()); // No section headers.
    }

    for i in 0..shnum {
        let off = shoff + i * shent;
        let shdr = unsafe { &*(bytes.as_ptr().add(off) as *const Elf64Shdr) };

        if shdr.sh_type != SHT_RELA { continue; }
        if shdr.sh_size == 0 { continue; }

        let reloc_offset = shdr.sh_offset as usize;
        let reloc_size = shdr.sh_size as usize;
        if reloc_offset + reloc_size > bytes.len() { continue; }

        let entry_size = core::mem::size_of::<Elf64Rela>();
        let reloc_count = reloc_size / entry_size;

        for j in 0..reloc_count {
            let r_off = reloc_offset + j * entry_size;
            let reloc = unsafe {
                &*(bytes.as_ptr().add(r_off) as *const Elf64Rela)
            };

            let r_type = (reloc.r_info & 0xFFFFFFFF) as u32;
            let _sym_idx = (reloc.r_info >> 32) as u32;

            // Convert ELF relocation to BEF.
            let bef_kind = match elf_reloc_to_bef(r_type) {
                Some(k) => k,
                None => continue, // Unknown relocation type.
            };

            // Find the section containing this offset.
            let target_va = reloc.r_offset;
            for section in &mut img.sections {
                if target_va >= section.virt_addr
                    && target_va + 8 <= section.virt_addr + section.size
                {
                    if section.data_ptr == 0 { break; }

                    let _offset_in_section = (target_va - section.virt_addr) as usize;
                    let target_slice = unsafe {
                        core::slice::from_raw_parts_mut(
                            section.data_ptr as *mut u8,
                            section.size as usize,
                        )
                    };

                    // For R_X86_64_RELATIVE, symbol_addr = base + addend.
                    let symbol_addr = if r_type == R_X86_64_RELATIVE {
                        target_va.wrapping_add(reloc.r_addend as u64)
                    } else {
                        // TODO: resolve symbol via .dynsym lookup.
                        0
                    };

                    let bef_reloc = crate::bmo_core::bef::relocations::Relocation {
                        offset: (target_va - section.virt_addr),
                        symbol_idx: 0,
                        kind: bef_kind as u8,
                        target_section: 0,
                        _pad: [0; 2],
                        addend: reloc.r_addend,
                    };

                    let _ = crate::bmo_core::bef::relocations::apply(
                        &bef_reloc,
                        target_slice,
                        target_va,
                        symbol_addr,
                    );
                    break;
                }
            }
        }
    }

    Ok(())
}

/// Elige `SectionKind` BEF a partir de los flags de un PT_LOAD ELF.
fn pick_kind_from_flags(p_flags: u32) -> u8 {
    use crate::bmo_core::bef::sections::SectionKind;
    let x = p_flags & 1 != 0;
    let w = p_flags & 2 != 0;
    match (x, w) {
        (true, _)      => SectionKind::Code as u8,
        (false, true)  => SectionKind::Data as u8,
        (false, false) => SectionKind::RoData as u8,
    }
}

/// Normalize ELF library names (e.g., "libc.so.6" → "libc.so").
fn normalize_elf_lib(name: &str) -> &str {
    if let Some(pos) = name.find('.') {
        &name[..pos]
    } else {
        name
    }
}

/// Convierte una reloc x86_64 ELF al equivalente BEF.
pub fn elf_reloc_to_bef(elf_kind: bx_u32) -> Option<crate::bmo_core::bef::relocations::RelocationKind> {
    use crate::bmo_core::bef::relocations::RelocationKind;
    match elf_kind {
        R_X86_64_64 | R_X86_64_RELATIVE              => Some(RelocationKind::Abs64),
        R_X86_64_PC32 | R_X86_64_PLT32               => Some(RelocationKind::Rel32),
        R_X86_64_GLOB_DAT | R_X86_64_JUMP_SLOT       => Some(RelocationKind::Got64),
        _ => None,
    }
}
