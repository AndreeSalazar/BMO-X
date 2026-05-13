//! `status` — códigos de retorno BMO ABI.
//!
//! `BmoStatus` reemplaza:
//!   - Win32 `HRESULT` (32-bit) + `GetLastError()` (TLS global)
//!   - POSIX `errno` (TLS global)
//!   - COM `IUnknown` reference counts mezclados con error codes
//!
//! Todo viaja **por valor** en `RAX:RDX` (16 bytes). Cero acceso a memoria.

pub mod code;
pub mod error;

pub use code::{BmoStatus, StatusFlags};
pub use error::ErrorCode;
