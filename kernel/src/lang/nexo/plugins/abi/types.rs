//! ABI type definitions.

#![allow(dead_code)]

extern crate alloc;
use alloc::string::String;
use alloc::vec::Vec;

/// ABI type representation
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AbiType {
    Void,
    Bool,
    I8,
    I16,
    I32,
    I64,
    U8,
    U16,
    U32,
    U64,
    F32,
    F64,
    Pointer,
    Struct(u32),
    Array(u32, u32),
}

impl AbiType {
    /// Get size in bytes
    pub fn size_bytes(&self) -> usize {
        match self {
            AbiType::Void => 0,
            AbiType::Bool => 1,
            AbiType::I8 | AbiType::U8 => 1,
            AbiType::I16 | AbiType::U16 => 2,
            AbiType::I32 | AbiType::U32 | AbiType::F32 => 4,
            AbiType::I64 | AbiType::U64 | AbiType::F64 => 8,
            AbiType::Pointer => 8, // 64-bit
            AbiType::Struct(id) => 0, // Would need struct table
            AbiType::Array(elem, count) => elem.size_bytes() * count as usize,
        }
    }

    /// Check if type is numeric
    pub fn is_numeric(&self) -> bool {
        matches!(self,
            AbiType::I8 | AbiType::I16 | AbiType::I32 | AbiType::I64 |
            AbiType::U8 | AbiType::U16 | AbiType::U32 | AbiType::U64 |
            AbiType::F32 | AbiType::F64
        )
    }

    /// Check if type is integer
    pub fn is_integer(&self) -> bool {
        matches!(self,
            AbiType::I8 | AbiType::I16 | AbiType::I32 | AbiType::I64 |
            AbiType::U8 | AbiType::U16 | AbiType::U32 | AbiType::U64
        )
    }

    /// Check if type is floating point
    pub fn is_float(&self) -> bool {
        matches!(self, AbiType::F32 | AbiType::F64)
    }

    /// Check if type is signed
    pub fn is_signed(&self) -> bool {
        matches!(self, AbiType::I8 | AbiType::I16 | AbiType::I32 | AbiType::I64)
    }
}

/// ABI function signature
#[derive(Debug, Clone)]
pub struct AbiSignature {
    pub name: String,
    pub params: Vec<AbiParam>,
    pub return_type: AbiType,
}

/// ABI parameter
#[derive(Debug, Clone)]
pub struct AbiParam {
    pub name: String,
    pub ty: AbiType,
}
