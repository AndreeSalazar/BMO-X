//! Faggin stage 4 — CPUID detection (vendor, brand, features).
//!
//! Responsibilities (one only):
//!   - Read vendor string (leaf 0).
//!   - Read brand string (leaves 0x80000002..04).
//!   - Read feature bits (leaf 1, leaf 7).
//!   - Log them to serial.
//!   - Jump to s5_control.
//!
//! Writes nothing to BootContext — s5 reads the same CPUID outputs
//! directly from the CPU (CPUID is idempotent).

#![no_std]
#![no_main]
#![allow(static_mut_refs)]

use core::panic::PanicInfo;
use core::arch::asm;

extern "C" {
    fn s5_control(ctx: *mut boot_context::BootContext) -> !;
}

#[inline]
fn cpuid(leaf: u32, sub: u32) -> (u32, u32, u32, u32) {
    let (eax, ebx, ecx, edx);
    unsafe {
        asm!(
            "push rbx",
            "cpuid",
            "mov {ebx_out:e}, ebx",
            "pop rbx",
            inout("eax") leaf => eax,
            inout("ecx") sub => ecx,
            ebx_out = out(reg) ebx,
            out("edx") edx,
        );
    }
    (eax, ebx, ecx, edx)
}

fn detect_vendor() -> [u8; 12] {
    let (_, ebx, ecx, edx) = cpuid(0, 0);
    let mut v = [0u8; 12];
    v[0..4].copy_from_slice(&ebx.to_ne_bytes());
    v[4..8].copy_from_slice(&edx.to_ne_bytes());
    v[8..12].copy_from_slice(&ecx.to_ne_bytes());
    v
}

fn print_brand() {
    let (a, b, c, d) = cpuid(0x80000002, 0);
    let (e, f, g, h) = cpuid(0x80000003, 0);
    let (i, j, k, l) = cpuid(0x80000004, 0);
    let mut buf = [0u8; 48];
    let mut idx = 0;
    for v in [a, b, c, d, e, f, g, h, i, j, k, l] {
        if idx < 48 { buf[idx] = v as u8; idx += 1; }
        if v > 0xFF && idx < 48 { buf[idx] = (v >> 8) as u8; idx += 1; }
        if v > 0xFFFF && idx < 48 { buf[idx] = (v >> 16) as u8; idx += 1; }
        if v > 0xFFFFFF && idx < 48 { buf[idx] = (v >> 24) as u8; idx += 1; }
    }
    serial_shared::puts("[s4 cpuid] ");
    if let Ok(s) = core::str::from_utf8(&buf[..idx.min(48)]) {
        serial_shared::puts(s.trim_end_matches('\0').trim_end());
    }
    serial_shared::puts("\n");
}

#[no_mangle]
#[link_section = ".text._start"]
pub extern "C" fn _start(ctx_ptr: *mut boot_context::BootContext) -> ! {
    let vendor = detect_vendor();
    serial_shared::puts("[s4 cpuid] vendor: ");
    if let Ok(s) = core::str::from_utf8(&vendor) {
        serial_shared::puts(s.trim_end_matches('\0'));
    }
    serial_shared::puts("\n");
    print_brand();

    // Print feature flags (so the user sees them in serial)
    let (_, _, ecx1, edx1) = cpuid(1, 0);
    let (eax7, ebx7, _, _) = cpuid(7, 0);
    serial_shared::puts("[s4 cpuid] features: XSAVE=");
    serial_shared::puts(if ecx1 & (1 << 26) != 0 { "Y " } else { "N " });
    serial_shared::puts("AVX=");
    serial_shared::puts(if ecx1 & (1 << 28) != 0 { "Y " } else { "N " });
    serial_shared::puts("SMEP=");
    serial_shared::puts(if ebx7 & (1 << 7) != 0 { "Y " } else { "N " });
    serial_shared::puts("FSGSBASE=");
    serial_shared::puts(if ebx7 & (1 << 0) != 0 { "Y " } else { "N " });
    serial_shared::puts("UMIP=");
    serial_shared::puts(if eax7 & (1 << 2) != 0 { "Y" } else { "N" });
    serial_shared::puts("\n");

    unsafe {
        asm!(
            "jmp {next}",
            next = in(reg) s5_control as *const () as u64,
            in("rdi") ctx_ptr,
            options(noreturn)
        );
    }
}

#[panic_handler]
fn panic(_info: &PanicInfo) -> ! { loop { unsafe { asm!("hlt"); } } }
