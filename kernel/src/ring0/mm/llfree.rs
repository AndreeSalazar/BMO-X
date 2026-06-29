//! LLFree adapter — Lock-free backing allocator via llfree crate.
//!
//! Wraps `llfree::LLFree` into the `BackingAllocator` trait.
//! Active with feature `alloc-llfree`.
//!
//! Architecture:
//!   Lower level: bitfield-based per-page metadata (4 KiB .. 4 MiB)
//!   Upper level: tree-based locality classes with per-CPU reservations
//!
//! Reference: Wrenger et al., USENIX ATC '23

use core::sync::atomic::{AtomicUsize, Ordering};
use fastos_boot_protocol::{MemoryEntry, MemoryType};
use super::PAGE_SIZE;
use super::MAX_ORDER;
use super::BackingAllocator;

use llfree::{LLFree, Alloc, FrameId, Init, Classing, MetaData, Request, Class};

const BASE: u64 = 0x0100_0000;
const HUGE_ORDER: usize = 9; // 2^9 = 512 pages = 2 MiB

static mut ALLOC: Option<LLFree<'static>> = None;
static mut TOTAL_RAM: u64 = 0;
static mut TRACKED_FRAMES: usize = 0;
static FREE_COUNT: AtomicUsize = AtomicUsize::new(0);

/// LLFree backing allocator.
pub struct LlfreeAllocator;

unsafe impl Sync for LlfreeAllocator {}

fn mk_request(order: usize, cpu: usize) -> Request {
    let class = if order >= HUGE_ORDER { Class(1) } else { Class(0) };
    Request::new(order, class, Some(cpu))
}

fn addr_to_frame(addr: u64) -> Option<FrameId> {
    Some(FrameId((addr / PAGE_SIZE) as usize))
}

fn frame_to_addr(frame: FrameId) -> u64 {
    (frame.0 as u64) * PAGE_SIZE
}

/// Free all pages in a usable region into the LLFree allocator,
/// trying coarser orders for efficiency.
unsafe fn free_region(alloc: &LLFree<'static>, base: u64, size: u64) {
    let mut addr = base;
    let end = base + size;
    while addr < end {
        let remaining = ((end - addr) / PAGE_SIZE) as usize;
        if remaining == 0 { break; }

        let mut order = 0usize;
        let mut block = 1usize;
        while block <= remaining && order < MAX_ORDER {
            if (addr as usize & ((block << 12) - 1)) != 0 { break; }
            order += 1;
            block <<= 1;
        }
        order -= 1;
        block >>= 1;

        let frame = addr_to_frame(addr).unwrap();
        alloc.put(frame, mk_request(order, 0)).ok();
        addr += (block as u64) * PAGE_SIZE;
    }
}

