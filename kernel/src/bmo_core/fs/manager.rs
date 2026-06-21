//! VFS Manager for FastOS.
//!
//! Unified filesystem API that routes operations to the correct filesystem
//! driver based on the mount point.

#![allow(dead_code)]

use crate::dev::console;
use super::inode;
use super::mount;

const MAX_PATH: usize = 256;

/// Open a file by path. Returns a file descriptor or error.
pub fn open(path: &str) -> Result<u32, &'static str> {
    let mp = mount::find_mount(path).ok_or("path not mounted")?;

    let _rel_path = if path.len() > mp.path.len() {
        &path[mp.path.len()..]
    } else {
        "/"
    };

    match mp.fs_type {
        mount::FsType::RamFs => {
            let result = super::ramdisk::open(
                path.as_ptr() as u64,
                path.len() as u64,
            );
            if result == u64::MAX {
                Err("file not found")
            } else {
                Ok(result as u32)
            }
        }
        mount::FsType::Fat32 => {
            Err("FAT32: no disk driver wired")
        }
        mount::FsType::Exfat => {
            Err("exFAT: no disk driver wired")
        }
        mount::FsType::ProcFs | mount::FsType::DevFs => {
            let id = inode::InodeId::new(mp.mount_id, 1);
            inode::open(id, inode::InodeType::File, 0)
                .ok_or("inode table full")
        }
        mount::FsType::TmpFs => {
            let id = inode::InodeId::new(mp.mount_id, 1);
            inode::open(id, inode::InodeType::File, 0)
                .ok_or("inode table full")
        }
        mount::FsType::None => Err("no filesystem"),
    }
}

/// Read data from an open file descriptor.
pub fn read(fd: u32, buf: &mut [u8]) -> Result<usize, &'static str> {
    let open_inode = inode::get(fd).ok_or("bad fd")?;
    let mount_id = open_inode.id.mount_id;
    let mp = mount::get_mount(mount_id).ok_or("mount not found")?;

    match mp.fs_type {
        mount::FsType::RamFs => {
            let result = super::ramdisk::read(fd as u64, buf.as_mut_ptr() as u64, buf.len() as u64);
            if result == u64::MAX {
                Err("read error")
            } else {
                Ok(result as usize)
            }
        }
        mount::FsType::Fat32 => Err("FAT32: no disk driver wired"),
        mount::FsType::Exfat => Err("exFAT: no disk driver wired"),
        _ => Err("read not supported for this filesystem"),
    }
}

/// Close a file descriptor.
pub fn close(fd: u32) -> bool {
    inode::close(fd)
}

/// Get file size from an open file descriptor.
pub fn size(fd: u32) -> Option<u64> {
    inode::get(fd).map(|f| f.size)
}

/// Initialize the VFS.
pub fn init() {
    console::serial_write("[vfs] Initializing VFS...\n");

    // Root: RamFs (archivos embebidos del kernel)
    mount::mount(mount::FsType::RamFs, "/", 0, 0, true);

    // Virtual filesystems
    mount::mount(mount::FsType::ProcFs, "/proc", 0, 0, true);
    mount::mount(mount::FsType::DevFs, "/dev", 0, 0, false);
    mount::mount(mount::FsType::TmpFs, "/tmp", 0, 0, false);

    // FAT32 boot partition (when disk driver is wired)
    // mount::mount(mount::FsType::Fat32, "/boot", 0, 0, true);

    // exFAT data partition (when disk driver is wired)
    // mount::mount(mount::FsType::Exfat, "/data", 0, 0, false);

    console::serial_write("[vfs] VFS initialized\n");
}
