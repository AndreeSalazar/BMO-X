//! FastOS Kernel v0.2.0 - Entry Point
//!
//! Receives control from stage2 in 64-bit long mode, Ring 0.
//! System V AMD64 ABI: RDI=gsp_addr, RSI=gsp_size, RDX=mem_map

#![no_std]
#![no_main]

mod arch;
mod boot_info;
mod console;
mod drivers;
mod fb;
mod fs;
mod gpu;
mod render3d;
mod vga;
mod panic;
mod platform;
mod shell;
mod tests;
mod crypto;

use core::arch::asm;

/// Write a 2-character diagnostic code directly to VGA text buffer.
/// This works even if the framebuffer or serial is broken.
/// Visible at bottom-right corner of screen (row 24, col 76-77).
#[inline(always)]
fn vga_diag(c1: u8, c2: u8) {
    unsafe {
        let vga = 0xB8000 as *mut u16;
        // Row 24, col 76 = offset (24*80+76) = 1996
        vga.add(1996).write_volatile(0x4F00 | c1 as u16); // white on red
        vga.add(1997).write_volatile(0x4F00 | c2 as u16);
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn kernel_main(gsp_addr: u64, gsp_size: u64, mem_map: u64) -> ! {
    // ── Diagnostic: "K0" = kernel entry reached ──────────────────────
    vga_diag(b'K', b'0');

    // Save GSP firmware info globally
    unsafe {
        boot_info::GSP_FW_ADDR = gsp_addr;
        boot_info::GSP_FW_SIZE = gsp_size;
    }

    // ── CRITICAL: Zero BSS before any Rust code runs ─────────────────
    unsafe {
        extern "C" {
            static __bss_start: u8;
            static __bss_end: u8;
        }
        let bss_start = &__bss_start as *const u8 as *mut u8;
        let bss_end = &__bss_end as *const u8 as *mut u8;
        let len = bss_end as usize - bss_start as usize;
        core::ptr::write_bytes(bss_start, 0, len);
    }
    vga_diag(b'K', b'1'); // BSS zeroed

    // ── Set up stack at high address ───────────────────────────────────────
    unsafe {
        asm!("mov rsp, 0x200000", options(nomem, nostack));
    }

    // ── Initialize serial port for debug output ───────────────────────────
    drivers::serial::init_serial();
    drivers::serial::serial_write("[FastOS] Kernel v0.5.0 starting\n");
    vga_diag(b'K', b'2'); // Serial OK

    // ── Print GSP firmware info ──────────────────────────────────────────────
    drivers::serial::serial_write("[FastOS] GSP firmware loaded\n");
    vga_diag(b'K', b'3'); // GSP info OK

    // ── PCI scan for RTX 3060 (10DE:2504) ────────────────────────────────
    drivers::serial::serial_write("[FastOS] Scanning PCI for RTX 3060...\n");
    // TODO: Implement PCI scan
    drivers::serial::serial_write("[FastOS] PCI scan not yet implemented\n");
    vga_diag(b'K', b'4'); // PCI scan

    // ── Halt loop ──────────────────────────────────────────────────────────
    drivers::serial::serial_write("[FastOS] Halting...\n");
    loop {
        unsafe { core::arch::asm!("hlt") };
    }
}
