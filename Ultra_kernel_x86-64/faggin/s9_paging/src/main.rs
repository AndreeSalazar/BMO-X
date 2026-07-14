//! Faggin stage 9 — Build fresh PML4 page tables.
//!
//! Responsibilities (one only):
//!   - Allocate 1 frame for the PML4 root.
//!   - Identity-map 0..4 MB (stages live here, we run here).
//!   - Higher-half map 0..2 GB to 0xFFFF_8000_0000_0000.
//!   - Identity-map the GOP framebuffer region (if present).
//!   - Load CR3 with the new PML4.
//!   - Publish `pml4` in BootContext.
//!   - Jump to s10_heap.
//!
//! Why we build fresh instead of inheriting UEFI's tables:
//!   UEFI identity-maps the GOP on most firmware, but after
//!   ExitBootServices some implementations unmap runtime
//!   regions. Building our own PML4 guarantees that the GOP
//!   remains accessible for the rest of the kernel's lifetime.

#![no_std]
#![no_main]
#![allow(static_mut_refs)]

use core::panic::PanicInfo;
use core::arch::asm;

const NEXT_ADDR: u64 = 0x190000;

const PAGE_SIZE: u64 = 4096;
const HIGH_MEM_BASE: u64 = 0xFFFF_8000_0000_0000;

const PTE_PRESENT:    u64 = 1 << 0;
const PTE_WRITABLE:   u64 = 1 << 1;
const PTE_HUGE:       u64 = 1 << 7;
const PTE_GLOBAL:     u64 = 1 << 8;
const PTE_CACHE_DISABLE: u64 = 1 << 4;

const fn pte_addr(e: u64) -> u64 { e & 0x000F_FFFF_FFFF_F000 }

// ── Tiny frame pool (64 frames = 256 KB) ─────────────────────────

const POOL_SIZE: usize = 64;
static mut POOL: [u64; POOL_SIZE / 64] = [0u64; POOL_SIZE / 64];
static mut POOL_BASE: u64 = 0;
static mut POOL_END: u64 = 0;

unsafe fn pool_init(ctx: &boot_context::BootContext) {
    // Find the first usable memory entry above 4 MB.
    for e in &ctx.memory_map[..ctx.memory_map_count as usize] {
        if e.kind != 1 || e.size == 0 { continue; }
        let base = (e.base + PAGE_SIZE - 1) & !(PAGE_SIZE - 1);
        if base < 0x400000 { continue; }
        POOL_BASE = base;
        POOL_END = base + (POOL_SIZE as u64) * PAGE_SIZE;
        // Carve out the pool pages from the bitmap in s10_heap
        // (s10 reads ctx, but the pool frames are ours).
        break;
    }
}

unsafe fn pool_alloc() -> *mut u64 {
    for i in 0..POOL_SIZE {
        if POOL[i / 64] & (1 << (i % 64)) == 0 {
            POOL[i / 64] |= 1 << (i % 64);
            return (POOL_BASE + (i as u64) * PAGE_SIZE) as *mut u64;
        }
    }
    core::ptr::null_mut()
}

unsafe fn zeroed_frame() -> &'static mut [u64; 512] {
    let p = pool_alloc() as *mut [u64; 512];
    if p.is_null() {
        serial_shared::puts("[s9] FATAL: out of frames\n");
        loop { asm!("hlt"); }
    }
    core::ptr::write_bytes(p as *mut u8, 0, PAGE_SIZE as usize);
    &mut *p
}

unsafe fn get_or_create(table: *mut u64, idx: usize, verbose: bool) -> *mut u64 {
    let entry = table.add(idx).read_volatile();
    if entry & PTE_PRESENT == 0 {
        let p = zeroed_frame();
        if verbose {
            serial_shared::puts("[s9]   new table at 0x");
            serial_shared::hex(p as *const [u64; 512] as u64);
            serial_shared::puts(" idx=");
            serial_shared::dec(idx);
            serial_shared::puts("\n");
        }
        table.add(idx).write_volatile(p.as_ptr() as u64 | PTE_PRESENT | PTE_WRITABLE);
        return p.as_mut_ptr();
    }
    (entry & pte_addr(!0u64)) as *mut u64
}

/// Map a single 4 KB page at virtual address `v` to physical address `p`
/// with the given flags, creating intermediate table levels as needed.
unsafe fn map_page(pml4: *mut u64, v: u64, p: u64, flags: u64, verbose: bool) {
    let i4 = ((v >> 39) & 0x1FF) as usize;
    let i3 = ((v >> 30) & 0x1FF) as usize;
    let i2 = ((v >> 21) & 0x1FF) as usize;
    let i1 = ((v >> 12) & 0x1FF) as usize;

    let pdpt = get_or_create(pml4, i4, verbose);
    let pd   = get_or_create(pdpt, i3, verbose);
    let pt   = get_or_create(pd,   i2, verbose);

    let entry = (p & pte_addr(!0u64)) | flags;
    pt.add(i1).write_volatile(entry);

    if verbose {
        serial_shared::puts("[s9]   mapped 0x");
        serial_shared::hex(v);
        serial_shared::puts(" -> 0x");
        serial_shared::hex(p);
        serial_shared::puts("\n");
    }
}

