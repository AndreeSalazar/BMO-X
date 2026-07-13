#![no_std]
#![no_main]

use core::panic::PanicInfo;
use core::arch::asm;
use boot_context::BootContext;

// ═══════════════════════════════════════════════════════════════════════════
// Constants
// ═══════════════════════════════════════════════════════════════════════════

const PAGE_SIZE: u64 = 4096;
const HIGH_MEM_BASE: u64 = 0xFFFF_8000_0000_0000;
const MAX_ORDER: usize = 11;

const PTE_PRESENT: u64     = 1 << 0;
const PTE_WRITABLE: u64    = 1 << 1;
const PTE_USER: u64        = 1 << 2;
const PTE_WRITE_THROUGH: u64 = 1 << 3;
const PTE_CACHE_DISABLE: u64 = 1 << 4;
const PTE_HUGE: u64        = 1 << 7;
const PTE_GLOBAL: u64      = 1 << 8;
const PTE_NO_EXECUTE: u64  = 1 << 63;

// ═══════════════════════════════════════════════════════════════════════════
// Serial I/O
// ═══════════════════════════════════════════════════════════════════════════

const COM1: u16 = 0x3F8;

fn outb(port: u16, val: u8) { unsafe { asm!("out dx, al", in("dx") port, in("al") val); } }
fn inb(port: u16) -> u8 { let v: u8; unsafe { asm!("in al, dx", in("dx") port, out("al") v); } v }

fn serial_write_byte(b: u8) {
    let mut timeout = 100_000u32;
    while inb(COM1 + 5) & 0x20 == 0 {
        timeout = timeout.saturating_sub(1);
        if timeout == 0 { return; }
    }
    outb(COM1, b);
}

fn serial_write(s: &str) {
    for b in s.bytes() {
        if b == b'\n' { serial_write_byte(b'\r'); }
        serial_write_byte(b);
    }
}

fn serial_write_u64(value: u64, min_width: usize) {
    if value == 0 {
        for _ in 0..min_width.min(1) { serial_write_byte(b'0'); }
        if min_width == 0 { serial_write_byte(b'0'); }
        return;
    }
    let mut buf = [0u8; 16];
    let mut i = 0;
    let mut v = value;
    while v > 0 {
        let digit = (v & 0xF) as u8;
        buf[i] = if digit < 10 { b'0' + digit } else { b'a' + digit - 10 };
        v >>= 4;
        i += 1;
    }
    while i < min_width && i < buf.len() { buf[i] = b'0'; i += 1; }
    while i > 0 { i -= 1; serial_write_byte(buf[i]); }
}

// ═══════════════════════════════════════════════════════════════════════════
// VMM — phys_to_virt / virt_to_phys helpers
// ═══════════════════════════════════════════════════════════════════════════

fn phys_to_virt(phys: u64) -> u64 { phys + HIGH_MEM_BASE }
fn virt_to_phys(virt: u64) -> u64 { virt - HIGH_MEM_BASE }

// ═══════════════════════════════════════════════════════════════════════════
// Frame allocator (bitmap-based, contiguous allocation support)
// ═══════════════════════════════════════════════════════════════════════════

const MAX_FRAMES: usize = 1024 * 1024; // 4 TB max addressable
static mut FRAME_BITMAP: [u8; MAX_FRAMES / 8] = [0u8; MAX_FRAMES / 8];
static mut TOTAL_FRAMES: usize = 0;
static mut USED_FRAMES: usize = 0;

unsafe fn frame_set_used(frame: usize) {
    FRAME_BITMAP[frame / 8] |= 1 << (frame % 8);
    USED_FRAMES += 1;
}

unsafe fn frame_set_free(frame: usize) {
    FRAME_BITMAP[frame / 8] &= !(1 << (frame % 8));
    USED_FRAMES -= 1;
}

fn frame_is_used(frame: usize) -> bool {
    unsafe { FRAME_BITMAP[frame / 8] & (1 << (frame % 8)) != 0 }
}

fn frame_alloc() -> Option<usize> {
    unsafe {
        for byte_idx in 0..MAX_FRAMES / 8 {
            if FRAME_BITMAP[byte_idx] != 0xFF {
                for bit in 0..8 {
                    let frame = byte_idx * 8 + bit;
                    if frame >= TOTAL_FRAMES { return None; }
                    if !frame_is_used(frame) {
                        frame_set_used(frame);
                        return Some(frame);
                    }
                }
            }
        }
    }
    None
}

