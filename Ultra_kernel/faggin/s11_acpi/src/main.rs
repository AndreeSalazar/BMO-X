//! Faggin stage 11 — ACPI RSDP scan.
//!
//! Responsibilities (one only):
//!   - Try the BootContext.rsdp hint first.
//!   - If zero, scan the EBDA pointer (0x40E) for the RSDP signature.
//!   - If still not found, scan the BIOS ROM (0xE0000..0xFFFFF).
//!   - Validate the checksum.
//!   - Publish `rsdp` in BootContext.
//!   - Jump to s12_devices.

#![no_std]
#![no_main]
#![allow(static_mut_refs)]

use core::panic::PanicInfo;
use core::arch::asm;

extern "C" {
    fn s12_devices(ctx: *mut boot_context::BootContext) -> !;
}

const RSDP_SIG: [u8; 8] = *b"RSD PTR ";

fn checksum_ok(addr: u64, len: usize) -> bool {
    let ptr = addr as *const u8;
    let mut sum: u8 = 0;
    for i in 0..len {
        sum = sum.wrapping_add(unsafe { ptr.add(i).read() });
    }
    sum == 0
}

fn matches_rsdp(addr: u64) -> bool {
    let ptr = addr as *const u8;
    for i in 0..8 {
        if unsafe { ptr.add(i).read() } != RSDP_SIG[i] { return false; }
    }
    let rev = unsafe { ptr.add(15).read() };
    let len: usize = if rev >= 2 { unsafe { ptr.add(20).read() as usize } } else { 20 };
    checksum_ok(addr, len)
}

fn scan() -> u64 {
    let ebda_seg: u16 = unsafe { (0x40E as *const u16).read() };
    let ebda_start = (ebda_seg as u64) << 4;
    let mut a = ebda_start;
    while a < ebda_start + 1024 {
        if matches_rsdp(a) { return a; }
        a += 16;
    }
    let mut a = 0xE0000u64;
    while a < 0x100000 {
        if matches_rsdp(a) { return a; }
        a += 16;
    }
    0
}

#[no_mangle]
#[link_section = ".text._start"]
pub extern "C" fn _start(ctx_ptr: *mut boot_context::BootContext) -> ! {
    let mut rsdp = unsafe { (*ctx_ptr).rsdp };
    if rsdp == 0 { rsdp = scan(); }

    let ctx = unsafe { &mut *ctx_ptr };
    ctx.rsdp = rsdp;

    if rsdp != 0 {
        serial_shared::puts("[s11 acpi] RSDP at 0x");
        serial_shared::hex(rsdp);
        serial_shared::puts("\n");
    } else {
        serial_shared::puts("[s11 acpi] RSDP not found\n");
    }

    unsafe {
        asm!(
            "jmp {next}",
            next = in(reg) s12_devices as *const () as u64,
            in("rdi") ctx_ptr,
            options(noreturn)
        );
    }
}

#[panic_handler]
fn panic(_info: &PanicInfo) -> ! { loop { unsafe { asm!("hlt"); } } }
