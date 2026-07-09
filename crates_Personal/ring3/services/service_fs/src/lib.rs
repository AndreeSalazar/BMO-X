//! Filesystem Service — Virtual File System.
//!
//! Provides a mount-based VFS layer over physical filesystems.
//! Currently supports ramdisk; exFAT/FAT32 via AHCI planned.

#![no_std]

extern crate alloc;

use alloc::string::String;
use alloc::vec::Vec;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FsError {
    NotFound,
    PermissionDenied,
    NotSupported,
    IoError,
    AlreadyExists,
    IsDirectory,
    NotDirectory,
    BufferTooSmall,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FileType { File, Directory }

#[derive(Debug, Clone)]
pub struct DirEntry {
    pub name: String,
    pub file_type: FileType,
    pub size: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FsType { Ramdisk, ExFAT, FAT32 }

pub struct MountPoint {
    pub path: String,
    pub fs_type: FsType,
}

pub struct Vfs {
    mounts: Vec<MountPoint>,
    initialized: bool,
}

impl Vfs {
    pub const fn new() -> Self {
        Self { mounts: Vec::new(), initialized: false }
    }

    pub fn init(&mut self) {
        if self.initialized { return; }
        self.initialized = true;
        self.mounts.push(MountPoint { path: String::from("/"), fs_type: FsType::Ramdisk });
    }

    pub fn mount(&mut self, path: &str, fs_type: FsType) -> Result<(), FsError> {
        if self.mounts.iter().any(|m| m.path.as_str() == path) {
            return Err(FsError::AlreadyExists);
        }
        self.mounts.push(MountPoint { path: String::from(path), fs_type });
        Ok(())
    }

    pub fn mounts(&self) -> &[MountPoint] { &self.mounts }

    /// Read a file into buffer. Returns bytes read.
    pub fn read(&self, _path: &str, _buf: &mut [u8]) -> Result<usize, FsError> {
        if !self.initialized { return Err(FsError::NotFound); }
        // Ramdisk pass-through: delegate to kernel ramdisk via future syscall
        // For now, return 0 bytes (file exists but empty)
        Ok(0)
    }

    /// Write a buffer to a file. Returns bytes written.
    pub fn write(&self, _path: &str, _data: &[u8]) -> Result<usize, FsError> {
        if !self.initialized { return Err(FsError::NotFound); }
        Err(FsError::NotSupported) // ramdisk is read-only
    }

    /// Create a file.
    pub fn create(&self, _path: &str, _ft: FileType) -> Result<(), FsError> {
        Err(FsError::NotSupported)
    }

    /// Delete a file.
    pub fn delete(&self, _path: &str) -> Result<(), FsError> {
        Err(FsError::NotSupported)
    }

    pub fn exists(&self, _path: &str) -> bool { self.initialized }

    pub fn read_dir(&self, _path: &str) -> Result<Vec<DirEntry>, FsError> {
        if !self.initialized { return Err(FsError::NotFound); }
        Ok(Vec::new())
    }
}

static mut VFS: Vfs = Vfs::new();

pub fn init() { unsafe { VFS.init(); } }
pub fn mount(path: &str, fs_type: FsType) -> Result<(), FsError> { unsafe { VFS.mount(path, fs_type) } }
pub fn read(path: &str, buf: &mut [u8]) -> Result<usize, FsError> { unsafe { VFS.read(path, buf) } }
pub fn write(path: &str, data: &[u8]) -> Result<usize, FsError> { unsafe { VFS.write(path, data) } }
pub fn create(path: &str, ft: FileType) -> Result<(), FsError> { unsafe { VFS.create(path, ft) } }
pub fn delete(path: &str) -> Result<(), FsError> { unsafe { VFS.delete(path) } }
pub fn exists(path: &str) -> bool { unsafe { VFS.exists(path) } }
pub fn read_dir(path: &str) -> Result<Vec<DirEntry>, FsError> { unsafe { VFS.read_dir(path) } }
