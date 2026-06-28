//! `ops` — operaciones sobre `BmoHandle`: duplicate, close, wait.
//!
//! Este módulo define el trait `BmoHandleOps` que todo backend de handles
//! (kernel, driver, biblioteca) debe implementar.

use crate::bmo_abi::primitives::{bx_u32, bx_u64};
use crate::bmo_abi::fundamentals::status::BmoStatus;
use crate::bmo_abi::fundamentals::handle::BmoHandle;

/// Resultado de `duplicate`.
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct BmoDuplicateResult {
    pub status: BmoStatus,
    pub new_handle: BmoHandle,
}

/// Resultado de `wait`.
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct BmoWaitResult {
    pub status: BmoStatus,
    pub signaled_handle: BmoHandle,
}

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

/// Información consultable de un handle.
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct BmoHandleInfo {
    pub handle: BmoHandle,
    pub kind: bx_u64,
    pub capabilities: bx_u64,
    pub ref_count: bx_u32,
    pub is_signaled: bool,
}
