//! Trait y errores comunes para marshalling.

use crate::bmo_core::barex::{BxError, BxResult};
use crate::bmo_core::bmo_abi::primitives::bx_usize;
use crate::bmo_core::bmo_abi::type_system::{TypeDescriptor, TypeLayout, TypeKind};

#[repr(u32)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MarshalError {
    TypeMismatch       = 1,
    UnsupportedSource  = 2,
    UnsupportedTarget  = 3,
    BufferTooSmall     = 4,
    InvalidEncoding    = 5,
}

impl From<MarshalError> for BxError {
    fn from(e: MarshalError) -> BxError {
        match e {
            MarshalError::TypeMismatch       => BxError::InvalidArgument,
            MarshalError::UnsupportedSource  => BxError::Unsupported,
            MarshalError::UnsupportedTarget  => BxError::Unsupported,
            MarshalError::BufferTooSmall     => BxError::BufferTooSmall,
            MarshalError::InvalidEncoding    => BxError::InvalidArgument,
        }
    }
}

/// Calcula el tamaño en bytes que ocupa un tipo según su `TypeKind`.
/// (El layout completo está en `TypeLayout`, pero este helper no necesita
/// la struct entera — solo el kind.)
pub fn size_of_kind(kind: TypeKind) -> bx_usize {
    match kind {
        TypeKind::Void | TypeKind::Never => 0,
        TypeKind::Bool | TypeKind::Char => 1,
        TypeKind::SignedInt | TypeKind::UnsignedInt | TypeKind::Float => 8,
        TypeKind::Pointer | TypeKind::Handle => 8,
        TypeKind::Slice => 16,           // (ptr, len)
        TypeKind::String => 16,          // mismo layout que BmoStr
        _ => 16,                         // structs/arrays: por defecto puntero
    }
}

/// Trait que cualquier marshaller per-lenguaje implementa.
pub trait Marshaller {
    /// Convierte un valor del lenguaje origen a representación BMO ABI.
    fn to_bmo(&self, src: *const u8, src_layout: &TypeLayout, src_kind: TypeKind,
              dst: *mut u8) -> BxResult<bx_usize>;

    /// Convierte BMO ABI de vuelta al lenguaje destino.
    fn from_bmo(&self, src: *const u8, dst: *mut u8,
                dst_layout: &TypeLayout, dst_kind: TypeKind) -> BxResult<bx_usize>;

    /// Identifica este marshaller (para debug/registry).
    fn id(&self) -> u32;
}

/// Marshaller canónico: copia bytes sin transformación (identidad).
///
/// Útil cuando el lenguaje origen ya usa el BMO ABI o uno compatible
/// (Rust sin features exotic, C con `repr(C)` bien definido).
pub struct IdentityMarshaller;

impl Marshaller for IdentityMarshaller {
    fn to_bmo(&self, src: *const u8, src_layout: &TypeLayout, _src_kind: TypeKind,
              dst: *mut u8) -> BxResult<bx_usize> {
        if src.is_null() || dst.is_null() {
            return Err(MarshalError::InvalidEncoding.into());
        }
        let n = src_layout.padded_size() as usize;
        if n == 0 { return Ok(0); }
        unsafe {
            core::ptr::copy_nonoverlapping(src, dst, n);
        }
        Ok(n as bx_usize)
    }

    fn from_bmo(&self, src: *const u8, dst: *mut u8,
                dst_layout: &TypeLayout, _dst_kind: TypeKind) -> BxResult<bx_usize> {
        if src.is_null() || dst.is_null() {
            return Err(MarshalError::InvalidEncoding.into());
        }
        let n = dst_layout.padded_size() as usize;
        if n == 0 { return Ok(0); }
        unsafe {
            core::ptr::copy_nonoverlapping(src, dst, n);
        }
        Ok(n as bx_usize)
    }

    fn id(&self) -> u32 { 0 }
}

/// Marshaller primitivo: maneja los `TypeKind` primitivos (int, float, bool).
///
/// - `SignedInt`/`UnsignedInt`: copia 8 bytes little-endian.
/// - `Float`: copia 8 bytes IEEE 754.
/// - `Bool`: 1 byte (0 o 1).
/// - `Pointer`/`Handle`: 8 bytes.
pub struct PrimitiveMarshaller;

impl Marshaller for PrimitiveMarshaller {
    fn to_bmo(&self, src: *const u8, _src_layout: &TypeLayout, src_kind: TypeKind,
              dst: *mut u8) -> BxResult<bx_usize> {
        if src.is_null() || dst.is_null() {
            return Err(MarshalError::InvalidEncoding.into());
        }
        let n = size_of_kind(src_kind) as usize;
        if n == 0 { return Ok(0); }
        unsafe {
            core::ptr::copy_nonoverlapping(src, dst, n);
        }
        Ok(n as bx_usize)
    }

    fn from_bmo(&self, src: *const u8, dst: *mut u8,
                _dst_layout: &TypeLayout, dst_kind: TypeKind) -> BxResult<bx_usize> {
        if src.is_null() || dst.is_null() {
            return Err(MarshalError::InvalidEncoding.into());
        }
        let n = size_of_kind(dst_kind) as usize;
        if n == 0 { return Ok(0); }
        unsafe {
            core::ptr::copy_nonoverlapping(src, dst, n);
        }
        Ok(n as bx_usize)
    }

    fn id(&self) -> u32 { 1 }
}

/// Registry global de marshallers por lenguaje origen.
pub struct MarshallerRegistry {
    marshallers: [Option<&'static dyn Marshaller>; 8],
}

impl MarshallerRegistry {
    pub const fn empty() -> Self {
        Self { marshallers: [None; 8] }
    }

    pub fn register(&mut self, idx: usize, m: &'static dyn Marshaller) {
        if idx < 8 {
            self.marshallers[idx] = Some(m);
        }
    }

    pub fn get(&self, idx: usize) -> Option<&'static dyn Marshaller> {
        if idx < 8 { self.marshallers[idx] } else { None }
    }
}

/// Helper: marshall desde un TypeDescriptor (en lugar de TypeId suelto).
#[inline]
pub fn to_bmo_via<M: Marshaller>(m: &M, src: *const u8, td: &TypeDescriptor<'_>,
                                   dst: *mut u8) -> BxResult<bx_usize> {
    m.to_bmo(src, &td.layout, td.kind, dst)
}

#[inline]
pub fn from_bmo_via<M: Marshaller>(m: &M, src: *const u8, dst: *mut u8,
                                     td: &TypeDescriptor<'_>) -> BxResult<bx_usize> {
    m.from_bmo(src, dst, &td.layout, td.kind)
}