/// Map a range with 2 MB huge pages for identity mapping.
unsafe fn map_2m_huge(pml4: *mut u64, v_start: u64, p_start: u64, count_2m: usize, flags: u64) {
    for i in 0..count_2m {
        let v = v_start + (i as u64) * 0x20_0000u64;
        let p = p_start + (i as u64) * 0x20_0000u64;
        let i4 = ((v >> 39) & 0x1FF) as usize;
        let i3 = ((v >> 30) & 0x1FF) as usize;
        let i2 = ((v >> 21) & 0x1FF) as usize;

        let pdpt = get_or_create(pml4, i4, false);
        let pd   = get_or_create(pdpt, i3, false);
        let entry = (p & pte_addr(!0u64)) | flags | PTE_HUGE;
        pd.add(i2).write_volatile(entry);
    }
}

#[no_mangle]
#[link_section = ".text._start"]
pub extern "C" fn _start(ctx_ptr: *mut boot_context::BootContext) -> ! {
    let ctx = unsafe { &*ctx_ptr };
    serial_shared::puts("[s9 paging] building new PML4\n");

    // 1. Init frame pool from memory map.
    unsafe { pool_init(ctx); }

    // 2. Allocate the PML4 root frame.
    let pml4 = unsafe { zeroed_frame() };
    let pml4_phys = pml4.as_ptr() as u64;
    serial_shared::puts("[s9 paging] PML4 at physical=0x");
    serial_shared::hex(pml4_phys);
    serial_shared::puts("\n");

    // 3. Identity-map 0..20 MB with 2 MB huge pages (fast path).
    //    This covers:
    //    - 0..4 MB: the 12 faggin stages (each at 0x100000 + n*0x10000)
    //    - 4..20 MB: the kernel at 0x400000 (16 MiB reserved by L3)
    //    Without the 4..20 MB range, s12_devices's jmp 0x400000
    //    hits a not-present page and the kernel never runs.
    //    The physical address matches the virtual address.
    unsafe {
        map_2m_huge(pml4.as_mut_ptr(), 0x0, 0x0, 10,
            PTE_PRESENT | PTE_WRITABLE);
    }
    serial_shared::puts("[s9 paging] identity-mapped 0..20MB\n");

    // 4. Higher-half mirror: map 0..16GB physical to 0xFFFF_8000_0000_0000.
    //    This covers:
    //    - 0..2GB: kernel, heap, channel pages, any allocations
    //    - 2..16GB: the UEFI stack and runtime memory (which lives
    //      in high physical memory and is accessed via the
    //      higher-half mirror by the boot services before Exit)
    //    The higher-half mirror is what the kernel uses to access
    //    physical memory (HIGHER_HALF_BASE + phys_addr).
    unsafe {
        // 16 GB = 8192 × 2 MB huge pages
        map_2m_huge(pml4.as_mut_ptr(), HIGH_MEM_BASE, 0x0, 8192,
            PTE_PRESENT | PTE_WRITABLE | PTE_GLOBAL);
    }
    serial_shared::puts("[s9 paging] higher-half 0..16GB -> ");
    serial_shared::hex(HIGH_MEM_BASE);
    serial_shared::puts("\n");

    // 5. Identity-map the GOP framebuffer (if any).
    //    We map it with WC (write-combining) semantics via the
    //    cache-disable bit so the kernel can write pixels
    //    efficiently. Without this explicit mapping, the
    //    framebuffer may not be reachable after ExitBootServices.
    if ctx.fb_addr != 0 {
        let fb_start = ctx.fb_addr & !(PAGE_SIZE - 1);
        let fb_size_pages = ((ctx.fb_stride as u64) * (ctx.fb_height as u64) * 4);
        let fb_pages = ((fb_size_pages + PAGE_SIZE - 1) / PAGE_SIZE) as usize;
        serial_shared::puts("[s9 paging] GOP fb 0x");
        serial_shared::hex(fb_start);
        serial_shared::puts(" size=");
        serial_shared::dec(fb_size_pages as usize);
        serial_shared::puts(" bytes (");
        serial_shared::dec(fb_pages);
        serial_shared::puts(" pages)\n");

        for i in 0..fb_pages {
            let p = fb_start + (i as u64) * PAGE_SIZE;
            unsafe {
                map_page(pml4.as_mut_ptr(), p, p,
                    PTE_PRESENT | PTE_WRITABLE | PTE_CACHE_DISABLE,
                    false);
            }
        }
        serial_shared::puts("[s9 paging] GOP identity-mapped with CD (WC-like)\n");
    } else {
        serial_shared::puts("[s9 paging] no GOP framebuffer (headless)\n");
    }

    // 6. Load CR3 with our new PML4.
    let ctx = unsafe { &mut *ctx_ptr };
    serial_shared::puts("[s9 paging] loading CR3 -> 0x");
    serial_shared::hex(pml4_phys);
    serial_shared::puts("\n");
    unsafe { asm!("mov cr3, {}", in(reg) pml4_phys); }
    ctx.pml4 = pml4_phys;

    // 7. Jump to the next stage.
    serial_shared::puts("[s9 paging] done, jumping to s10_heap\n");
    unsafe {
        asm!(
            "jmp {next}",
            next = in(reg) NEXT_ADDR,
            in("rdi") ctx_ptr,
            options(noreturn)
        );
    }
}

#[panic_handler]
fn panic(_info: &PanicInfo) -> ! { loop { unsafe { asm!("hlt"); } } }
