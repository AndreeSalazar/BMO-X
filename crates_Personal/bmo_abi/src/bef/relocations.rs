//! Relocations BEF — solo 3 tipos (vs 38 de ELF x86_64, 16 de PE).
//!
//! Modelo BEF:
//!   - **Abs64**  — escribir dirección absoluta de 64 bits.
//!   - **Rel32**  — escribir delta de 32 bits (PC-relative).
//!   - **Got64**  — escribir dirección via Global Offset Table.
//!
//! Eso cubre el 100 % de los casos que ELF resuelve con sus 38 tipos. El
//! resto eran legacy (R_X86_64_8, R_X86_64_16, R_X86_64_TPOFF*, etc.).

#![allow(dead_code)]

use crate::bmo_abi::primitives::{bx_u8, bx_u32, bx_u64, bx_i64};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum RelocationKind {
    /// Escribe `symbol_addr + addend` (64 bits absolutos).
    /// ELF: `R_X86_64_64`. PE: `IMAGE_REL_BASED_DIR64`.
    Abs64    = 0x01,
    /// Escribe `symbol_addr + addend - reloc_addr` (32 bits, PC-relative).
    /// ELF: `R_X86_64_PC32`/`R_X86_64_PLT32`.
    Rel32    = 0x02,
    /// Escribe la dirección del slot GOT del símbolo (64 bits).
    /// ELF: `R_X86_64_GLOB_DAT`/`R_X86_64_JUMP_SLOT`.
    Got64    = 0x03,
}

impl RelocationKind {
    pub fn from_u8(v: bx_u8) -> Option<Self> {
        match v { 0x01 => Some(Self::Abs64), 0x02 => Some(Self::Rel32), 0x03 => Some(Self::Got64), _ => None }
    }
}

/// Una relocation — 24 bytes.
#[repr(C, align(8))]
#[derive(Debug, Clone, Copy)]
pub struct Relocation {
    /// Offset en la sección target donde aplicar la reloc.
    pub offset: bx_u64,
    /// Índice del símbolo en la sección Symbols (o Imports).
    pub symbol_idx: bx_u32,
    /// Tipo `RelocationKind as u8`.
    pub kind: bx_u8,
    /// `0` = sección target es `.code`, `1` = `.data`, `2` = `.rodata`.
    pub target_section: bx_u8,
    /// Padding.
    pub _pad: [bx_u8; 2],
    /// Addend con signo.
    pub addend: bx_i64,
}
const _: () = assert!(core::mem::size_of::<Relocation>() == 24);

impl Relocation {
    pub const SIZE: usize = 24;

    pub fn kind(&self) -> Option<RelocationKind> {
        RelocationKind::from_u8(self.kind)
    }
}

/// Aplica una relocation single sobre un buffer mutable que representa la
/// sección target ya cargada en memoria.
///
/// `reloc_va` es la dirección virtual final de `target[reloc.offset]`.
/// `symbol_addr` es la dirección virtual final del símbolo.
pub fn apply(reloc: &Relocation, target: &mut [u8], reloc_va: u64, symbol_addr: u64) -> Result<(), &'static str> {
    let off = reloc.offset as usize;
    let kind = reloc.kind().ok_or("kind de relocation desconocido")?;
    match kind {
        RelocationKind::Abs64 => {
            if off + 8 > target.len() { return Err("offset Abs64 fuera de rango"); }
            let v = symbol_addr.wrapping_add(reloc.addend as u64);
            target[off..off + 8].copy_from_slice(&v.to_le_bytes());
        }
        RelocationKind::Rel32 => {
            if off + 4 > target.len() { return Err("offset Rel32 fuera de rango"); }
            let pc = reloc_va as i64;
            let v = (symbol_addr as i64).wrapping_add(reloc.addend).wrapping_sub(pc);
            target[off..off + 4].copy_from_slice(&(v as i32).to_le_bytes());
        }
        RelocationKind::Got64 => {
            if off + 8 > target.len() { return Err("offset Got64 fuera de rango"); }
            // En BEF, el GOT slot ya fue resuelto por el loader; aquí escribimos
            // su dirección. El addend se suma como offset dentro del slot (raro).
            let v = symbol_addr.wrapping_add(reloc.addend as u64);
            target[off..off + 8].copy_from_slice(&v.to_le_bytes());
        }
    }
    Ok(())
}
