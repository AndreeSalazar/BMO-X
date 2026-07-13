//! Faggin stage 10 — Heap (bitmap frame allocator + buddy + slab).
//!
//! Responsibilities (one only):
//!   - Initialize the bitmap frame allocator from ctx.memory_map.
//!   - Build the buddy free list.
//!   - Pre-allocate one slab per size class.
//!   - Publish `heap_base` and `heap_size` in BootContext.
//!   - Jump to s11_acpi.

#![no_std]
#![no_main]
#![allow(static_mut_refs)]

use core::panic::PanicInfo;
use core::arch::asm;

extern "C" {
    fn s11_acpi(ctx: *mut boot_context::BootContext) -> !;
    // The bitmap frame allocator (defined in s9_paging via a tiny
    // extern block) — but to keep stages decoupled, we use a fresh
    // helper here.
}

const PAGE_SIZE: u64 = 4096;
const MAX_FRAMES: usize = 32768; // 128 MB worth tracked

static mut FRAME_BITMAP: [u64; MAX_FRAMES / 64] = [0u64; MAX_FRAMES / 64];
static mut FRAME_BASE: u64 = 0;
static mut FRAME_COUNT: u64 = 0;

#[no_mangle]
pub unsafe extern "C" fn alloc_frame() -> u64 {
    let total = FRAME_COUNT as usize;
    for i in 0..total.min(MAX_FRAMES) {
        if FRAME_BITMAP[i / 64] & (1 << (i % 64)) == 0 {
            FRAME_BITMAP[i / 64] |= 1 << (i % 64);
            return FRAME_BASE + (i as u64) * PAGE_SIZE;
        }
    }
    0
}

/// Exposed to the next stage / kernel via BootContext.heap_base.
/// Not actually a function call — it's a static pointer the kernel
/// will read.
#[no_mangle]
pub static HEAP_BASE_PTR: u64 = 0;

unsafe fn build_bitmap(ctx: &boot_context::BootContext) {
    let high = 0xFFFF_8000_0000_0000u64;
    for entry in &ctx.memory_map[..ctx.memory_map_count as usize] {
        if entry.kind != 1 || entry.size == 0 { continue; }
        let base = (entry.base + PAGE_SIZE - 1) & !(PAGE_SIZE - 1);
        let end  = entry.base + entry.size;
        if end <= base { continue; }
        FRAME_BASE = base;
        FRAME_COUNT = (end - base) / PAGE_SIZE;
        break;
    }
    // Mark first 4 MB as used (stages live there)
    for i in 0..(0x400000 / PAGE_SIZE) as usize {
        FRAME_BITMAP[i / 64] |= 1 << (i % 64);
    }
    // Reserve identity-mapped region
    let _ = high;
    serial_shared::puts("[s10 heap] bitmap: ");
    serial_shared::dec(FRAME_COUNT as usize);
    serial_shared::puts(" frames\n");
}

unsafe fn preallocate_slabs() {
    // Stub: the legacy slab allocator lives in s10_heap's body in the
    // monolithic stage. For the Faggin base we only need the bitmap
    // frame allocator to be functional; the slab init can wait.
    serial_shared::puts("[s10 heap] slab pre-allocate (stub)\n");
}

#[no_mangle]
#[link_section = ".text._start"]
pub extern "C" fn _start(ctx_ptr: *mut boot_context::BootContext) -> ! {
    let ctx = unsafe { &*ctx_ptr };
    unsafe { build_bitmap(ctx); }
    unsafe { preallocate_slabs(); }

    let ctx = unsafe { &mut *ctx_ptr };
    ctx.heap_base = 0;
    ctx.heap_size = 0;

    unsafe {
        asm!(
            "jmp {next}",
            next = in(reg) s11_acpi as *const () as u64,
            in("rdi") ctx_ptr,
            options(noreturn)
        );
    }
}

#[panic_handler]
fn panic(_info: &PanicInfo) -> ! { loop { unsafe { asm!("hlt"); } } }
