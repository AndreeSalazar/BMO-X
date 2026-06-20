//! `fundamentals` — tipos que TODO el código del BMO ABI necesita.
//!
//! Si un tipo se usa en más del 50% del código BMO, vive aquí.
//!
//! - [`primitives`] — tipos numéricos (`bx_u8..u64`, `bx_i*`, `bx_f16/32/64`).
//! - [`status`]     — `BmoStatus` + `BxError` (sustituye `HRESULT`/`errno`).
//! - [`handle`]     — `BmoHandle` 64-bit con generación (sustituye `HANDLE`/`fd`).
//! - [`option`]     — `BmoOption<T>` FFI-safe.
//! - [`result`]     — `BmoResult<T>` FFI-safe.
//! - [`memory`]     — slices, ranges, alignment helpers (sustituye `void*` + `size_t`).
//!
//! ## Filosofía
//!
//! Estos tipos son la "alfabetización" del BMO ABI. Todo lo demás se
//! construye encima de ellos. Si dudas dónde poner un nuevo tipo, mira
//! si depende solo de aquí: si sí, también va aquí.

pub mod primitives;
pub mod status;
pub mod handle;
pub mod option;
pub mod result;
pub mod memory;
pub mod sync;
