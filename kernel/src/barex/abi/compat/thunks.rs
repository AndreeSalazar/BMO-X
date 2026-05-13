//! Trampolines C ABI ↔ BMO ABI.
//!
//! Marcadores y helpers — la materialización real ocurre vía atributos
//! en codegen o en `extern "C"` envueltos.

use crate::barex::abi::primitives::bx_usize;

/// Marca conceptual de "esta función debe llamarse con MS x64 ABI".
/// En Rust nightly se materializa con `extern "win64"`.
#[allow(non_camel_case_types)]
pub struct MsX64Marker;

/// Marca conceptual de "esta función debe llamarse con SysV AMD64 ABI".
/// En Rust nightly se materializa con `extern "sysv64"`.
#[allow(non_camel_case_types)]
pub struct SysVMarker;

/// Wrapper retórico — adopta el contrato MS x64 sobre el callee.
///
/// Uso real (nightly):
/// ```ignore
/// extern "win64" fn legacy_dx12_callback(arg: u32) -> u32 { ... }
/// ```
#[inline(always)]
pub fn wrap_msx64<F: Fn() -> u64>(f: F) -> u64 { f() }

/// Wrapper retórico — adopta el contrato SysV AMD64 sobre el callee.
#[inline(always)]
pub fn wrap_sysv<F: Fn() -> u64>(f: F) -> u64 { f() }

/// Tamaño de "shadow space" requerido cuando se llama código MS x64
/// desde código BMO. **Debe reservarse antes del `call`.**
pub const MSX64_SHADOW_SPACE: bx_usize = 32;

/// Stack alignment requerido por SysV antes de `call`.
pub const SYSV_STACK_ALIGNMENT: bx_usize = 16;

/// Stack alignment requerido por MS x64 antes de `call`.
pub const MSX64_STACK_ALIGNMENT: bx_usize = 16;
