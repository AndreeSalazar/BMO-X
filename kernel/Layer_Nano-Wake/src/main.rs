//! BMO Layer_Nano-Wake — ultra-compact micro bootstrap
//!
//! Single-purpose: paint framebuffer, show progress, jump to real kernel.
//! Zero allocations, zero strings, zero fonts, zero math.

#![no_std]
#![no_main]
#![allow(static_mut_refs)]

const BG: u32       = 0xFF0A0F1D;
const CYAN: u32     = 0xFF00E5FF;
const BAR_BG: u32   = 0xFF1E293B;
const LOGO_OUT: u32 = 0xFF4F46E5;
const LOGO_MID: u32 = 0xFF312E81;

static mut FB: u64 = 0;
static mut W: u32  = 0;
static mut H: u32  = 0;
static mut S: u32  = 0;

/// Fast full-screen clear using rep stosq (8 bytes per iteration)
fn clear_screen(color: u32) {
    let fb = unsafe { FB } as *mut u64;
    let stride = unsafe { S } as usize;
    let height = unsafe { H } as usize;
    // Pack two pixels into one u64
    let dword = ((color as u64) << 32) | color as u64;
    let count = (stride * height) / 2;
    unsafe {
        core::arch::asm!(
            "rep stosq",
            inout("rdi") fb => _,
            inout("rcx") count => _,
            in("rax") dword,
            options(nostack)
        );
    }
}

/// Horizontal line fill (single row, used by hline/fill_small)
fn hline(x: u32, y: u32, w: u32, c: u32) {
    let fb = unsafe { FB } as *mut u32;
    let stride = unsafe { S } as usize;
    let base = y as usize * stride + x as usize;
    let mut i = 0u32;
    while i < w {
        unsafe { fb.add(base + i as usize).write_volatile(c); }
        i += 1;
    }
}

/// Small rectangle fill (for logo & progress bar only)
fn fill_small(x: u32, y: u32, w: u32, h: u32, c: u32) {
    let mut row = 0u32;
    while row < h {
        hline(x, y + row, w, c);
        row += 1;
    }
}

fn outline(x: u32, y: u32, w: u32, h: u32, t: u32, c: u32) {
    fill_small(x, y, w, t, c);
    fill_small(x, y + h - t, w, t, c);
    fill_small(x, y, t, h, c);
    fill_small(x + w - t, y, t, h, c);
}

#[unsafe(no_mangle)]
#[link_section = ".text._start"]
#[unsafe(naked)]
unsafe extern "C" fn _start() -> ! {
    core::arch::naked_asm!(
        "mov r12, rdi",
        "lea rax, [rip + __bss_start]",
        "lea rcx, [rip + __bss_end]",
        "sub rcx, rax",
        "jz 2f",
        "mov rdi, rax",
        "xor eax, eax",
        "mov rdx, rcx",
        "shr rcx, 3",
        "jz 1f",
        "rep stosq",
        "1: and rdx, 7",
        "mov rcx, rdx",
        "jz 2f",
        "rep stosb",
        "2: mov rdi, r12",
        "call nano_wake_main",
        "3: hlt",
        "jmp 3b",
    );
}

#[inline(always)]
fn rdtsc() -> u64 {
    let lo: u32; let hi: u32;
    unsafe { core::arch::asm!("rdtsc", out("eax") lo, out("edx") hi, options(nostack, nomem)); }
    ((hi as u64) << 32) | lo as u64
}

#[inline(always)]
fn tsc_wait(cycles: u64) {
    let t = rdtsc();
    while rdtsc() - t < cycles { core::hint::spin_loop(); }
}

#[unsafe(no_mangle)]
extern "C" fn nano_wake_main(bi: *const bmo_boot_protocol::BootInfo) -> ! {
    if bi.is_null() { halt(); }
    let b = unsafe { &*bi };
    if b.magic != bmo_boot_protocol::BOOT_MAGIC { halt(); }

    unsafe { FB = b.fb_addr; W = b.fb_width; H = b.fb_height; S = b.fb_stride; }

    let (w, h) = unsafe { (W, H) };
    let cx = w / 2;
    let cy = h / 2;

    // Clear screen using rep stosq
    clear_screen(BG);

    // Animated Logo: concentric squares growing from inside-out
    fill_small(cx - 5, cy - 65, 10, 10, CYAN);
    tsc_wait(30_000_000);

    outline(cx - 16, cy - 76, 32, 32, 1, CYAN);
    tsc_wait(30_000_000);

    outline(cx - 24, cy - 84, 48, 48, 3, LOGO_MID);
    tsc_wait(30_000_000);

    outline(cx - 32, cy - 92, 64, 64, 2, LOGO_OUT);
    tsc_wait(30_000_000);

    // Progress bar
    let bx = cx - 160;
    let by = cy + 20;

    // Smooth progress bar slide from 0 to 100
    let mut pct = 0u32;
    while pct <= 100 {
        progress(bx, by, pct);
        tsc_wait(4_000_000); // Smooth fluid transition
        pct += 2;
    }

    let entry = b.services_entry;
    if entry == 0 { halt(); }
    unsafe {
        core::arch::asm!(
            "jmp {e}",
            e = in(reg) entry,
            in("rdi") bi,
            options(noreturn)
        );
    }
}

fn progress(bx: u32, by: u32, pct: u32) {
    fill_small(bx, by, 320, 4, BAR_BG);
    if pct > 0 {
        let fw = (320u64 * pct as u64 / 100) as u32;
        fill_small(bx, by, fw, 4, CYAN);
    }
}

#[inline(never)]
fn halt() -> ! { loop { unsafe { core::arch::asm!("hlt"); } } }

#[panic_handler]
fn panic(_: &core::panic::PanicInfo) -> ! { halt() }
