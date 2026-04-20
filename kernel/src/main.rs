//! FastOS Kernel v0.5.0 — Entry Point
//!
//! Receives control from UEFI bootloader in 64-bit long mode, Ring 0.
//! RDI = *const fastos_boot_protocol::BootInfo

#![no_std]
#![no_main]
#![feature(naked_functions)]

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

use core::arch::naked_asm;

/// Print a u64 as 16-digit hex to serial.
fn serial_hex(val: u64) {
    let hex = b"0123456789ABCDEF";
    drivers::serial::serial_write("0x");
    for i in (0..16).rev() {
        drivers::serial::serial_write_byte(hex[((val >> (i * 4)) & 0xF) as usize]);
    }
}

/// ELF entry point. Bootloader passes BootInfo pointer in RDI.
#[unsafe(no_mangle)]
#[link_section = ".text._start"]
#[unsafe(naked)]
unsafe extern "C" fn _start() -> ! {
    naked_asm!(
        "call kernel_main",
        "2: hlt",
        "jmp 2b",
    );
}

#[unsafe(no_mangle)]
extern "C" fn kernel_main(boot_info_ptr: *const fastos_boot_protocol::BootInfo) -> ! {
    // ── Initialize serial first for debug output ─────────────────────
    drivers::serial::init_serial();
    drivers::serial::serial_write("[FastOS] Kernel v0.5.0 starting\n");

    // ── Validate BootInfo magic ──────────────────────────────────────
    let bi = unsafe { &*boot_info_ptr };
    if bi.magic != fastos_boot_protocol::BOOT_MAGIC {
        drivers::serial::serial_write("[FastOS] FATAL: Invalid BootInfo magic: ");
        serial_hex(bi.magic);
        drivers::serial::serial_write("\n");
        loop { unsafe { core::arch::asm!("hlt"); } }
    }
    drivers::serial::serial_write("[FastOS] BootInfo valid\n");

    // ── Print boot info ──────────────────────────────────────────────
    drivers::serial::serial_write("[FastOS] FB addr: ");
    serial_hex(bi.fb_addr);
    drivers::serial::serial_write("\n");

    drivers::serial::serial_write("[FastOS] FB resolution: ");
    serial_hex(bi.fb_width as u64);
    drivers::serial::serial_write("x");
    serial_hex(bi.fb_height as u64);
    drivers::serial::serial_write("\n");

    drivers::serial::serial_write("[FastOS] Memory map entries: ");
    serial_hex(bi.memory_map_count);
    drivers::serial::serial_write("\n");

    // ── Zero BSS section ─────────────────────────────────────────────
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
    drivers::serial::serial_write("[FastOS] BSS zeroed\n");

    // ── Store boot info globally ─────────────────────────────────────
    unsafe {
        boot_info::BOOT_INFO = boot_info_ptr;
        boot_info::GSP_FW_ADDR = bi.gsp_addr;
        boot_info::GSP_FW_SIZE = bi.gsp_size;
    }

    // ── Initialize arch subsystems ───────────────────────────────────
    arch::pic::init_pic();
    arch::pic::set_mask_keyboard_timer();
    drivers::serial::serial_write("[FastOS] PIC initialized\n");

    arch::idt::init_idt();
    drivers::serial::serial_write("[FastOS] IDT loaded\n");

    arch::pit::init_pit();
    arch::idt::register_irq(0, arch::pit::tick);
    drivers::serial::serial_write("[FastOS] PIT @ 100Hz\n");

    // ── Enable interrupts ────────────────────────────────────────────
    unsafe { core::arch::asm!("sti"); }
    drivers::serial::serial_write("[FastOS] Interrupts enabled\n");

    // ── Initialize PS/2 keyboard ─────────────────────────────────────
    drivers::keyboard::init_keyboard();
    drivers::serial::serial_write("[FastOS] PS/2 keyboard ready\n");

    // ── PCI scan ─────────────────────────────────────────────────────
    drivers::serial::serial_write("[FastOS] Scanning PCI bus...\n");
    let _pci = drivers::pci::scan_pci_bus();
    drivers::serial::serial_write("[FastOS] PCI scan complete\n");

    // ── Console / Shell ──────────────────────────────────────────────
    if bi.fb_addr != 0 {
        drivers::serial::serial_write("[FastOS] Framebuffer detected, launching shell\n");
        let mut con = console::Console::new(bi.fb_addr, bi.fb_pitch());
        con.clear();
        shell::run(&mut con);
    }

    drivers::serial::serial_write("[FastOS] No framebuffer — serial-only mode\n");
    drivers::serial::serial_write("[FastOS] Halting.\n");
    loop { unsafe { core::arch::asm!("hlt"); } }
}
