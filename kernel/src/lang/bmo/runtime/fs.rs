//! BMO Runtime — Filesystem operations.
//!
//! Wraps kernel RAMdisk filesystem API.

#![allow(dead_code)]

use super::error::{Error, Result};

/// File handle (descriptor).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FileHandle(pub u64);

/// Open a file by name. Returns handle.
pub fn open(name: &str) -> Result<FileHandle> {
    let name_ptr = name.as_ptr() as u64;
    let name_len = name.len() as u64;
    let fd = crate::bmo_core::fs::ramdisk::open(name_ptr, name_len);
    if fd == u64::MAX {
        Err(Error::NotFound)
    } else {
        Ok(FileHandle(fd))
    }
}

/// Read from file into buffer. Returns bytes read.
pub fn read(fd: FileHandle, buf: &mut [u8]) -> Result<usize> {
    let ptr = buf.as_mut_ptr() as u64;
    let len = buf.len() as u64;
    let n = crate::bmo_core::fs::ramdisk::read(fd.0, ptr, len);
    if n == u64::MAX {
        Err(Error::IoError)
    } else {
        Ok(n as usize)
    }
}

/// Close a file handle.
pub fn close(fd: FileHandle) -> Result<()> {
    let ret = crate::bmo_core::fs::ramdisk::close(fd.0);
    if ret == u64::MAX {
        Err(Error::BadHandle)
    } else {
        Ok(())
    }
}

/// Get file size.
pub fn size(fd: FileHandle) -> Result<u64> {
    let s = crate::bmo_core::fs::ramdisk::size(fd.0);
    if s == u64::MAX {
        Err(Error::BadHandle)
    } else {
        Ok(s)
    }
}

/// Read entire file into a pre-allocated buffer.
pub fn read_all(name: &str, buf: &mut [u8]) -> Result<usize> {
    let fd = open(name)?;
    let total = size(fd)? as usize;
    let to_read = if total > buf.len() { buf.len() } else { total };
    let n = read(fd, &mut buf[..to_read])?;
    let _ = close(fd);
    Ok(n)
}

/// Initialize filesystem subsystem.
pub fn init() {
    crate::bmo_core::diag::info("bmo_fs", "Filesystem subsystem initialized");
}
