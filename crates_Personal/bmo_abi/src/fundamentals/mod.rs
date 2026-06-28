//! `fundamentals` — tipos que TODO el código del BMO ABI necesita.
//!
//! Si un tipo se usa en más del 50% del código BMO, vive aquí.
//!
//! - [`primitives`] — tipos numéricos (`bx_u8..u64`, `bx_i*`, `bx_f16/32/64`).
//! - [`status`]     — `BmoStatus` 16-byte (sustituye `HRESULT`/`errno`).
//! - [`handle`]     — `BmoHandle` 64-bit con generación (sustituye `HANDLE`/`fd`).
//! - [`option`]     — `BmoOption<T>` FFI-safe (sustituye punteros nullable).
//! - [`result`]     — `BmoResult<T, E>` FFI-safe (errores inline sin TLS).
//! - [`error`]      — `BmoError` unificado de 16 bytes.
//! - [`convert`]    — conversiones BmoStatus ↔ BmoError ↔ ErrorCode.
//! - [`string`]     — `BmoStr`/`BmoString` (ptr+len UTF-8).
//! - [`memory`]     — `BmoSlice`, `BmoRange`, `BmoAligned`.
//! - [`io`]         — traits `BmoRead`/`BmoWrite`/`BmoSeek` + `BmoPipe`.
//! - [`fmt`]        — `BmoFormatter` stack-allocated (sin heap).
//! - [`sync`]       — `BmoAtomicU32/U64/Bool`, `MemOrder`, `BmoSpinLock`.

pub mod primitives;
pub mod status;
pub mod handle;
pub mod option;
pub mod result;
pub mod error;
pub mod convert;
pub mod string;
pub mod memory;
pub mod io;
pub mod fmt;
pub mod sync;
