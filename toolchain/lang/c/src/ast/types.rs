//! C Abstract Syntax Tree — type system definitions.

#[derive(Debug, Clone, PartialEq)]
pub enum TypeSpec {
    Void,
    Char,
    Short,
    Int,
    Long,
    LongLong,
    UnsignedInt,
    UnsignedLong,
    UnsignedChar,
    UnsignedShort,
    UnsignedLongLong,
    Float,
    Double,
    Ptr(Box<TypeSpec>),
    // Array real: int arr[4] ocupa 4*4 bytes en el stack, no 8.
    Array(Box<TypeSpec>, u32),
    StructRef(String),
    UnionRef(String),
}

impl TypeSpec {
    pub fn stack_size(&self) -> u32 {
        match self {
            TypeSpec::Void => 0,
            TypeSpec::Char | TypeSpec::UnsignedChar => 1,
            TypeSpec::Short | TypeSpec::UnsignedShort => 2,
            TypeSpec::Int | TypeSpec::UnsignedInt => 4,
            TypeSpec::Long | TypeSpec::UnsignedLong | TypeSpec::LongLong | TypeSpec::UnsignedLongLong => 8,
            TypeSpec::Float => 4,
            TypeSpec::Double => 8,
            TypeSpec::Ptr(_) => 8,
            TypeSpec::Array(t, n) => t.stack_size() * n,
            TypeSpec::StructRef(_) | TypeSpec::UnionRef(_) => 0,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct Param {
    pub typ: TypeSpec,
    pub name: String,
}
