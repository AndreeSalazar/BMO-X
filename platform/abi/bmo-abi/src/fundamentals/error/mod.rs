//! `error` -- BmoError, el tipo de error unificado del BMO ABI.
//!
//! Reemplaza el caos de codigos sueltos de C y los enums/thiserror de Rust
//! con un solo tipo de 16 bytes que puede representar cualquier error del
//! sistema, incluyendo el codigo de error, flags de contexto, y un payload
//! opcional de 64 bits.
//!
//! -- EL SEMAFORO (L6g) y las dos preguntas de antes (L6e, L6f) --------
//!
//! Que cuesta que falle, por que falla ESTA pieza, y que arrastro si la
//! toco. La ley esta en `META-KERNEL_HARD.md`.
//!
//! [carril]  AMARILLO     el `BmoError` unificado. No decide nada: describe
//! [cuesta]  NADA         un error mal descrito manda a mirar donde no es
//! [riesgo]  ESPEJO       es la TERCERA forma de decir lo mismo, con
//!                        `BmoStatus` y `ErrorCode`. Tres nombres de un fallo
//!                        pueden discrepar

use crate::bmo_abi::error_code;
use crate::bmo_abi::fundamentals::status::error::message as error_message;
use crate::bmo_abi::fundamentals::status::BmoStatus;
use crate::bmo_abi::primitives::{bx_u32, bx_u64};

/// Error unificado del BMO ABI -- 16 bytes, cabe en RAX:RDX.
///
/// # Layout
/// ```text
/// [0..3]  code:    u32  -- error_code::* (0 = OK)
/// [4..7]  flags:   u32  -- StatusFlags bits
/// [8..15] context: u64  -- handle, direccion, offset, lo que aplique
/// ```
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BmoError {
    pub code: bx_u32,
    pub flags: bx_u32,
    pub context: bx_u64,
}
const _: () = assert!(core::mem::size_of::<BmoError>() == 16);

impl BmoError {
    pub const OK: Self = Self {
        code: 0,
        flags: 0,
        context: 0,
    };

    pub const fn new(code: bx_u32) -> Self {
        Self {
            code,
            flags: 0,
            context: 0,
        }
    }

    pub const fn with_context(code: bx_u32, context: bx_u64) -> Self {
        Self {
            code,
            flags: 0,
            context,
        }
    }

    pub const fn with_flags(code: bx_u32, flags: bx_u32) -> Self {
        Self {
            code,
            flags,
            context: 0,
        }
    }

    pub const fn is_ok(&self) -> bool {
        self.code == 0
    }
    pub const fn is_err(&self) -> bool {
        self.code != 0
    }

    /// Human-readable message for the error code.
    pub fn message(&self) -> &'static str {
        error_message(self.code)
    }

    /// Convert to `BmoStatus`, dropping context.
    pub const fn into_status(self) -> BmoStatus {
        BmoStatus {
            code: self.code,
            flags: self.flags,
            value: self.context,
        }
    }

    /// Re-wrap a `BmoStatus` into `BmoError`.
    pub const fn from_status(s: BmoStatus) -> Self {
        Self {
            code: s.code,
            flags: s.flags,
            context: s.value,
        }
    }
}

// --- Constructors convenience ---------------------------------------

impl BmoError {
    pub const fn out_of_memory() -> Self {
        Self::new(error_code::OUT_OF_MEMORY)
    }
    pub const fn invalid_argument() -> Self {
        Self::new(error_code::INVALID_ARGUMENT)
    }
    pub const fn not_implemented() -> Self {
        Self::new(error_code::UNSUPPORTED)
    }
    pub const fn io_error() -> Self {
        Self::new(error_code::IO)
    }
    pub const fn permission_denied() -> Self {
        Self::new(error_code::PERMISSION_DENIED)
    }
    pub const fn not_found() -> Self {
        Self::new(error_code::NOT_FOUND)
    }
    pub const fn bad_handle() -> Self {
        Self::new(error_code::INVALID_HANDLE)
    }
    pub const fn timeout() -> Self {
        Self::new(error_code::TIMEOUT)
    }
    pub const fn would_block() -> Self {
        Self::new(error_code::AGAIN)
    }
    pub const fn cancelled() -> Self {
        Self::new(error_code::CANCELLED)
    }
}
