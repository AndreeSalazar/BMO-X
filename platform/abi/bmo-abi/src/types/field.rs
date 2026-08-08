//! Type field descriptor -- per-field metadata for structs, enums, and unions.
//!
//! Each `TypeField` describes one field: its name (FNV-1a hashed), its type
//! (index into TypeRegistry), and its byte offset within the parent struct.

use crate::bmo_abi::primitives::{bx_u32, bx_u64};

/// Maximum number of fields per type.
pub const MAX_FIELDS: usize = 64;

/// Describes a single field in a struct, enum variant, or union.
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct TypeField {
    /// FNV-1a 64-bit hash of the field name.
    pub name_hash: bx_u64,
    /// Index into TypeRegistry for the field's type.
    pub type_id: bx_u32,
    /// Byte offset from the start of the parent type.
    pub offset: bx_u32,
    /// Field flags (reserved: packed, volatile, bitfield).
    pub flags: bx_u32,
}

const _: () = assert!(core::mem::size_of::<TypeField>() == 24);

impl TypeField {
    pub const fn new(name_hash: bx_u64, type_id: bx_u32, offset: bx_u32) -> Self {
        Self {
            name_hash,
            type_id,
            offset,
            flags: 0,
        }
    }
}

/// A container of field descriptors for one type.
///
/// Stored alongside TypeMeta in the BEF `.type_map` section. The parent
/// TypeMeta's `field_count` tells the loader how many fields to read.
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct FieldTable {
    pub fields: [TypeField; MAX_FIELDS],
    pub count: bx_u32,
}

impl FieldTable {
    pub fn empty() -> Self {
        Self {
            fields: [TypeField::new(0, 0, 0); MAX_FIELDS],
            count: 0,
        }
    }

    pub fn add(&mut self, field: TypeField) -> Option<bx_u32> {
        if self.count as usize >= MAX_FIELDS {
            return None;
        }
        let idx = self.count;
        self.fields[idx as usize] = field;
        self.count += 1;
        Some(idx)
    }

    pub fn lookup_by_name(&self, name_hash: bx_u64) -> Option<&TypeField> {
        self.fields[..self.count as usize]
            .iter()
            .find(|f| f.name_hash == name_hash)
    }

    pub fn lookup_by_offset(&self, offset: bx_u32) -> Option<&TypeField> {
        self.fields[..self.count as usize]
            .iter()
            .find(|f| f.offset == offset)
    }

    pub fn get(&self, idx: bx_u32) -> Option<&TypeField> {
        self.fields
            .get(idx as usize)
            .filter(|_| (idx as usize) < self.count as usize)
    }
}
