//! Filesystem Service — Virtual File System.
//!
//! Provides a mount-based VFS layer over physical filesystems.
//! Currently supports ramdisk; exFAT/FAT32 via AHCI planned.
//!
//! ## Architecture
//!
//! ```text
//! VFS → MountPoint → FS Driver (ramdisk | exFAT | FAT32)
//! ```

#![no_std]

extern crate alloc;

use alloc::string::String;
use alloc::vec::Vec;

/// FS error codes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FsError {
    NotFound,
    PermissionDenied,
    NotSupported,
    IoError,
    AlreadyExists,
    IsDirectory,
    NotDirectory,
}

/// File type.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FileType {
    File,
    Directory,
}

/// Directory entry.
#[derive(Debug, Clone)]
pub struct DirEntry {
    pub name: String,
    pub file_type: FileType,
    pub size: u64,
}

/// Mount point in the VFS tree.
pub struct MountPoint {
    pub path: String,
    pub fs_type: FsType,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FsType {
    Ramdisk,
    ExFAT,
    FAT32,
}

/// Virtual File System root.
pub struct Vfs {
    mounts: Vec<MountPoint>,
    initialized: bool,
}

impl Vfs {
    pub const fn new() -> Self {
        Self { mounts: Vec::new(), initialized: false }
    }

    /// Initialize the VFS: mount ramdisk as root.
    pub fn init(&mut self) {
        if self.initialized { return; }
        self.initialized = true;
        self.mounts.push(MountPoint {
            path: String::from("/"),
            fs_type: FsType::Ramdisk,
        });
    }

    /// Mount a filesystem at a path.
    pub fn mount(&mut self, path: &str, fs_type: FsType) -> Result<(), FsError> {
        if self.mounts.iter().any(|m| m.path.as_str() == path) {
            return Err(FsError::AlreadyExists);
        }
        self.mounts.push(MountPoint {
            path: String::from(path),
            fs_type,
        });
        Ok(())
    }

    /// List mount points.
    pub fn mounts(&self) -> &[MountPoint] {
        &self.mounts
    }

    /// Check if a path exists.
    pub fn exists(&self, _path: &str) -> bool {
        // Ramdisk-only: all paths exist
        self.initialized
    }

    /// Read directory entries at path.
    pub fn read_dir(&self, _path: &str) -> Result<Vec<DirEntry>, FsError> {
        if !self.initialized { return Err(FsError::NotFound); }
        Ok(Vec::new())
    }
}

/// Global VFS instance.
static mut VFS: Vfs = Vfs::new();

/// Initialize the global VFS.
pub fn init() {
    unsafe { VFS.init(); }
}

/// Mount a filesystem.
pub fn mount(path: &str, fs_type: FsType) -> Result<(), FsError> {
    unsafe { VFS.mount(path, fs_type) }
}

/// Read a directory.
pub fn read_dir(path: &str) -> Result<Vec<DirEntry>, FsError> {
    unsafe { VFS.read_dir(path) }
}
