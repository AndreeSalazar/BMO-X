//! Parser de la sección `PT_DYNAMIC` de ELF.
//!
//! Es un array de tuplas `Elf64Dyn { d_tag, d_un }` terminado por
//! `d_tag = DT_NULL`. Cada tag dice algo sobre el dynamic linking:
//!   - `DT_NEEDED`   → nombre de lib requerida (índice en `.dynstr`)
//!   - `DT_STRTAB`   → dirección virtual de `.dynstr`
//!   - `DT_SYMTAB`   → dirección virtual de `.dynsym`
//!   - `DT_RELA`     → dirección de la relocation table
//!   - `DT_PLTGOT`   → GOT
//!   - `DT_INIT/FINI` → constructores/destructores

#![allow(dead_code)]

use crate::bmo_core::bmo_abi::primitives::{bx_i64, bx_u64};

#[repr(C, packed)]
#[derive(Debug, Clone, Copy)]
pub struct Elf64Dyn {
    pub d_tag: bx_i64,
    pub d_un: bx_u64,        // value o ptr según el tag
}

// ─── Tags más usados ───────────────────────────────────────────────────
pub const DT_NULL:      bx_i64 = 0;
pub const DT_NEEDED:    bx_i64 = 1;
pub const DT_PLTRELSZ:  bx_i64 = 2;
pub const DT_PLTGOT:    bx_i64 = 3;
pub const DT_HASH:      bx_i64 = 4;
pub const DT_STRTAB:    bx_i64 = 5;
pub const DT_SYMTAB:    bx_i64 = 6;
pub const DT_RELA:      bx_i64 = 7;
pub const DT_RELASZ:    bx_i64 = 8;
pub const DT_RELAENT:   bx_i64 = 9;
pub const DT_STRSZ:     bx_i64 = 10;
pub const DT_SYMENT:    bx_i64 = 11;
pub const DT_INIT:      bx_i64 = 12;
pub const DT_FINI:      bx_i64 = 13;
pub const DT_SONAME:    bx_i64 = 14;
pub const DT_RPATH:     bx_i64 = 15;
pub const DT_SYMBOLIC:  bx_i64 = 16;
pub const DT_REL:       bx_i64 = 17;
pub const DT_RELSZ:     bx_i64 = 18;
pub const DT_RELENT:    bx_i64 = 19;
pub const DT_PLTREL:    bx_i64 = 20;
pub const DT_DEBUG:     bx_i64 = 21;
pub const DT_TEXTREL:   bx_i64 = 22;
pub const DT_JMPREL:    bx_i64 = 23;
pub const DT_BIND_NOW:  bx_i64 = 24;
pub const DT_INIT_ARRAY: bx_i64 = 25;
pub const DT_FINI_ARRAY: bx_i64 = 26;
pub const DT_INIT_ARRAYSZ: bx_i64 = 27;
pub const DT_FINI_ARRAYSZ: bx_i64 = 28;
pub const DT_RUNPATH:   bx_i64 = 29;
pub const DT_FLAGS:     bx_i64 = 30;
pub const DT_PREINIT_ARRAY: bx_i64 = 32;
pub const DT_PREINIT_ARRAYSZ: bx_i64 = 33;
pub const DT_GNU_HASH:  bx_i64 = 0x6FFFFEF5;

/// Resumen ya extraído de `PT_DYNAMIC` para uso del loader.
#[derive(Debug, Clone, Copy, Default)]
pub struct DynamicInfo {
    /// Lista de offsets en `.dynstr` con nombres de libs requeridas (DT_NEEDED).
    pub needed_offsets: [u64; 16],
    pub needed_count: u8,

    pub strtab_va: u64,
    pub strtab_size: u64,
    pub symtab_va: u64,
    pub syment_size: u64,
    pub rela_va: u64,
    pub rela_size: u64,
    pub jmprel_va: u64,
    pub jmprel_size: u64,
    pub pltgot_va: u64,
    pub init_va: u64,
    pub fini_va: u64,
}

/// Parsea el array `PT_DYNAMIC` del segmento.
pub fn parse(dyn_segment: &[u8]) -> DynamicInfo {
    let mut info = DynamicInfo::default();
    let entries = dyn_segment.len() / core::mem::size_of::<Elf64Dyn>();
    let ptr = dyn_segment.as_ptr() as *const Elf64Dyn;
    let dyns = unsafe { core::slice::from_raw_parts(ptr, entries) };

    for d in dyns {
        let tag = d.d_tag;
        let un = d.d_un;
        match tag {
            DT_NULL => break,
            DT_NEEDED => {
                if (info.needed_count as usize) < info.needed_offsets.len() {
                    info.needed_offsets[info.needed_count as usize] = un;
                    info.needed_count += 1;
                }
            }
            DT_STRTAB => info.strtab_va = un,
            DT_STRSZ  => info.strtab_size = un,
            DT_SYMTAB => info.symtab_va = un,
            DT_SYMENT => info.syment_size = un,
            DT_RELA   => info.rela_va = un,
            DT_RELASZ => info.rela_size = un,
            DT_JMPREL => info.jmprel_va = un,
            DT_PLTRELSZ => info.jmprel_size = un,
            DT_PLTGOT => info.pltgot_va = un,
            DT_INIT   => info.init_va = un,
            DT_FINI   => info.fini_va = un,
            _ => {}
        }
    }
    info
}

/// Lee el nombre ASCII de una lib desde `.dynstr` dado el offset DT_NEEDED.
pub fn read_dynstr<'a>(strtab: &'a [u8], offset: u64) -> Option<&'a str> {
    let off = offset as usize;
    if off >= strtab.len() { return None; }
    let slice = &strtab[off..];
    let len = slice.iter().position(|&b| b == 0).unwrap_or(slice.len());
    core::str::from_utf8(&slice[..len]).ok()
}
