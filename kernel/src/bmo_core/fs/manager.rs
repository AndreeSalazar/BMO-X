#![allow(dead_code)]

//! VFS Manager for FastOS.
//!
//! Unified filesystem API that routes operations to the correct filesystem
//! driver based on the mount point.

use crate::dev::console;
use super::inode;
use super::mount;

/// Maximum path length.
const MAX_PATH: usize = 256;

/// Open a file by path. Returns a file descriptor or error.
pub fn open(path: &str) -> Result<u32, &'static str> {
    // Resolve mount point
    let mp = mount::find_mount(path).ok_or("path not mounted")?;

    // Strip mount prefix from path to get relative path
    let _rel_path = if path.len() > mp.path.len() {
        &path[mp.path.len()..]
    } else {
        "/"
    };

    match mp.fs_type {
        mount::FsType::BmoFs => {
            // Delegate to BMO-FS via ramdisk file lookup
            let full_path = path;
            let result = super::ramdisk::open(
                full_path.as_ptr() as u64,
                full_path.len() as u64,
            );
            if result == u64::MAX {
                Err("file not found")
            } else {
                Ok(result as u32)
            }
        }
        mount::FsType::Fat32 => {
            // FAT32 — read-only, requires disk driver
            // TODO: wire FAT32 parse + locate_file + cluster read
            Err("FAT32 not wired yet")
        }
        mount::FsType::ProcFs | mount::FsType::DevFs => {
            // Virtual filesystems — handled via inode
            let id = inode::InodeId::new(mp.mount_id, 1);
            inode::open(id, inode::InodeType::File, 0)
                .ok_or("inode table full")
        }
        mount::FsType::TmpFs => {
            // Temporary filesystem (RAM-backed)
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
        mount::FsType::BmoFs => {
            let result = super::ramdisk::read(fd as u64, buf.as_mut_ptr() as u64, buf.len() as u64);
            if result == u64::MAX {
                Err("read error")
            } else {
                Ok(result as usize)
            }
        }
        mount::FsType::Fat32 => {
            // TODO: read from FAT32 via cluster chain
            Err("FAT32 read not wired yet")
        }
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
/// Mounts the root filesystem and virtual filesystems.
pub fn init() {
    console::serial_write("[vfs] Initializing VFS...\n");

    // Mount root BMO-FS at "/"
    mount::mount(mount::FsType::BmoFs, "/", 0, 0, true);

    // Mount procfs at "/proc"
    mount::mount(mount::FsType::ProcFs, "/proc", 0, 0, true);

    // Mount devfs at "/dev"
    mount::mount(mount::FsType::DevFs, "/dev", 0, 0, false);

    // Mount tmpfs at "/tmp"
    mount::mount(mount::FsType::TmpFs, "/tmp", 0, 0, false);

    console::serial_write("[vfs] VFS initialized\n");
}
