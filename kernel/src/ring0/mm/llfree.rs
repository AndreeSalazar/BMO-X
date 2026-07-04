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
use super::BackingAllocator;

use llfree::{LLFree, Alloc, FrameId, Init, Classing, MetaData, Request, Class};

const BASE: u64 = 0x0100_0000;
// Phase 1 initializes the physical allocator before `map_high_mem()`, so
// LLFree metadata must live in the firmware's low identity-mapped window.
// The bootloader already keeps critical handoff data below 0x8000_0000 for
// this exact reason. Track low RAM first; high RAM can be enabled later once
// Ring 0 owns all page tables.
const LOW_IDENTITY_LIMIT: u64 = 0x8000_0000;
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

fn align_up(addr: u64, align: u64) -> u64 {
    (addr + align - 1) & !(align - 1)
}

fn ranges_overlap(a_start: u64, a_end: u64, b_start: u64, b_end: u64) -> bool {
    a_start < b_end && a_end > b_start
}

impl BackingAllocator for LlfreeAllocator {
    unsafe fn init(&self, memory_map: &[MemoryEntry], count: usize,
                   reserved_addr: u64, reserved_size: u64,
                   kernel_base: u64, kernel_size: u64) {
        if ALLOC.is_some() { return; }

        let entries = &memory_map[..count];

        // Detect max physical address and total RAM. LLFree metadata is built
        // before high memory is mapped, so the active tracking window is capped
        // to low identity-mapped RAM for now.
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

        let tracked_limit = max_usable.min(LOW_IDENTITY_LIMIT);
        if tracked_limit <= BASE {
            crate::dev::console::serial_write("[llfree] FATAL: no low identity-mapped usable RAM\n");
            loop { core::arch::asm!("cli; hlt"); }
        }

        let total_frames = (tracked_limit / PAGE_SIZE) as usize;
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
            let region_start = align_up(e.base.max(BASE), PAGE_SIZE);
            let region_end = (e.base + e.size).min(tracked_limit);
            if region_start >= region_end { continue; }

            let meta_len = (meta_pages as u64) * PAGE_SIZE;
            let mut candidate = region_start;
            loop {
                let meta_start = candidate;
                let meta_end = meta_start + meta_len;
                if meta_end > region_end { break; }

                if kernel_size > 0 && ranges_overlap(meta_start, meta_end, kernel_base, kernel_base + kernel_size) {
                    candidate = align_up(kernel_base + kernel_size, PAGE_SIZE);
                    continue;
                }
                if reserved_size > 0 && ranges_overlap(meta_start, meta_end, reserved_addr, reserved_addr + reserved_size) {
                    candidate = align_up(reserved_addr + reserved_size, PAGE_SIZE);
                    continue;
                }
                if ranges_overlap(meta_start, meta_end, 0x9_0000, 0x9_1000) {
                    candidate = 0x9_1000;
                    continue;
                }

                meta_phys = meta_start;
                break;
            }
            if meta_phys != 0 { break; }
        }
        if meta_phys == 0 {
            crate::dev::console::serial_write("[llfree] FATAL: cannot allocate low metadata\n");
            loop { core::arch::asm!("cli; hlt"); }
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
                fn reserve_range(alloc: &LLFree<'static>, start: u64, end: u64, tracked_limit: u64) {
                    let mut addr = start;
                    let end = end.min(tracked_limit);
                    while addr < end {
                        if let Some(frame) = addr_to_frame(addr) {
                            alloc.get(Some(frame), mk_request(0, 0)).ok();
                        }
                        addr += PAGE_SIZE;
                    }
                }
                reserve_range(&alloc, 0, BASE, tracked_limit); // below 16 MB
                reserve_range(&alloc, kernel_base, kernel_base + kernel_size, tracked_limit);
                reserve_range(&alloc, reserved_addr, reserved_addr + reserved_size, tracked_limit);
                reserve_range(&alloc, meta_phys, meta_phys + (meta_pages as u64) * PAGE_SIZE, tracked_limit);
                reserve_range(&alloc, 0x9_0000, 0x9_1000, tracked_limit); // crash marker

                // Also reserve non-usable regions from UEFI map.
                for e in entries {
                    if e.mem_type != MemoryType::Usable {
                        let start = e.base.max(BASE);
                        let end = (e.base + e.size).min(tracked_limit);
                        if start < end {
                            reserve_range(&alloc, start, end, tracked_limit);
                        }
                    }
                }

                // Reserve unmapped tails (non-2MB-aligned start and end of usable regions)
                const HUGE_2MB: u64 = 2 * 1024 * 1024;
                for e in entries {
                    if e.mem_type == MemoryType::Usable {
                        let region_start = e.base.max(BASE);
                        let region_end = (e.base + e.size).min(tracked_limit);
                        if region_start >= region_end { continue; }
                        
                        let start_aligned = (region_start + HUGE_2MB - 1) & !(HUGE_2MB - 1);
                        let end_aligned = region_end & !(HUGE_2MB - 1);
                        
                        if region_start < start_aligned {
                            reserve_range(&alloc, region_start, start_aligned, tracked_limit);
                        }
                        if end_aligned < region_end {
                            reserve_range(&alloc, end_aligned, region_end, tracked_limit);
                        }
                    }
                }

                let free = alloc.tree_stats().free_frames;
                FREE_COUNT.store(free, Ordering::Relaxed);

                crate::dev::console::serial_write("[llfree] init: ");
                crate::dev::console::serial_write_u64(free as u64, 10);
                crate::dev::console::serial_write(" free frames, metadata=");
                crate::dev::console::serial_write_u64(meta_pages as u64, 10);
                crate::dev::console::serial_write(" pages, tracked_low_mb=");
                crate::dev::console::serial_write_u64(tracked_limit / (1024 * 1024), 10);
                crate::dev::console::serial_write("\n");

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
