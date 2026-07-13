//! Filesystem — stub.
//!
//! VFS / ext2 / FAT are deferred. The boot chain's `stage3_dev` does
//! not implement a real filesystem; the ramdisk is the only "FS"
//! currently.

pub fn init() {}
pub fn mount(_path: &str) -> bool { false }
