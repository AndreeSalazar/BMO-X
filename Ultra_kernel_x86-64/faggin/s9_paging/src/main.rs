//! Faggin stage 9 ??? Paging handoff.
//!
//! Responsibilities (one only):
//!   - Preserve the UEFI page tables that map the active stack, context,
//!     fixed-address binaries, and GOP framebuffer.
//!   - Publish the current `pml4` in BootContext.
//!   - Jump to s10_heap.

#![no_std]
#![no_main]
#![allow(static_mut_refs)]

use core::panic::PanicInfo;
use core::arch::asm;

const NEXT_ADDR: u64 = 0x190000;

const PAGE_SIZE: u64 = 4096;
const HIGH_MEM_BASE: u64 = 0xFFFF_8000_0000_0000;
const PTE_PRESENT: u64 = 1 << 0;
const PTE_WRITABLE: u64 = 1 << 1;
const PTE_CACHE_DISABLE: u64 = 1 << 4;
const PTE_GLOBAL: u64 = 1 << 8;

const fn pte_addr(e: u64) -> u64 { e & 0x000F_FFFF_FFFF_F000 }

// Tiny local bitmap frame allocator (just for s9's own allocations
// of page-table frames). The real bitmap lives in s10_heap.
const POOL_SIZE: usize = 64; // 64 frames = 256 KB, plenty for PML4/PDPT/PD/PT
static mut POOL: [u64; POOL_SIZE / 64] = [0u64; POOL_SIZE / 64];
static mut POOL_BASE: u64 = 0;

unsafe fn pool_init(ctx: &boot_context::BootContext) {
    for e in &ctx.memory_map[..ctx.memory_map_count as usize] {
        if e.kind != 1 || e.size == 0 { continue; }
        let base = (e.base + PAGE_SIZE - 1) & !(PAGE_SIZE - 1);
        if base < 0x400000 { continue; } // skip first 4MB (stages)
        POOL_BASE = base;
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

unsafe fn zeroed_frame() -> *mut u64 {
    let p = pool_alloc();
    if p.is_null() { return core::ptr::null_mut(); }
    core::ptr::write_bytes(p as *mut u8, 0, PAGE_SIZE as usize);
    p
}

unsafe fn get_or_create(table: *mut u64, idx: usize) -> *mut u64 {
    let entry = table.add(idx).read_volatile();
    if entry & PTE_PRESENT == 0 {
        let p = zeroed_frame();
        if p.is_null() { return core::ptr::null_mut(); }
        table.add(idx).write_volatile(p as u64 | PTE_PRESENT | PTE_WRITABLE);
        return p;
    }
    (entry & pte_addr(!0u64)) as *mut u64
}

unsafe fn map_page(pml4: *mut u64, v: u64, p: u64, flags: u64) {
    let i4 = ((v >> 39) & 0x1FF) as usize;
    let i3 = ((v >> 30) & 0x1FF) as usize;
    let i2 = ((v >> 21) & 0x1FF) as usize;
    let i1 = ((v >> 12) & 0x1FF) as usize;
    let pdpt = get_or_create(pml4, i4);
    let pd   = get_or_create(pdpt, i3);
    let pt   = get_or_create(pd,   i2);
    pt.add(i1).write_volatile((p & pte_addr(!0u64)) | flags);
}

#[no_mangle]
#[link_section = ".text._start"]
pub extern "C" fn _start(ctx_ptr: *mut boot_context::BootContext) -> ! {
    let pml4_phys: u64;
    unsafe { asm!("mov {}, cr3", out(reg) pml4_phys); }

    let ctx = unsafe { &mut *ctx_ptr };
    ctx.pml4 = pml4_phys;

    serial_shared::puts("[s9 paging] preserving UEFI mappings until kernel VMM init\n");

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