fn alloc_pages_contiguous(count: usize) -> Option<u64> {
    // Find `count` consecutive free frames
    unsafe {
        let mut start = 0usize;
        while start < TOTAL_FRAMES {
            if frame_is_used(start) { start += 1; continue; }
            let mut all_free = true;
            for i in 0..count {
                if start + i >= TOTAL_FRAMES || frame_is_used(start + i) {
                    all_free = false;
                    start = start + i + 1;
                    break;
                }
            }
            if all_free {
                for i in 0..count { frame_set_used(start + i); }
                return Some((start * PAGE_SIZE as usize) as u64);
            }
        }
    }
    None
}

unsafe fn frame_free(frame: usize) {
    if frame < MAX_FRAMES && frame_is_used(frame) {
        frame_set_free(frame);
    }
}

unsafe fn alloc_zeroed_frame() -> Option<u64> {
    let f = frame_alloc()?;
    let ptr = (f * PAGE_SIZE as usize) as *mut u8;
    core::ptr::write_bytes(ptr, 0, PAGE_SIZE as usize);
    Some((f * PAGE_SIZE as usize) as u64)
}

// ═══════════════════════════════════════════════════════════════════════════
// Buddy system — for contiguous multi-page allocations
// ═══════════════════════════════════════════════════════════════════════════

const BUDDY_MIN_ORDER: usize = 0;  // 4 KB
const BUDDY_MAX_ORDER: usize = 10; // 4 MB

static mut BUDDY_LISTS: [u64; BUDDY_MAX_ORDER + 1] = [0; BUDDY_MAX_ORDER + 1];
static mut BUDDY_INIT: bool = false;

unsafe fn buddy_init() {
    if BUDDY_INIT { return; }
    // Walk all free frames and add them to the buddy system
    let mut i = 0;
    while i < TOTAL_FRAMES {
        if !frame_is_used(i) {
            // Find largest power-of-2 aligned free block
            let mut order = 0;
            while order < BUDDY_MAX_ORDER {
                let block_size = 1 << order;
                if (i & (block_size - 1)) != 0 { break; } // not aligned
                if i + block_size > TOTAL_FRAMES { break; }
                // Check all frames in block are free
                let mut all_free = true;
                for j in 0..block_size {
                    if frame_is_used(i + j) { all_free = false; break; }
                }
                if !all_free { break; }
                order += 1;
            }
            order -= 1;
            if order > 0 { // Don't bother with single pages
                let block_size = 1 << order;
                // Mark all frames as used in bitmap
                for j in 0..block_size { frame_set_used(i + j); }
                // Add to buddy free list
                let addr = (i * PAGE_SIZE as usize) as u64;
                list_push(addr, order);
                i += block_size;
            } else {
                i += 1;
            }
        } else {
            i += 1;
        }
    }
    BUDDY_INIT = true;
    serial_write("[buddy] initialized\n");
}

unsafe fn list_push(addr: u64, order: usize) {
    let ptr = phys_to_virt(addr) as *mut u64;
    ptr.write(BUDDY_LISTS[order]);
    BUDDY_LISTS[order] = addr;
}

unsafe fn list_pop(order: usize) -> Option<u64> {
    let addr = BUDDY_LISTS[order];
    if addr == 0 { return None; }
    let ptr = phys_to_virt(addr) as *mut u64;
    BUDDY_LISTS[order] = ptr.read();
    Some(addr)
}

unsafe fn buddy_alloc(order: usize) -> Option<u64> {
    // Search from requested order upward
    for o in order..=BUDDY_MAX_ORDER {
        if let Some(block) = list_pop(o) {
            // Split descending to the requested order
            let remaining = block;
            for split_o in (order + 1..=o).rev() {
                let buddy = remaining + (PAGE_SIZE as u64) * (1u64 << (split_o - 1));
                list_push(buddy, split_o - 1);
            }
            return Some(remaining);
        }
    }
    None
}

