//! Storage HAL (Ring 0).
//!
//! Provides the disk-read primitives that the HalServices table
//! advertises to Ring 3 modules. v1.x ships with safe stubs that
//! return clear errors so userland can detect "no driver yet" and
//! fall back to a "storage not available" UI.
//!
//! When the AHCI driver lands in `crates_Personal/ring0/`, the
//! `*_sectors` functions get real implementations; the public API
//! is stable.

use core::sync::atomic::{AtomicBool, AtomicU64, Ordering};

/// True if a working storage driver is loaded.
static STORAGE_READY: AtomicBool = AtomicBool::new(false);
/// Number of ports detected by the driver.
static PORT_COUNT: AtomicU64 = AtomicU64::new(0);

/// Called by the AHCI driver once it has detected at least one port.
pub fn set_ready(ports: u8) {
    STORAGE_READY.store(true, Ordering::Release);
    PORT_COUNT.store(ports as u64, Ordering::Relaxed);
}

/// Returns true if storage is available.
pub fn is_ready() -> bool {
    STORAGE_READY.load(Ordering::Acquire)
}

/// Returns the number of detected storage ports.
pub fn port_count() -> u8 {
    PORT_COUNT.load(Ordering::Relaxed) as u8
}

/// Read `sector_count` sectors (512 B each) from `port`, `lba` into `dst`.
/// Returns true on success, false on any error.
///
/// SAFETY: `dst` must be at least `sector_count * 512` bytes.
pub unsafe fn read_sectors(port: u8, lba: u64, sector_count: u16, dst: *mut u8) -> bool {
    if !is_ready() { return false; }
    if port >= port_count() { return false; }
    if dst.is_null() || sector_count == 0 { return false; }
    // Real implementation will live in the AHCI driver. For now, log
    // the request and return false so Ring 3 can handle it.
    crate::dev::console::serial_write("[storage] read_sectors stub: port=");
    crate::dev::console::serial_write_u64(port as u64, 10);
    crate::dev::console::serial_write(" lba=0x");
    crate::dev::console::serial_write_u64(lba, 16);
    crate::dev::console::serial_write(" count=");
    crate::dev::console::serial_write_u64(sector_count as u64, 10);
    crate::dev::console::serial_write("\n");
    false
}

/// Write `sector_count` sectors from `src` to `port`, `lba`. Returns true
/// on success.
///
/// SAFETY: `src` must be at least `sector_count * 512` bytes.
pub unsafe fn write_sectors(port: u8, _lba: u64, sector_count: u16, src: *const u8) -> bool {
    if !is_ready() { return false; }
    if port >= port_count() { return false; }
    if src.is_null() || sector_count == 0 { return false; }
    crate::dev::console::serial_write("[storage] write_sectors stub: not yet implemented\n");
    false
}

/// Returns true if the given port has a working device.
pub fn port_active(port: u8) -> bool {
    if !is_ready() { return false; }
    port < port_count()
}

/// Test the storage subsystem end-to-end. Used during boot to decide
/// whether to surface a "storage ready" indicator.
pub fn self_test() -> bool {
    is_ready()
}