impl BackingAllocator for LlfreeAllocator {
    unsafe fn init(&self, memory_map: &[MemoryEntry], count: usize,
                   reserved_addr: u64, reserved_size: u64,
                   kernel_base: u64, kernel_size: u64) {
        if ALLOC.is_some() { return; }

        let entries = &memory_map[..count];

        // Detect max physical address.
        let mut max_usable: u64 = 0;
        let mut total_ram: u64 = 0;
        for e in entries {
            if e.mem_type == MemoryType::Usable {
                total_ram += e.size;
                let end = e.base + e.size;
                if end > max_usable { max_usable = end; }
            }
        }
        TOTAL_RAM = total_ram;

        let total_frames = (max_usable / PAGE_SIZE) as usize;
        TRACKED_FRAMES = total_frames;

        // Build classing with 1 core (BSP; SMP later adds more).
        let classing = Classing::simple(1).0;

        // Calculate metadata size.
        let ms = LLFree::metadata_size(&classing, total_frames);

        // Allocate metadata buffers from first large enough usable region.
        let meta_size_total = ms.local + ms.trees + ms.lower;
        let meta_pages = (meta_size_total + PAGE_SIZE as usize - 1) / PAGE_SIZE as usize;

        let mut meta_phys: u64 = 0;
        for e in entries {
            if e.mem_type != MemoryType::Usable { continue; }
            let avail = ((e.base + e.size - e.base.max(BASE)) / PAGE_SIZE) as usize;
            if avail >= meta_pages {
                meta_phys = (e.base.max(BASE) + PAGE_SIZE - 1) / PAGE_SIZE * PAGE_SIZE;
                break;
            }
        }
        if meta_phys == 0 {
            crate::dev::console::serial_write("[llfree] FATAL: cannot allocate metadata\n");
            return;
        }

        let local_ptr  = meta_phys as *mut u8;
        let trees_ptr  = (meta_phys + ms.local as u64) as *mut u8;
        let lower_ptr  = (meta_phys + ms.local as u64 + ms.trees as u64) as *mut u8;

        let local  = core::slice::from_raw_parts_mut(local_ptr,  ms.local);
        let trees  = core::slice::from_raw_parts_mut(trees_ptr,  ms.trees);
        let lower  = core::slice::from_raw_parts_mut(lower_ptr,  ms.lower);

        let meta = MetaData { local, trees, lower };

        // Initialize allocator with all frames marked free.
        match LLFree::new(total_frames, Init::FreeAll, &classing, meta) {
            Ok(alloc) => {
                // Reserve kernel image, reserved area, metadata pages.
                fn reserve_range(alloc: &LLFree<'static>, start: u64, end: u64) {
                    let mut addr = start;
                    while addr < end {
                        if let Some(frame) = addr_to_frame(addr) {
                            alloc.get(Some(frame), mk_request(0, 0)).ok();
                        }
                        addr += PAGE_SIZE;
                    }
                }
                reserve_range(&alloc, 0, BASE); // below 16 MB
                reserve_range(&alloc, kernel_base, kernel_base + kernel_size);
                reserve_range(&alloc, reserved_addr, reserved_addr + reserved_size);
                reserve_range(&alloc, meta_phys, meta_phys + (meta_pages as u64) * PAGE_SIZE);
                reserve_range(&alloc, 0x9_0000, 0x9_1000); // crash marker

                // Also reserve non-usable regions from UEFI map.
                for e in entries {
                    if e.mem_type != MemoryType::Usable {
                        let start = e.base.max(BASE);
                        let end = (e.base + e.size).min(max_usable);
                        if start < end {
                            reserve_range(&alloc, start, end);
                        }
                    }
                }

                let free = alloc.tree_stats().free_frames;
                FREE_COUNT.store(free, Ordering::Relaxed);

                crate::dev::console::serial_write("[llfree] init: ");
                crate::dev::console::serial_write_u64(free as u64, 10);
                crate::dev::console::serial_write(" free frames, metadata=");
                crate::dev::console::serial_write_u64(meta_pages as u64, 10);
                crate::dev::console::serial_write(" pages\n");

                ALLOC = Some(alloc);
            }
            Err(e) => {
                crate::dev::console::serial_write("[llfree] init FAILED: error code ");
                crate::dev::console::serial_write_u64(e as u64, 10);
                crate::dev::console::serial_write("\n");
            }
        }
    }

    unsafe fn alloc_order(&self, order: usize) -> Option<u64> {
        let alloc = ALLOC.as_ref()?;
        let (frame, _) = alloc.get(None, mk_request(order, 0)).ok()?;
        let addr = frame_to_addr(frame);
        FREE_COUNT.fetch_sub(1usize << order, Ordering::Relaxed);
        Some(addr)
    }

    unsafe fn free_order(&self, addr: u64, order: usize) {
        let alloc = match ALLOC.as_ref() { Some(a) => a, None => return };
        if let Some(frame) = addr_to_frame(addr) {
            alloc.put(frame, mk_request(order, 0)).ok();
            FREE_COUNT.fetch_add(1usize << order, Ordering::Relaxed);
        }
    }

    fn free_count(&self) -> usize {
        FREE_COUNT.load(Ordering::Relaxed)
    }

    fn total_ram(&self) -> u64 {
        unsafe { TOTAL_RAM }
    }

    fn tracked_pages(&self) -> usize {
        unsafe { TRACKED_FRAMES }
    }
}
