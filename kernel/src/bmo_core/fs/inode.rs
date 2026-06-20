#![allow(dead_code)]

//! VFS Inode abstraction for FastOS.

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

/// Inode type (file, directory, symlink, etc.)
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum InodeType {
    File,
    Directory,
    SymLink,
    BlockDevice,
    CharDevice,
}

/// Permission bits.
#[derive(Debug, Clone, Copy)]
pub struct Perm {
    pub owner_r: bool,
    pub owner_w: bool,
    pub owner_x: bool,
    pub group_r: bool,
    pub group_w: bool,
    pub group_x: bool,
    pub other_r: bool,
    pub other_w: bool,
    pub other_x: bool,
}

impl Perm {
    pub const fn rw() -> Self {
        Self {
            owner_r: true, owner_w: true, owner_x: false,
            group_r: true, group_w: false, group_x: false,
            other_r: true, other_w: false, other_x: false,
        }
    }

    pub const fn rwx() -> Self {
        Self {
            owner_r: true, owner_w: true, owner_x: true,
            group_r: true, group_w: false, group_x: true,
            other_r: true, other_w: false, other_x: true,
        }
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
            itype: InodeType::File,
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