unsafe fn buddy_free(addr: u64, order: usize) {
    let mut current = addr;
    let mut cur_order = order;
    while cur_order < BUDDY_MAX_ORDER {
        let block_size = (PAGE_SIZE as u64) * (1u64 << cur_order);
        let buddy = current ^ block_size; // XOR across the block boundary
        // Check if buddy is in our free list (simplified: scan)
        let mut found = false;
        let mut prev = &mut BUDDY_LISTS[cur_order];
        let mut next = BUDDY_LISTS[cur_order];
        while next != 0 {
            if next == buddy {
                // Remove buddy from list
                let next_ptr = phys_to_virt(next) as *mut u64;
                *prev = next_ptr.read();
                found = true;
                break;
            }
            prev = &mut *(phys_to_virt(next) as *mut u64);
            next = *prev;
        }
        if found {
            current = core::cmp::min(current, buddy);
            cur_order += 1;
        } else {
            break;
        }
    }
    list_push(current, cur_order);
}

unsafe fn buddy_has(addr: u64, order: usize) -> bool {
    let mut next = BUDDY_LISTS[order];
    while next != 0 {
        if next == addr { return true; }
        let ptr = phys_to_virt(next) as *const u64;
        next = *ptr;
    }
    false
}

/// Allocate 2^order consecutive pages from buddy system.
#[no_mangle]
pub unsafe extern "C" fn alloc_order(order: usize) -> u64 {
    buddy_alloc(order).unwrap_or(0)
}

/// Free 2^order pages back to buddy system.
#[no_mangle]
pub unsafe extern "C" fn free_order(addr: u64, order: usize) {
    buddy_free(addr, order);
}

// ═══════════════════════════════════════════════════════════════════════════
// VMM — Page table operations
// ═══════════════════════════════════════════════════════════════════════════

const fn pte_addr(entry: u64) -> u64 { entry & 0x000F_FFFF_FFFF_F000 }

unsafe fn get_or_create_table(table: *mut u64, index: usize) -> Option<*mut u64> {
    let entry = table.add(index).read();
    if entry & PTE_PRESENT == 0 {
        let page = alloc_zeroed_frame()?;
        table.add(index).write(page | PTE_PRESENT | PTE_WRITABLE);
    }
    let phys = pte_addr(table.add(index).read());
    Some(phys_to_virt(phys) as *mut u64)
}

unsafe fn map_page_internal(pml4: *mut u64, virt: u64, phys: u64, flags: u64) -> bool {
    let i4 = ((virt >> 39) & 0x1FF) as usize;
    let i3 = ((virt >> 30) & 0x1FF) as usize;
    let i2 = ((virt >> 21) & 0x1FF) as usize;
    let i1 = ((virt >> 12) & 0x1FF) as usize;

    let pdpt = match get_or_create_table(pml4, i4) { Some(t) => t, None => return false };
    let pd = match get_or_create_table(pdpt, i3) { Some(t) => t, None => return false };
    let pt = match get_or_create_table(pd, i2) { Some(t) => t, None => return false };
    let entry = (phys & 0x000F_FFFF_FFFF_F000) | flags;
    pt.add(i1).write(entry);
    true
}

unsafe fn unmap_page(pml4: *mut u64, virt: u64) {
    let i4 = ((virt >> 39) & 0x1FF) as usize;
    let i3 = ((virt >> 30) & 0x1FF) as usize;
    let i2 = ((virt >> 21) & 0x1FF) as usize;
    let i1 = ((virt >> 12) & 0x1FF) as usize;

    let pdpt = match get_or_create_table(pml4, i4) { Some(t) => t, None => return };
    let pd = match get_or_create_table(pdpt, i3) { Some(t) => t, None => return };
    let pt = match get_or_create_table(pd, i2) { Some(t) => t, None => return };
    pt.add(i1).write(0);
}

