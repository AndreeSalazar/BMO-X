//! `bmo::marshal` — Type marshaling between language ABIs.
//!
//! Converts values from one language's ABI to another.
//! This is what makes interop possible without a universal bytecode.

#![allow(dead_code)]

/// ABI type categories for marshaling.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MarshalType {
    Void, Bool,
    I8, I16, I32, I64,
    U8, U16, U32, U64,
    F32, F64,
    Pointer, String, Slice,
}

impl MarshalType {
    pub const fn size(&self) -> usize {
        match self {
            MarshalType::Void => 0, MarshalType::Bool => 1,
            MarshalType::I8 | MarshalType::U8 => 1,
            MarshalType::I16 | MarshalType::U16 => 2,
            MarshalType::I32 | MarshalType::U32 | MarshalType::F32 => 4,
            MarshalType::I64 | MarshalType::U64 | MarshalType::F64 | MarshalType::Pointer => 8,
            MarshalType::String | MarshalType::Slice => 16,
        }
    }
    pub const fn is_numeric(&self) -> bool {
        matches!(self, MarshalType::I8 | MarshalType::I16 | MarshalType::I32 | MarshalType::I64
                     | MarshalType::U8 | MarshalType::U16 | MarshalType::U32 | MarshalType::U64
                     | MarshalType::F32 | MarshalType::F64)
    }
}

/// A marshaled value (max 8 bytes inline).
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct MarshaledValue {
    pub data: u64,
    pub ty: MarshalType,
    _pad: u32,
}

impl MarshaledValue {
    pub const fn from_u64(v: u64) -> Self { Self { data: v, ty: MarshalType::U64, _pad: 0 } }
    pub const fn from_i64(v: i64) -> Self { Self { data: v as u64, ty: MarshalType::I64, _pad: 0 } }
    pub const fn from_bool(v: bool) -> Self { Self { data: if v { 1 } else { 0 }, ty: MarshalType::Bool, _pad: 0 } }
    pub const fn from_f64(v: f64) -> Self { Self { data: v.to_bits(), ty: MarshalType::F64, _pad: 0 } }
    pub const fn from_ptr(v: u64) -> Self { Self { data: v, ty: MarshalType::Pointer, _pad: 0 } }
    pub fn as_u64(&self) -> u64 { self.data }
    pub fn as_i64(&self) -> i64 { self.data as i64 }
    pub fn as_bool(&self) -> bool { self.data != 0 }
    pub fn as_f64(&self) -> f64 { f64::from_bits(self.data) }
    pub fn as_ptr(&self) -> *mut u8 { self.data as *mut u8 }
}

/// Widening conversion (no precision loss).
pub fn widen(val: MarshaledValue, target: MarshalType) -> Result<MarshaledValue, super::plugins::traits::AdapterError> {
    match (val.ty, target) {
        (a, b) if a == b => Ok(val),
        (MarshalType::I8, MarshalType::I16 | MarshalType::I32 | MarshalType::I64)
        | (MarshalType::I16, MarshalType::I32 | MarshalType::I64)
        | (MarshalType::I32, MarshalType::I64)
        | (MarshalType::U8, MarshalType::U16 | MarshalType::U32 | MarshalType::U64)
        | (MarshalType::U16, MarshalType::U32 | MarshalType::U64)
        | (MarshalType::U32, MarshalType::U64)
        | (MarshalType::F32, MarshalType::F64)
        | (MarshalType::Pointer, MarshalType::U64) => Ok(MarshaledValue { data: val.data, ty: target, _pad: 0 }),
        _ => Err(super::plugins::traits::AdapterError::TypeError),
    }
}
