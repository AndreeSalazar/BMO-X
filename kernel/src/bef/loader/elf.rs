//! ⭐ DEVOUR ELF — loader que come binarios Linux/Unix (.elf / .so).
//!
//! Lee el formato ELF64 (x86_64) y produce una `Image` BEF con
//! `format = BinaryFormat::ElfDevoured`. Los segments de programa (LOAD,
//! TLS, DYNAMIC) se mapean a `SectionKind` BEF; las relocs ELF
//! (`R_X86_64_*`) se canonicalizan a las 3 de BEF; los `DT_NEEDED` se
//! re-resuelven a libc-shim BMO.

#![allow(dead_code)]

use super::{Image, LoadError, fake_provenance_image};
use crate::bef::manifest::Provenance;
use crate::barex::abi::primitives::{bx_u8, bx_u16, bx_u32, bx_u64, bx_i64};

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

// ─── Relocations canonicales x86_64 ────────────────────────────────────
pub const R_X86_64_64:        bx_u32 = 1;   // → BEF Abs64
pub const R_X86_64_PC32:      bx_u32 = 2;   // → BEF Rel32
pub const R_X86_64_PLT32:     bx_u32 = 4;   // → BEF Rel32
pub const R_X86_64_GLOB_DAT:  bx_u32 = 6;   // → BEF Got64
pub const R_X86_64_JUMP_SLOT: bx_u32 = 7;   // → BEF Got64
pub const R_X86_64_RELATIVE:  bx_u32 = 8;   // → BEF Abs64 (sin símbolo)

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
        return Err(LoadError::UnsupportedArch); // ELF32 no soportado
    }
    if ehdr.e_ident.ei_data != 1 {
        return Err(LoadError::UnsupportedArch); // BE no soportado
    }
    let machine = ehdr.e_machine;
    if machine != ELF_MACHINE_X86_64 {
        return Err(LoadError::UnsupportedArch);
    }

    // TODO: iterar program headers, mapear PT_LOAD a secciones BEF,
    // procesar PT_DYNAMIC para encontrar DT_NEEDED, DT_RELA, DT_SYMTAB,
    // DT_STRTAB, DT_PLTGOT, etc., aplicar relocs x86_64 → BEF (3 tipos).
    let mut img = fake_provenance_image(Provenance::ElfDevoured);
    img.entry_point = ehdr.e_entry;
    Ok(img)
}

/// Convierte una reloc x86_64 ELF al equivalente BEF.
pub fn elf_reloc_to_bef(elf_kind: bx_u32) -> Option<crate::bef::relocations::RelocationKind> {
    use crate::bef::relocations::RelocationKind;
    match elf_kind {
        R_X86_64_64 | R_X86_64_RELATIVE              => Some(RelocationKind::Abs64),
        R_X86_64_PC32 | R_X86_64_PLT32               => Some(RelocationKind::Rel32),
        R_X86_64_GLOB_DAT | R_X86_64_JUMP_SLOT       => Some(RelocationKind::Got64),
        _ => None,
    }
}
