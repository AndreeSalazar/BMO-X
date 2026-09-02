//! `ops` -- operaciones sobre `BmoHandle`: duplicate, close, wait.
//!
//! Este modulo define el trait `BmoHandleOps` que todo backend de handles
//! (kernel, driver, biblioteca) debe implementar.
//!
//! -- EL SEMAFORO (L6g) y las dos preguntas de antes (L6e, L6f) --------
//!
//! Que cuesta que falle, por que falla ESTA pieza, y que arrastro si la
//! toco. La ley esta en `META-KERNEL_HARD.md`.
//!
//! [carril]  ROJO         las operaciones que se piden con un handle:
//!                        superficie congelada
//! [cuesta]  PUERTA       cambiar un numero rompe binarios ya firmados
//! [riesgo]  UNICO        un opcode se elige una vez. `MEMORIA_PEDIR` en el
//!                        0x12 de `REINICIAR` habria reiniciado la maquina

use crate::bmo_abi::fundamentals::handle::BmoHandle;
use crate::bmo_abi::fundamentals::status::BmoStatus;
use crate::bmo_abi::primitives::{bx_u32, bx_u64};

/// Resultado de `duplicate`.
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct BmoDuplicateResult {
    pub status: BmoStatus,
    pub new_handle: BmoHandle,
}
const _: () = assert!(core::mem::size_of::<BmoDuplicateResult>() == 24);

/// Resultado de `wait`.
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct BmoWaitResult {
    pub status: BmoStatus,
    pub signaled_handle: BmoHandle,
}
const _: () = assert!(core::mem::size_of::<BmoWaitResult>() == 24);

/// Operaciones sobre handles.
///
/// Reemplaza `DuplicateHandle`, `CloseHandle`, `WaitForSingleObject`
/// de Win32 y `dup`/`close`/`poll` de POSIX.
pub trait BmoHandleOps {
    /// Duplicate a handle, optionally with new capabilities.
    fn duplicate(&self, handle: BmoHandle, access_mask: bx_u64) -> BmoDuplicateResult;

    /// Close a handle, releasing its resources.
    fn close(&mut self, handle: BmoHandle) -> BmoStatus;

    /// Wait for a handle to become signaled (ready).
    fn wait(&self, handle: BmoHandle, timeout_us: bx_u64) -> BmoWaitResult;

    /// Wait for any of an array of handles.
    fn wait_any(&self, handles: &[BmoHandle], timeout_us: bx_u64) -> BmoWaitResult;

    /// Query handle info: type, capabilities, reference count.
    fn query(&self, handle: BmoHandle) -> BmoHandleInfo;
}

/// Informacion consultable de un handle.
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct BmoHandleInfo {
    pub handle: BmoHandle,
    pub kind: bx_u64,
    pub capabilities: bx_u64,
    pub ref_count: bx_u32,
    pub is_signaled: bool,
}