/// Map physical memory into the higher half (identity map for first 4GB + high mem)
unsafe fn map_high_mem(pml4: *mut u64, ctx: &BootContext) {
    // Identity map first 4MB (contains stage1-3 code)
    for addr in (0..0x400000u64).step_by(PAGE_SIZE as usize) {
        map_page_internal(pml4, addr, addr, PTE_PRESENT | PTE_WRITABLE);
    }

    // Map full higher-half mirror (first 2GB of phys → HIGH_MEM_BASE + phys)
    for addr in (0..0x80000000u64).step_by(PAGE_SIZE as usize) {
        let virt = HIGH_MEM_BASE + addr;
        map_page_internal(pml4, virt, addr, PTE_PRESENT | PTE_WRITABLE | PTE_GLOBAL);
    }

    // Map framebuffer (identity + higher-half with WC attributes)
    if ctx.fb_addr != 0 {
        let fb_size = (ctx.fb_stride * ctx.fb_height * 4) as u64;
        let fb_start = ctx.fb_addr & !(PAGE_SIZE - 1);
        let fb_end = (ctx.fb_addr + fb_size + PAGE_SIZE - 1) & !(PAGE_SIZE - 1);
        for addr in (fb_start..fb_end).step_by(PAGE_SIZE as usize) {
            map_page_internal(pml4, addr, addr, PTE_PRESENT | PTE_WRITABLE | PTE_CACHE_DISABLE);
            let virt = HIGH_MEM_BASE + addr;
            map_page_internal(pml4, virt, addr, PTE_PRESENT | PTE_WRITABLE | PTE_CACHE_DISABLE | PTE_GLOBAL);
        }
    }

    serial_write("[vmm] Higher-half mapped (0xFFFF800000000000)\n");
}

unsafe fn flush_tlb() {
    let cr3: u64;
    asm!("mov {}, cr3", out(reg) cr3);
    asm!("mov cr3, {}", in(reg) cr3);
}

// ═══════════════════════════════════════════════════════════════════════════
// Slab Heap — per-size class allocator
// ═══════════════════════════════════════════════════════════════════════════

const SLAB_SIZE: usize = 4096;
const CACHE_COUNT: usize = 8;
const CACHE_SIZES: [usize; CACHE_COUNT] = [16, 32, 64, 128, 256, 512, 1024, 2048];
const BUDDY_FALLBACK: usize = CACHE_COUNT; // index for large allocs

struct SlabHead {
    next: *mut SlabHead,
    obj_size: u16,
    free_count: u16,
    first_free: u16, // index of first free object, or u16::MAX
}

struct SlabCache {
    obj_size: usize,
    head: Option<&'static mut SlabHead>,
}

static mut CACHES: [SlabCache; CACHE_COUNT + 1] = [
    SlabCache { obj_size: 16, head: None },
    SlabCache { obj_size: 32, head: None },
    SlabCache { obj_size: 64, head: None },
    SlabCache { obj_size: 128, head: None },
    SlabCache { obj_size: 256, head: None },
    SlabCache { obj_size: 512, head: None },
    SlabCache { obj_size: 1024, head: None },
    SlabCache { obj_size: 2048, head: None },
    SlabCache { obj_size: 0, head: None }, // buddy fallback
];

fn cache_for_size(size: usize) -> usize {
    for i in 0..CACHE_COUNT {
        if size <= CACHE_SIZES[i] { return i; }
    }
    BUDDY_FALLBACK
}

unsafe fn slab_create(cache_idx: usize) -> Option<*mut SlabHead> {
    let obj_size = CACHE_SIZES[cache_idx];
    let obj_per_slab = (SLAB_SIZE - core::mem::size_of::<SlabHead>()) / obj_size;
    if obj_per_slab == 0 { return None; }

    let page = alloc_zeroed_frame()?;
    let head = phys_to_virt(page) as *mut SlabHead;
    *head = SlabHead {
        next: core::ptr::null_mut(),
        obj_size: obj_size as u16,
        free_count: obj_per_slab as u16,
        first_free: 0,
    };

    // Build free list
    let base = head.add(1) as *mut u8;
    for i in 0..obj_per_slab - 1 {
        let ptr = base.add(i * obj_size) as *mut u16;
        *ptr = (i + 1) as u16;
    }
    // Last object points to end marker
    let last = base.add((obj_per_slab - 1) * obj_size) as *mut u16;
    *last = u16::MAX;

    Some(head)
}

unsafe fn cache_alloc(cache_idx: usize) -> *mut u8 {
    let cache = &mut CACHES[cache_idx];

    // Find a slab with free objects
    if let Some(ref mut head) = cache.head {
        if head.free_count > 0 {
            let idx = head.first_free as usize;
            let obj_size = head.obj_size as usize;
            let base = (*head as *mut SlabHead).add(1) as *mut u8;
            let ptr = base.add(idx * obj_size);

            head.first_free = *(ptr as *const u16);
            head.free_count -= 1;

            return ptr;
        }
    }

    // Create a new slab
    let new_slab = match slab_create(cache_idx) { Some(s) => s, None => return core::ptr::null_mut() };
    let head = &mut *new_slab;
    head.next = cache.head.take().map(|h| h as *mut SlabHead).unwrap_or(core::ptr::null_mut());
    cache.head = Some(&mut *new_slab);

    // Allocate from the new slab
    let idx = head.first_free as usize;
    let obj_size = head.obj_size as usize;
    let base = new_slab.add(1) as *mut u8;
    let ptr = base.add(idx * obj_size);
    head.first_free = *(ptr as *const u16);
    head.free_count -= 1;

    ptr
}

