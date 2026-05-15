//! Trait y errores comunes para marshalling.

use crate::barex::{BxError, BxResult};
use crate::barex::abi::type_system::TypeId;

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

/// Trait que cualquier marshaller per-lenguaje implementa.
pub trait Marshaller {
    /// Convierte un valor del lenguaje origen a representación BMO ABI.
    fn to_bmo(&self, src: *const u8, src_type: TypeId, dst: *mut u8) -> BxResult<usize>;

    /// Convierte BMO ABI de vuelta al lenguaje destino.
    fn from_bmo(&self, src: *const u8, dst: *mut u8, dst_type: TypeId) -> BxResult<usize>;
}
