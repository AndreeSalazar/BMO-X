//! Filesystem HAL (Ring 0).
//!
//! The actual FAT32 / exFAT / ramdisk implementation lives in
//! `crates_Personal/ring0/`. This module provides the public API
//! that `HalServices::fs_*` function pointers bind to. v1.x ships
//! with safe stubs that return `None`/`false` so Ring 3 can detect
//! "no filesystem mounted" and report the condition to the user
//! instead of crashing.

use core::sync::atomic::{AtomicBool, AtomicU64, Ordering};

/// One byte returned in a `fs_read_file` error to signal "not mounted".
pub const FS_ERR_NOT_MOUNTED: u8 = 0xFE;

/// One byte returned in a `fs_read_file` error to signal "file not found".
pub const FS_ERR_NOT_FOUND: u8 = 0xFD;

/// One byte returned in a `fs_read_file` error to signal "buffer too small".
pub const FS_ERR_TOO_SMALL: u8 = 0xFC;

static FS_MOUNTED: AtomicBool = AtomicBool::new(false);
static MOUNT_DEV:  AtomicU64  = AtomicU64::new(0);

/// Mark a device as having a mounted filesystem.
pub fn set_mounted(dev: u8) {
    FS_MOUNTED.store(true, Ordering::Release);
    MOUNT_DEV.store(dev as u64, Ordering::Relaxed);
}

/// Mark the filesystem as unmounted.
pub fn set_unmounted() {
    FS_MOUNTED.store(false, Ordering::Release);
    MOUNT_DEV.store(0, Ordering::Relaxed);
}

/// Mount a filesystem on the given storage device.
/// v1.x: stub — returns false until a real driver is loaded.
pub fn mount(dev: u8) -> bool {
    crate::dev::console::serial_write("[fs] mount stub: dev=");
    crate::dev::console::serial_write_u64(dev as u64, 10);
    crate::dev::console::serial_write(" (no driver yet)\n");
    false
}

/// Read a file. `path` is a UTF-8 path, `buf` is the output buffer.
/// Returns the number of bytes read, or `None` on error.
pub fn read_file(_path: &str, _buf: &mut [u8]) -> Option<usize> {
    if !FS_MOUNTED.load(Ordering::Acquire) { return None; }
    None
}

/// Write a file.
pub fn write_file(_path: &str, _buf: &[u8]) -> bool {
    if !FS_MOUNTED.load(Ordering::Acquire) { return false; }
    false
}

/// Find a subdirectory. Returns the inode id (opaque u64).
pub fn find_subdir(_path: &str) -> Option<u64> {
    if !FS_MOUNTED.load(Ordering::Acquire) { return None; }
    None
}

/// True if a filesystem is currently mounted.
pub fn is_mounted() -> bool {
    FS_MOUNTED.load(Ordering::Acquire)
}
