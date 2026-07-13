//! Physical frame allocator — minimal bitmap-based.

use super::types::{MemoryEntry, MemType};

const FRAME_SIZE: u64 = 4096;
const MAX_FRAMES: usize = 32768; // 128 MB worth of frames tracked

static mut FRAME_BITMAP: [u64; MAX_FRAMES / 64] = [0; MAX_FRAMES / 64];
static mut FRAME_BASE: u64 = 0;
static mut FRAME_COUNT: u64 = 0;
static mut NEXT_HINT: usize = 0;

/// Initialize the frame allocator from a list of UEFI memory entries.
/// The `boot_info_reserve` parameter is the physical base of the
/// `BootContext` struct itself — those pages are reserved so the
/// allocator never hands them out.
pub fn init(entries: &[MemoryEntry], boot_info_reserve: u64) {
    unsafe {
        FRAME_BASE = 0;
        FRAME_COUNT = 0;
        for e in entries {
            if !e.mem_type.is_usable() { continue; }
            let end = e.end();
            // Round to page boundary.
            let base = (e.base + FRAME_SIZE - 1) & !(FRAME_SIZE - 1);
            if end <= base { continue; }
            FRAME_BASE = base;
            FRAME_COUNT = (end - base) / FRAME_SIZE;
            break;
        }

        // Mark all frames free.
        for word in FRAME_BITMAP.iter_mut() { *word = 0; }

        // Reserve the BootInfo pages.
        if boot_info_reserve != 0 {
            let off = boot_info_reserve.saturating_sub(FRAME_BASE) / FRAME_SIZE;
            for i in 0..16 {
                let idx = (off as usize).saturating_add(i as usize);
                if idx < MAX_FRAMES {
                    FRAME_BITMAP[idx / 64] |= 1 << (idx % 64);
                }
            }
        }

        NEXT_HINT = 0;

        crate::ring0::dev::console::serial_write("[phys] ");
        crate::ring0::dev::console::serial_write_u64(FRAME_COUNT, 10);
        crate::ring0::dev::console::serial_write(" frames @ 0x");
        crate::ring0::dev::console::serial_write_u64(FRAME_BASE, 16);
        crate::ring0::dev::console::serial_write("\n");
    }
}

/// Allocate a single 4KB physical frame. Returns 0 if exhausted.
pub fn alloc_frame() -> u64 {
    unsafe {
        let total = FRAME_COUNT as usize;
        let mut i = NEXT_HINT;
        while i < total.min(MAX_FRAMES) {
            if FRAME_BITMAP[i / 64] & (1 << (i % 64)) == 0 {
                FRAME_BITMAP[i / 64] |= 1 << (i % 64);
                NEXT_HINT = i + 1;
                return FRAME_BASE + (i as u64) * FRAME_SIZE;
            }
            i += 1;
        }
        0
    }
}

/// Free a previously allocated frame. No-op if address is outside range.
pub fn free_frame(addr: u64) {
    unsafe {
        if addr < FRAME_BASE { return; }
        let off = (addr - FRAME_BASE) / FRAME_SIZE;
        if off as usize >= MAX_FRAMES { return; }
        let i = off as usize;
        FRAME_BITMAP[i / 64] &= !(1 << (i % 64));
    }
}

pub fn free_count() -> u64 {
    unsafe {
        let mut count = 0u64;
        for i in 0..(FRAME_COUNT as usize).min(MAX_FRAMES) {
            if FRAME_BITMAP[i / 64] & (1 << (i % 64)) == 0 { count += 1; }
        }
        count
    }
}
