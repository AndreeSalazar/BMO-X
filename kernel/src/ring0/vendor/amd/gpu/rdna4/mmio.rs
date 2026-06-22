//! `vendor/amd/gpu/rdna4/mmio.rs` — RDNA4 MMIO read/write helpers.
//!
//! v1.8.8: skeleton. Provides safe wrappers around volatile MMIO reads
//! and writes. Uses `read_volatile`/`write_volatile` to prevent the
//! compiler from eliding or reordering accesses.

#![allow(dead_code)]

use core::ptr;

/// Reads a 32-bit value from MMIO at the given physical address.
#[inline]
pub fn read32(addr: u64) -> u32 {
    unsafe { ptr::read_volatile(addr as *const u32) }
}

/// Reads a 64-bit value from MMIO at the given physical address.
#[inline]
pub fn read64(addr: u64) -> u64 {
    unsafe { ptr::read_volatile(addr as *const u64) }
}

/// Writes a 32-bit value to MMIO at the given physical address.
#[inline]
pub fn write32(addr: u64, value: u32) {
    unsafe { ptr::write_volatile(addr as *mut u32, value); }
}

/// Writes a 64-bit value to MMIO at the given physical address.
#[inline]
pub fn write64(addr: u64, value: u64) {
    unsafe { ptr::write_volatile(addr as *mut u64, value); }
}

/// Reads a 32-bit value from MMIO at the given address + offset.
#[inline]
pub fn read32_off(base: u64, offset: u32) -> u32 {
    read32(base + offset as u64)
}

/// Writes a 32-bit value to MMIO at the given address + offset.
#[inline]
pub fn write32_off(base: u64, offset: u32, value: u32) {
    write32(base + offset as u64, value)
}

/// Sets specific bits in an MMIO register (read-modify-write).
#[inline]
pub fn set_bits32(base: u64, offset: u32, mask: u32) {
    let v = read32_off(base, offset);
    write32_off(base, offset, v | mask);
}

/// Clears specific bits in an MMIO register (read-modify-write).
#[inline]
pub fn clear_bits32(base: u64, offset: u32, mask: u32) {
    let v = read32_off(base, offset);
    write32_off(base, offset, v & !mask);
}

/// Polls an MMIO register until the given mask of bits is set or the
/// timeout (in microseconds) is reached.
/// Returns `true` if the bits were set before the timeout.
#[inline]
pub fn poll32_set(base: u64, offset: u32, mask: u32, timeout_us: u32) -> bool {
    let start = unsafe { core::arch::x86_64::_rdtsc() };
    let tsc_freq = crate::profile::active::BASE_HZ;
    let deadline = start + (tsc_freq / 1_000_000) * timeout_us as u64;
    loop {
        if read32_off(base, offset) & mask == mask {
            return true;
        }
        if unsafe { core::arch::x86_64::_rdtsc() } > deadline {
            return false;
        }
    }
}
