#![allow(dead_code)]

//! VFS Inode abstraction for FastOS.
//!
//! v1.8.8: usa `BmoFileType` y `BmoPerms` del ABI. Antes redefinía
//! `InodeType` y `Perm` con layouts incompatibles. Ahora son
//! type aliases del contrato.

use crate::bmo_abi::fs::{BmoFileType, BmoPerms};

/// Maximum open inodes system-wide.
const MAX_OPEN_INODES: usize = 256;

/// Inode number type.
pub type InoNum = u64;

/// Unique inode identifier across all mounts.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct InodeId {
    pub mount_id: u16,
    pub ino: InoNum,
}

impl InodeId {
    pub const fn new(mount_id: u16, ino: InoNum) -> Self {
        Self { mount_id, ino }
    }

    pub const ROOT: Self = Self { mount_id: 0, ino: 1 };
}

/// Inode type — alias del ABI `BmoFileType`.
/// 7 variantes: Unknown/Regular/Directory/Symlink/Character/Block/Pipe/Socket.
pub type InodeType = BmoFileType;

/// Permission bits — alias del ABI `BmoPerms` (u16 Unix-like).
pub type Perm = BmoPerms;

impl Perm {
    /// Permisos `rw-r--r--` (0644).
    pub const fn rw() -> Self {
        Self(0o644)
    }

    /// Permisos `rwxr-xr-x` (0755).
    pub const fn rwx() -> Self {
        Self(0o755)
    }
}

/// Open file handle.
#[derive(Debug, Clone, Copy)]
pub struct OpenInode {
    pub id: InodeId,
    pub itype: InodeType,
    pub size: u64,
    pub offset: u64,
    pub perm: Perm,
    pub in_use: bool,
}

impl OpenInode {
    pub const fn empty() -> Self {
        Self {
            id: InodeId::ROOT,
            itype: InodeType::Regular,
            size: 0,
            offset: 0,
            perm: Perm::rw(),
            in_use: false,
        }
    }
}

/// Global open inode table.
static mut OPEN_INODES: [OpenInode; MAX_OPEN_INODES] = [OpenInode::empty(); MAX_OPEN_INODES];

/// Open an inode. Returns a file descriptor (index into open table).
pub fn open(id: InodeId, itype: InodeType, size: u64) -> Option<u32> {
    unsafe {
        for i in 0..MAX_OPEN_INODES {
            if !OPEN_INODES[i].in_use {
                OPEN_INODES[i] = OpenInode {
                    id,
                    itype,
                    size,
                    offset: 0,
                    perm: Perm::rw(),
                    in_use: true,
                };
                return Some(i as u32);
            }
        }
    }
    None
}

/// Close a file descriptor.
pub fn close(fd: u32) -> bool {
    let fd = fd as usize;
    if fd >= MAX_OPEN_INODES {
        return false;
    }
    unsafe {
        if OPEN_INODES[fd].in_use {
            OPEN_INODES[fd].in_use = false;
            return true;
        }
    }
    false
}

/// Get an open inode by file descriptor.
pub fn get(fd: u32) -> Option<&'static mut OpenInode> {
    let fd = fd as usize;
    if fd >= MAX_OPEN_INODES {
        return None;
    }
    unsafe {
        if OPEN_INODES[fd].in_use {
            Some(&mut OPEN_INODES[fd])
        } else {
            None
        }
    }
}

/// Count of open file descriptors.
pub fn open_count() -> u32 {
    unsafe {
        OPEN_INODES.iter().filter(|f| f.in_use).count() as u32
    }
}
