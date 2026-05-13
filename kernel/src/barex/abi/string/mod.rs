//! `string` — strings BMO ABI. UTF-8 universal, sin `\0`-terminator.
//!
//! Reemplaza `<string.h>`, `<wchar.h>` y todo el zoo `char*`/`wchar_t*`/
//! `LPCSTR`/`LPCWSTR`/`BSTR` de Win32.
//!
//! - **`BmoStr`**    — string slice borrowed (`(ptr, len)`).
//! - **`BmoString`** — owned (heap-allocated, requiere `alloc::Vec<u8>`).
//! - **UTF-8 only.** UTF-16 está PROHIBIDO en la API pública. La conversión
//!   sólo existe en `compat` para hablar con código Win32 heredado.

pub mod bx_str;
pub mod ascii;

pub use bx_str::{BmoStr, BmoString};