unsafe fn slab_buddy_alloc(size: usize, align: usize) -> *mut u8 {
    let pages = (size + SLAB_SIZE - 1) / SLAB_SIZE;
    let order = (pages.next_power_of_two()).trailing_zeros() as usize;
    let phys = buddy_alloc(order).unwrap_or(0);
    if phys == 0 { return core::ptr::null_mut(); }
    let virt = phys_to_virt(phys) as *mut u8;
    let aligned = (virt as u64 + align as u64 - 1) & !(align as u64 - 1);
    aligned as *mut u8
}

#[no_mangle]
pub unsafe extern "C" fn heap_alloc(size: usize, align: usize) -> *mut u8 {
    if size > CACHE_SIZES[CACHE_COUNT - 1] {
        return slab_buddy_alloc(size, align);
    }
    let ci = cache_for_size(size);
    cache_alloc(ci)
}

#[no_mangle]
pub unsafe extern "C" fn heap_free(_ptr: *mut u8, _size: usize, _align: usize) {
    // Slab allocator — no per-object free in simplified version
    // Memory is reclaimed when all objects in a slab are freed (not implemented)
}

// ═══════════════════════════════════════════════════════════════════════════
// ACPI — scan for RSDP to get memory map info
// ═══════════════════════════════════════════════════════════════════════════

const RSDP_SIGNATURE: [u8; 8] = *b"RSD PTR ";

fn acpi_find_rsdp(ctx: &BootContext) -> u64 {
    // First try BootContext hint
    if ctx.rsdp != 0 { return ctx.rsdp; }

    // Scan EBDA (Extended BIOS Data Area)
    let ebda_seg = unsafe {
        let ptr = 0x40E as *const u16;
        ptr.read() as u64
    };
    let ebda_start = (ebda_seg as u64) << 4;
    for addr in (ebda_start..ebda_start + 1024).step_by(16) {
        if check_rsdp(addr) { return addr; }
    }

    // Scan BIOS area 0xE0000-0xFFFFF
    for addr in (0xE0000u64..0x100000u64).step_by(16) {
        if check_rsdp(addr) { return addr; }
    }

    0
}

fn check_rsdp(addr: u64) -> bool {
    unsafe {
        let ptr = addr as *const u8;
        for i in 0..8 {
            if ptr.add(i).read() != RSDP_SIGNATURE[i] { return false; }
        }
        true
    }
}

unsafe fn parse_rsdt(rsdp_addr: u64) {
    let rev = (rsdp_addr as *const u8).add(15).read();
    let xsdt_addr = if rev >= 2 {
        let ptr = rsdp_addr as *const u64;
        let len_ptr = (rsdp_addr + 16) as *const u8;
        if len_ptr.read() >= 24 {
            ptr.add(3).read() // XSDT address at offset 24
        } else { 0 }
    } else {
        let ptr = rsdp_addr as *const u32;
        ptr.add(2).read() as u64 // RSDT address at offset 16
    };

    if xsdt_addr != 0 {
        let header = xsdt_addr as *const AcpiSdtHeader;
        let entries = ((*header).length as usize - core::mem::size_of::<AcpiSdtHeader>()) / 8;
        for i in 0..entries {
            let entry_addr = (xsdt_addr + core::mem::size_of::<AcpiSdtHeader>() as u64) as *const u64;
            let _tbl_addr = entry_addr.add(i).read();
            // Check for MADT and other tables (stage3 handles them)
        }
    }
}

#[repr(C, packed)]
struct AcpiSdtHeader {
    signature: [u8; 4],
    length: u32,
    revision: u8,
    checksum: u8,
    oem_id: [u8; 6],
    oem_table_id: [u8; 8],
    oem_revision: u32,
    creator_id: [u8; 4],
    creator_revision: u32,
}

// ═══════════════════════════════════════════════════════════════════════════
// Entry point
// ═══════════════════════════════════════════════════════════════════════════

