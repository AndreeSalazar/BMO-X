//! `io` — I/O abstractions del BMO ABI.
//!
//! Reemplaza `FILE*`, `HANDLE`, `fd` con tipos unificados:
//! - `BmoFileHandle` — handle genérico para archivos/pipes/sockets
//! - `BmoPipe` — pipe unidireccional (inter-process communication)
//! - `BmoSeekMode` — modo de seek (reemplaza `SEEK_SET/CUR/END`)

#![allow(dead_code)]

use crate::bmo_core::bmo_abi::primitives::{bx_u32, bx_u64, bx_usize};
use crate::bmo_core::bmo_abi::handle::opaque::BmoHandle;

#[repr(u32)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BmoSeekMode {
    Start = 0,
    Current = 1,
    End = 2,
}

#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct BmoFileHandle {
    pub handle: BmoHandle,
    pub flags: bx_u32,
}

impl BmoFileHandle {
    pub const fn new(handle: BmoHandle) -> Self {
        Self { handle, flags: 0 }
    }

    pub const fn is_valid(&self) -> bool {
        !self.handle.is_null()
    }

    pub const fn from_raw(raw: bx_u64) -> Self {
        Self { handle: BmoHandle(raw), flags: 0 }
    }
}

pub const STDIN:  BmoFileHandle = BmoFileHandle { handle: BmoHandle::NULL, flags: 1 };
pub const STDOUT: BmoFileHandle = BmoFileHandle { handle: BmoHandle::NULL, flags: 2 };
pub const STDERR: BmoFileHandle = BmoFileHandle { handle: BmoHandle::NULL, flags: 3 };

#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct BmoPipe {
    pub read_end: BmoHandle,
    pub write_end: BmoHandle,
}

impl BmoPipe {
    pub const fn new(read_end: BmoHandle, write_end: BmoHandle) -> Self {
        Self { read_end, write_end }
    }

    pub const fn is_valid(&self) -> bool {
        !self.read_end.is_null() && !self.write_end.is_null()
    }
}

pub trait BmoRead {
    fn read(&mut self, buf: &mut [u8]) -> Result<bx_usize, crate::bmo_core::bmo_abi::fundamentals::error::BmoError>;
}

pub trait BmoWrite {
    fn write(&mut self, buf: &[u8]) -> Result<bx_usize, crate::bmo_core::bmo_abi::fundamentals::error::BmoError>;
    fn flush(&mut self) -> Result<(), crate::bmo_core::bmo_abi::fundamentals::error::BmoError>;
}

pub trait BmoSeek {
    fn seek(&mut self, offset: bx_u64, mode: BmoSeekMode) -> Result<bx_u64, crate::bmo_core::bmo_abi::fundamentals::error::BmoError>;
}

pub fn stdin() -> BmoFileHandle { STDIN }
pub fn stdout() -> BmoFileHandle { STDOUT }
pub fn stderr() -> BmoFileHandle { STDERR }
