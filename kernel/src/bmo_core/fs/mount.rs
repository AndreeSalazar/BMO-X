//! VFS mount table for FastOS.
//!
//! Maps path prefixes to filesystem drivers.
//! Supports: RamFs (root), exFAT (data), procfs, devfs, tmpfs.

#![allow(dead_code)]

use crate::dev::console;

const MAX_MOUNTS: usize = 16;

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum FsType {
    RamFs,       // Ramdisk (archivos embebidos del kernel)
    Exfat,       // exFAT (data, read-write)
    ProcFs,      // /proc (virtual)
    DevFs,       // /dev (virtual)
    TmpFs,       // /tmp (RAM-backed)
    None,
}

#[derive(Debug, Clone, Copy)]
pub struct MountPoint {
    pub mount_id: u16,
    pub fs_type: FsType,
    pub path: &'static str,
    pub device_lba: u64,
    pub device_sectors: u32,
    pub read_only: bool,
    pub in_use: bool,
}

impl MountPoint {
    pub const fn empty() -> Self {
        Self {
            mount_id: 0,
            fs_type: FsType::None,
            path: "",
            device_lba: 0,
            device_sectors: 0,
            read_only: false,
            in_use: false,
        }
    }
}

static mut MOUNT_TABLE: [MountPoint; MAX_MOUNTS] = [MountPoint::empty(); MAX_MOUNTS];
static mut NEXT_MOUNT_ID: u16 = 1;

pub fn mount(fs_type: FsType, path: &'static str, lba: u64, sectors: u32, read_only: bool) -> Option<u16> {
    unsafe {
        for i in 0..MAX_MOUNTS {
            if !MOUNT_TABLE[i].in_use {
                let mount_id = NEXT_MOUNT_ID;
                NEXT_MOUNT_ID += 1;

                MOUNT_TABLE[i] = MountPoint {
                    mount_id,
                    fs_type,
                    path,
                    device_lba: lba,
                    device_sectors: sectors,
                    read_only,
                    in_use: true,
                };

                console::serial_write("[vfs] Mounted ");
                console::serial_write(path);
                console::serial_write(" (");
                match fs_type {
                    FsType::RamFs  => console::serial_write("RamFs"),
                    FsType::Exfat  => console::serial_write("exFAT"),
                    FsType::ProcFs => console::serial_write("procfs"),
                    FsType::DevFs  => console::serial_write("devfs"),
                    FsType::TmpFs  => console::serial_write("tmpfs"),
                    FsType::None   => console::serial_write("none"),
                }
                console::serial_write(") id=");
                serial_write_u16(mount_id);
                console::serial_write("\n");

                return Some(mount_id);
            }
        }
    }
    None
}

pub fn unmount(mount_id: u16) -> bool {
    unsafe {
        for i in 0..MAX_MOUNTS {
            if MOUNT_TABLE[i].in_use && MOUNT_TABLE[i].mount_id == mount_id {
                MOUNT_TABLE[i].in_use = false;
                console::serial_write("[vfs] Unmounted id=");
                serial_write_u16(mount_id);
                console::serial_write("\n");
                return true;
            }
        }
    }
    false
}

pub fn find_mount(path: &str) -> Option<&'static MountPoint> {
    unsafe {
        let mut best_match: Option<usize> = None;
        let mut best_len = 0;

        for i in 0..MAX_MOUNTS {
            if !MOUNT_TABLE[i].in_use { continue; }
            let mp_path = MOUNT_TABLE[i].path;
            if path.starts_with(mp_path) && mp_path.len() >= best_len {
                best_match = Some(i);
                best_len = mp_path.len();
            }
        }
        best_match.map(|i| &MOUNT_TABLE[i])
    }
}

pub fn get_mount(mount_id: u16) -> Option<&'static MountPoint> {
    unsafe {
        for i in 0..MAX_MOUNTS {
            if MOUNT_TABLE[i].in_use && MOUNT_TABLE[i].mount_id == mount_id {
                return Some(&MOUNT_TABLE[i]);
            }
        }
    }
    None
}

pub fn mount_count() -> u32 {
    unsafe { MOUNT_TABLE.iter().filter(|m| m.in_use).count() as u32 }
}

pub fn print_mounts() {
    console::serial_write("[vfs] Active mounts:\n");
    unsafe {
        for i in 0..MAX_MOUNTS {
            if MOUNT_TABLE[i].in_use {
                console::serial_write("  ");
                console::serial_write(MOUNT_TABLE[i].path);
                console::serial_write(" -> ");
                match MOUNT_TABLE[i].fs_type {
                    FsType::RamFs  => console::serial_write("RamFs"),
                    FsType::Exfat  => console::serial_write("exFAT"),
                    FsType::ProcFs => console::serial_write("procfs"),
                    FsType::DevFs  => console::serial_write("devfs"),
                    FsType::TmpFs  => console::serial_write("tmpfs"),
                    FsType::None   => console::serial_write("?"),
                }
                console::serial_write("\n");
            }
        }
    }
}

fn serial_write_u16(val: u16) {
    let mut buf = [0u8; 5];
    let mut i = buf.len();
    let mut v = val;
    if v == 0 { i -= 1; buf[i] = b'0'; }
    else { while v > 0 { i -= 1; buf[i] = b'0' + (v % 10) as u8; v /= 10; } }
    console::serial_write(core::str::from_utf8(&buf[i..]).unwrap_or("0"));
}
