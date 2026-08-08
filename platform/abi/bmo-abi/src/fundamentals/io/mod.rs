//! `io` -- traits de E/S del BMO ABI.
//!
//! Reemplaza `<stdio.h>` de C con un triplete de traits: Read, Write, Seek.
//! Cada uno devuelve `BmoStatus` para composicion FFI-safe.

use crate::bmo_abi::fundamentals::memory::BmoSliceMut;
use crate::bmo_abi::fundamentals::status::BmoStatus;
use crate::bmo_abi::primitives::{bx_i64, bx_u64};

/// Resultado de una operacion Read.
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct ReadResult {
    pub status: BmoStatus,
    pub bytes_read: bx_u64,
}
const _: () = assert!(core::mem::size_of::<ReadResult>() == 24);

/// Resultado de una operacion Write.
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct WriteResult {
    pub status: BmoStatus,
    pub bytes_written: bx_u64,
}
const _: () = assert!(core::mem::size_of::<WriteResult>() == 24);

/// Resultado de una operacion Seek.
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct SeekResult {
    pub status: BmoStatus,
    pub new_offset: bx_u64,
}
const _: () = assert!(core::mem::size_of::<SeekResult>() == 24);

/// Trait de lectura. Reemplaza `read()` / `fread()` de C.
pub trait BmoRead {
    /// Read bytes into `buf`. Returns status + bytes read.
    fn read(&mut self, buf: BmoSliceMut) -> ReadResult;
}

/// Trait de escritura. Reemplaza `write()` / `fwrite()` de C.
pub trait BmoWrite {
    /// Write bytes from `buf`. Returns status + bytes written.
    fn write(&mut self, buf: &[u8]) -> WriteResult;

    /// Flush any internal buffers.
    fn flush(&mut self) -> BmoStatus;
}

/// Origen del seek.
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BmoSeekFrom {
    Start(bx_u64),
    End(bx_i64),
    Current(bx_i64),
}

/// Trait de seek. Reemplaza `fseek()` / `lseek()` de C.
pub trait BmoSeek {
    fn seek(&mut self, from: BmoSeekFrom) -> SeekResult;
}

// --- Pipe de un solo sentido (Ring 0 <-> Ring 3) ---------------------

/// Pipe unidireccional con buffer circular interno.
/// Disenado para comunicacion simple sin syscalls costosas.
pub struct BmoPipe {
    buf: [u8; 4096],
    read_pos: usize,
    write_pos: usize,
    full: bool,
}

impl BmoPipe {
    pub const fn new() -> Self {
        Self {
            buf: [0u8; 4096],
            read_pos: 0,
            write_pos: 0,
            full: false,
        }
    }

    fn len(&self) -> usize {
        if self.full {
            4096
        } else if self.write_pos >= self.read_pos {
            self.write_pos - self.read_pos
        } else {
            4096 - self.read_pos + self.write_pos
        }
    }

    fn is_empty(&self) -> bool {
        !self.full && self.read_pos == self.write_pos
    }
}

impl BmoRead for BmoPipe {
    fn read(&mut self, buf: BmoSliceMut) -> ReadResult {
        let buf_len = buf.len as usize;
        let avail = self.len();
        let n = avail.min(buf_len);
        if n == 0 {
            return ReadResult {
                status: BmoStatus::OK,
                bytes_read: 0,
            };
        }
        let slice = unsafe { core::slice::from_raw_parts_mut(buf.ptr, buf_len) };
        for i in 0..n {
            slice[i] = self.buf[self.read_pos];
            self.read_pos = (self.read_pos + 1) % 4096;
        }
        if self.full {
            self.full = false;
        }
        ReadResult {
            status: BmoStatus::OK,
            bytes_read: n as bx_u64,
        }
    }
}

impl BmoWrite for BmoPipe {
    fn write(&mut self, buf: &[u8]) -> WriteResult {
        let avail = if self.full {
            0
        } else {
            let used = self.len();
            4096 - used
        };
        let n = buf.len().min(avail);
        for i in 0..n {
            self.buf[self.write_pos] = buf[i];
            self.write_pos = (self.write_pos + 1) % 4096;
        }
        if n == 0 && !buf.is_empty() {
            return WriteResult {
                status: BmoStatus::err_with_flags(1, 1 << 0),
                bytes_written: 0,
            };
        }
        if self.len() == 4096 {
            self.full = true;
        }
        WriteResult {
            status: BmoStatus::OK,
            bytes_written: n as bx_u64,
        }
    }

    fn flush(&mut self) -> BmoStatus {
        BmoStatus::OK
    }
}
