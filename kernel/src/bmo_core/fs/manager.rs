//! VFS Manager for FastOS.
//!
//! Unified filesystem API that routes operations to the correct filesystem
//! driver based on the mount point.

#![allow(dead_code)]

use crate::dev::console;
use super::inode;
use super::mount;
use super::ramdisk_device::RamDiskDevice;

const MAX_PATH: usize = 256;

static mut GLOBAL_DISK: Option<RamDiskDevice> = None;

fn get_disk() -> &'static mut RamDiskDevice {
    unsafe {
        if GLOBAL_DISK.is_none() {
            let mut dev = RamDiskDevice::new();
            dev.init_with_exfat_image();
            GLOBAL_DISK = Some(dev);
        }
        GLOBAL_DISK.as_mut().unwrap()
    }
}

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
            Err("FAT32: not wired")
        }
        mount::FsType::Exfat => {
            let disk = get_disk();
            match super::exfat::open_file(path, disk) {
                Ok(fd) => {
                    let id = inode::InodeId::new(mp.mount_id, fd as u64);
                    inode::open(id, inode::InodeType::Regular, 0)
                        .ok_or("inode table full")
                }
                Err(e) => Err(e.as_str()),
            }
        }
        mount::FsType::ProcFs | mount::FsType::DevFs => {
            let id = inode::InodeId::new(mp.mount_id, 1);
            inode::open(id, inode::InodeType::Regular, 0)
                .ok_or("inode table full")
        }
        mount::FsType::TmpFs => {
            let id = inode::InodeId::new(mp.mount_id, 1);
            inode::open(id, inode::InodeType::Regular, 0)
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
        mount::FsType::Fat32 => Err("FAT32: not wired"),
        mount::FsType::Exfat => {
            let disk = get_disk();
            let exfat_fd = open_inode.id.ino;
            match super::exfat::read_file(exfat_fd as u32, buf, disk) {
                Ok(n) => Ok(n),
                Err(e) => Err(e.as_str()),
            }
        }
        _ => Err("read not supported for this filesystem"),
    }
}

/// Write data to an open file descriptor.
pub fn write(fd: u32, buf: &[u8]) -> Result<usize, &'static str> {
    let open_inode = inode::get(fd).ok_or("bad fd")?;
    let mount_id = open_inode.id.mount_id;
    let mp = mount::get_mount(mount_id).ok_or("mount not found")?;

    match mp.fs_type {
        mount::FsType::RamFs => Err("ramdisk is read-only"),
        mount::FsType::Fat32 => Err("FAT32: not wired"),
        mount::FsType::Exfat => {
            let disk = get_disk();
            let exfat_fd = open_inode.id.ino;
            match super::exfat::write_file(exfat_fd as u32, buf, disk) {
                Ok(n) => Ok(n),
                Err(e) => Err(e.as_str()),
            }
        }
        _ => Err("write not supported for this filesystem"),
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

/// Create a new file in the exFAT data partition.
pub fn create(path: &str) -> Result<u32, &'static str> {
    let mp = mount::find_mount(path).ok_or("path not mounted")?;
    match mp.fs_type {
        mount::FsType::Exfat => {
            let disk = get_disk();
            let name = if path.len() > mp.path.len() {
                &path[mp.path.len()..]
            } else {
                path
            };
            let name = name.trim_start_matches('/');
            match super::exfat::create_file(name, disk) {
                Ok(fd) => {
                    let id = inode::InodeId::new(mp.mount_id, fd as u64);
                    inode::open(id, inode::InodeType::Regular, 0)
                        .ok_or("inode table full")
                }
                Err(e) => Err(e.as_str()),
            }
        }
        _ => Err("create not supported"),
    }
}

/// Delete a file from the exFAT data partition.
pub fn delete(path: &str) -> Result<(), &'static str> {
    let mp = mount::find_mount(path).ok_or("path not mounted")?;
    match mp.fs_type {
        mount::FsType::Exfat => {
            let disk = get_disk();
            let name = if path.len() > mp.path.len() {
                &path[mp.path.len()..]
            } else {
                path
            };
            let name = name.trim_start_matches('/');
            match super::exfat::delete_file(name, disk) {
                Ok(()) => Ok(()),
                Err(e) => Err(e.as_str()),
            }
        }
        _ => Err("delete not supported"),
    }
}

/// Initialize the VFS.
pub fn init() {
    console::serial_write("[vfs] Initializing VFS...\n");

    let mut disk = RamDiskDevice::new();
    disk.init_with_exfat_image();

    mount::mount(mount::FsType::RamFs, "/", 0, 0, true);
    mount::mount(mount::FsType::ProcFs, "/proc", 0, 0, true);
    mount::mount(mount::FsType::DevFs, "/dev", 0, 0, false);
    mount::mount(mount::FsType::TmpFs, "/tmp", 0, 0, false);
    mount::mount(mount::FsType::Exfat, "/data", 0, 0, false);

    unsafe { GLOBAL_DISK = Some(disk); }

    match super::exfat::mount(get_disk()) {
        Ok(()) => console::serial_write("[vfs] exFAT mounted at /data\n"),
        Err(e) => {
            console::serial_write("[vfs] exFAT mount failed: ");
            console::serial_write(e.as_str());
            console::serial_write("\n");
        }
    }

    console::serial_write("[vfs] VFS initialized\n");
}

