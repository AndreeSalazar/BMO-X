//! BMO std::fs — Sistema de archivos.

#![allow(dead_code)]

use crate::lang::bmo::runtime::fs as rt;
use alloc::string::String;

pub fn open(name: &str) -> Option<u64> { rt::open(name).ok().map(|h| h.0) }
pub fn read(fd: u64, buf: &mut [u8]) -> Option<usize> { rt::read(crate::lang::bmo::runtime::fs::FileHandle(fd), buf).ok() }
pub fn close(fd: u64) { let _ = rt::close(crate::lang::bmo::runtime::fs::FileHandle(fd)); }
pub fn size(fd: u64) -> Option<u64> { rt::size(crate::lang::bmo::runtime::fs::FileHandle(fd)).ok() }
pub fn read_file(name: &str) -> Option<String> {
    let fd = open(name)?; let sz = size(fd)? as usize;
    let mut buf = alloc::vec![0u8; sz]; let n = read(fd, &mut buf)?; close(fd);
    buf.truncate(n); String::from_utf8(buf).ok()
}
