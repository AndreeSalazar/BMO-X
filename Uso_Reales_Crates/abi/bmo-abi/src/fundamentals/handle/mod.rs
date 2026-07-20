//! `handle` — handles opacos del BMO ABI.
//!
//! Reemplaza `HANDLE` (Win32), `int fd` (POSIX), `IUnknown*` (COM) con un
//! único tipo `BmoHandle` 64-bit que incluye **generación**: detecta UAF
//! por construcción.

pub mod kind;
pub mod opaque;
pub mod ops;

pub use opaque::BmoHandle;
