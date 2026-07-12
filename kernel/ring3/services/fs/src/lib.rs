#![no_std]
#![allow(static_mut_refs)]

extern crate alloc;

pub mod inode;
pub mod mount;
pub mod ramdisk;
pub mod ramdisk_device;

use bmo_abi::error_code::BmoErrorCode;
use bmo_abi::fs::BmoFileType;

pub type FsError = BmoErrorCode;

pub fn init() {
    mount::mount(mount::FsType::RamFs, "/", 0, 0, true);
}

pub struct FsResult;

impl FsResult {
    pub fn open(path: &str) -> Result<u32, FsError> {
        let file_idx = ramdisk::find(path).ok_or(FsError::NotFound)?;
        let size = ramdisk::file_size(file_idx);
        let id = inode::InodeId::new(0, file_idx as u64 + 1);
        let fd = inode::open(id, BmoFileType::Regular, size as u64)
            .ok_or(FsError::OutOfMemory)?;
        Ok(fd)
    }

    pub fn close(fd: u32) -> bool {
        inode::close(fd)
    }

    pub fn read(fd: u32, buf: &mut [u8]) -> Result<usize, FsError> {
        let entry = inode::get(fd).ok_or(FsError::InvalidHandle)?;
        let file_idx = (entry.id.ino - 1) as usize;
        let n = ramdisk::read_at(file_idx, entry.offset, buf);
        entry.offset += n as u64;
        Ok(n)
    }

    pub fn write(_fd: u32, _data: &[u8]) -> Result<usize, FsError> {
        Err(FsError::Unsupported)
    }

    pub fn seek(fd: u32, offset: i64, whence: u32) -> Result<u64, FsError> {
        let entry = inode::get(fd).ok_or(FsError::InvalidHandle)?;
        let file_size = entry.size;
        let new_pos = match whence {
            0 => offset.max(0) as u64,
            1 => (entry.offset as i64).saturating_add(offset).max(0) as u64,
            2 => file_size.saturating_sub(offset.unsigned_abs()),
            _ => return Err(FsError::InvalidArgument),
        };
        let new_pos = new_pos.min(file_size);
        entry.offset = new_pos;
        Ok(new_pos)
    }

    pub fn size(fd: u32) -> Result<u64, FsError> {
        let entry = inode::get(fd).ok_or(FsError::InvalidHandle)?;
        Ok(entry.size)
    }

    pub fn exists(path: &str) -> bool {
        ramdisk::find(path).is_some()
    }

    pub fn read_dir(_path: &str) -> Result<alloc::vec::Vec<alloc::string::String>, FsError> {
        let mut entries = alloc::vec::Vec::new();
        for f in ramdisk::RAMDISK_FILES.iter() {
            entries.push(alloc::string::String::from(f.name));
        }
        Ok(entries)
    }
}
