//! `handle` -- handles opacos del BMO ABI.
//!
//! Reemplaza `HANDLE` (Win32), `int fd` (POSIX), `IUnknown*` (COM) con un
//! unico tipo `BmoHandle` 64-bit que incluye **generacion**: detecta UAF
//! por construccion.

pub mod kind;
pub mod opaque;
pub mod ops;

pub use opaque::BmoHandle;