#[no_mangle]
#[link_section = ".text._start"]
pub extern "C" fn _start(ctx_ptr: *mut BootContext) -> ! {
    let ctx = unsafe { &mut *ctx_ptr };
    serial_write("\n[stage2] Memory init — page tables, frame allocator, heap\n");

    // ── 1. Initialize frame allocator from memory map ──────
    for i in 0..ctx.memory_map_count as usize {
        let entry = &ctx.memory_map[i];
        if entry.kind == 1 && entry.size > 0 {
            let _start_frame = (entry.base / PAGE_SIZE) as usize;
            let end_frame = ((entry.base + entry.size) / PAGE_SIZE) as usize;
            unsafe {
                if end_frame > MAX_FRAMES { continue; }
                if end_frame > TOTAL_FRAMES { TOTAL_FRAMES = end_frame; }
            }
        }
    }

    // Mark first 4MB (stages) and framebuffer as used
    for frame in 0..(0x400000 / PAGE_SIZE as u64) as usize {
        unsafe { frame_set_used(frame); }
    }
    if ctx.fb_addr != 0 {
        let fb_size = (ctx.fb_stride * ctx.fb_height * 4) as u64;
        let fb_start = ctx.fb_addr & !(PAGE_SIZE - 1);
        let fb_end = (ctx.fb_addr + fb_size + PAGE_SIZE - 1) & !(PAGE_SIZE - 1);
        for frame in (fb_start / PAGE_SIZE) as usize..(fb_end / PAGE_SIZE) as usize {
            unsafe { frame_set_used(frame); }
        }
    }

    serial_write("[stage2] Frame allocator: ");
    serial_write_u64(unsafe { TOTAL_FRAMES as u64 }, 10);
    serial_write(" frames (");
    serial_write_u64(unsafe { TOTAL_FRAMES as u64 * PAGE_SIZE as u64 / (1024 * 1024) }, 10);
    serial_write(" MB)\n");

    // ── 2. Build PML4 ──────────────────────────────────────
    let pml4 = unsafe {
        let page = alloc_zeroed_frame().unwrap();
        page as *mut u64
    };

    serial_write("[stage2] PML4 at phys 0x");
    serial_write_u64(pml4 as u64, 16);
    serial_write("\n");

    // ── 3. Map everything ──────────────────────────────────
    unsafe { map_high_mem(pml4, ctx); }

    // ── 4. Load CR3 ────────────────────────────────────────
    let pml4_phys = pml4 as u64;
    unsafe {
        asm!("mov cr3, {}", in(reg) pml4_phys);
        ctx.pml4 = pml4_phys;
    }
    serial_write("[stage2] PML4 loaded into CR3\n");

    // ── 5. Initialize buddy allocator ──────────────────────
    unsafe { buddy_init(); }

    // ── 6. Initialize heap (slab) ──────────────────────────
    // Pre-allocate slabs for each size class
    for ci in 0..CACHE_COUNT {
        for _ in 0..4 {
            unsafe { slab_create(ci); }
        }
    }
    serial_write("[stage2] Slab heap initialized (8 size classes)\n");

    // ── 7. Store heap info in context ──────────────────────
    ctx.heap_base = 0; // heap uses buddy + slab, not a fixed region
    ctx.heap_size = 0;

    // ── 8. ACPI RSDP scan ──────────────────────────────────
    let rsdp = acpi_find_rsdp(ctx);
    if rsdp != 0 {
        ctx.rsdp = rsdp;
        serial_write("[stage2] ACPI RSDP at 0x");
        serial_write_u64(rsdp, 16);
        serial_write("\n");
        unsafe { parse_rsdt(rsdp); }
    } else {
        serial_write("[stage2] ACPI RSDP not found\n");
    }

    serial_write("[stage2] Context updated, jumping to stage3\n");

    // ── Jump to Stage 3 ──────────────────────────────────
    let stage3_entry = ctx.stage_entry[2];
    if stage3_entry != 0 {
        unsafe {
            let stage3_fn: extern "C" fn(*mut BootContext) -> ! =
                core::mem::transmute(stage3_entry);
            stage3_fn(ctx_ptr);
        }
    }

    loop { unsafe { asm!("hlt"); } }
}

#[panic_handler]
fn panic(_info: &PanicInfo) -> ! {
    loop { unsafe { asm!("hlt"); } }
}
