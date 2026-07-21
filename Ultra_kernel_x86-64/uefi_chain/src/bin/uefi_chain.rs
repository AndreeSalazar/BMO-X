//! BMO-X unified UEFI shim.
//!
//! s1_cpu, s2_mem and the kernel are **embedded in this PE** at build time
//! (`include_bytes!` from paths in BMO_S1_BIN / BMO_S2_BIN / BMO_KERNEL_BIN,
//! set by build.ps1). The shim only needs two firmware services — console
//! output and AllocatePages — so it boots even on firmwares that never
//! bind SimpleFileSystem to any handle (MSI A320M AMI fast path: the boot
//! manager reads FAT with an internal reader; LocateHandle(ByProtocol,
//! SimpleFS) returns NOT_FOUND and HandleProtocol on the boot device
//! returns UNSUPPORTED, even after a recursive ConnectController pass).
//!
//! Layout contract (mirrors s1_cpu's reserves):
//!   s1_cpu  @ 0x100000, slot 512 KiB (bin + zeroed .bss)
//!   s2_mem  @ 0x200000, slot   2 MiB
//!   kernel  @ 0x400000, slot  16 MiB
//!
//! s1 receives a `boot_context::PreloadInfo` pointer in r8 and skips its
//! ESP loader when the magic matches.

#![no_std]
#![no_main]
#![allow(unsafe_op_in_unsafe_fn)]

use core::arch::asm;

use boot_context::{PreloadInfo, PRELOAD_MAGIC};

type EfiHandle = *mut core::ffi::c_void;
type EfiStatus = u64;

const EFI_SUCCESS: u64 = 0;
const COM1: u16 = 0x3F8;

const S1_ADDR: u64 = 0x100000;
const S1_SLOT: u64 = 512 * 1024;
const S2_ADDR: u64 = 0x200000;
const S2_SLOT: u64 = 2 * 1024 * 1024;
const KERNEL_ADDR: u64 = 0x400000;
const KERNEL_SLOT: u64 = 16 * 1024 * 1024;

static S1_BIN: &[u8] = include_bytes!(env!("BMO_S1_BIN"));
static S2_BIN: &[u8] = include_bytes!(env!("BMO_S2_BIN"));
static KERNEL_BIN: &[u8] = include_bytes!(env!("BMO_KERNEL_BIN"));

#[repr(C)] struct EfiTableHeader { signature: u64, revision: u32, header_size: u32, crc32: u32, _r: u32 }
#[repr(C)] struct EfiBootServices { hdr: EfiTableHeader, _pad: [u8; 44 * 8] }
#[repr(C)] struct EfiSystemTable { hdr: EfiTableHeader, _firmware: *mut core::ffi::c_void, _firmware_revision: u32, _firmware_pad: u32, _cin: EfiHandle, _cin_h: *mut core::ffi::c_void, _cout: EfiHandle, _cout_h: *mut core::ffi::c_void, _cerr: EfiHandle, _cerr_h: *mut core::ffi::c_void, _rt: *mut core::ffi::c_void, bs: *mut EfiBootServices, _nt: usize, _ct: *mut core::ffi::c_void }

#[inline] unsafe fn outb(p: u16, v: u8) { asm!("out dx, al", in("dx") p, in("al") v); }
#[inline] unsafe fn inb(p: u16) -> u8 { let v: u8; asm!("in al, dx", in("dx") p, out("al") v); v }
unsafe fn put(b: u8) { let mut t = 100_000u32; while inb(COM1+5) & 0x20 == 0 { t = t.saturating_sub(1); if t == 0 { return; } } outb(COM1, b); }
fn puts(s: &str) { unsafe { for &b in s.as_bytes() { if b == b'\n' { put(b'\r'); } put(b); } } }
fn hex(mut v: u64) { unsafe { if v == 0 { put(b'0'); return; } let mut b = [0u8; 16]; let mut i = 0; while v > 0 { b[i] = b"0123456789abcdef"[(v & 0xF) as usize]; v >>= 4; i += 1; } for j in (0..i).rev() { put(b[j]); } } }
fn dec(mut v: usize) { unsafe { if v == 0 { put(b'0'); return; } let mut b = [0u8; 20]; let mut i = 0; while v > 0 { b[i] = b'0' + (v % 10) as u8; v /= 10; i += 1; } for j in (0..i).rev() { put(b[j]); } } }
fn init_serial() { unsafe { outb(COM1+1, 0); outb(COM1+3, 0x80); outb(COM1+0, 1); outb(COM1+1, 0); outb(COM1+3, 3); outb(COM1+2, 0xC7); outb(COM1+4, 0xB); } }

// ── ConOut: visible screen output (serial alone hides errors from the user) ──

unsafe fn con_call1(st: *const EfiSystemTable, index: usize, arg: *const u16) {
    let con = (*st)._cout_h as *const *mut core::ffi::c_void;
    if con.is_null() { return; }
    let f: extern "efiapi" fn(*mut core::ffi::c_void, *const u16) -> EfiStatus =
        core::mem::transmute(*con.add(index));
    f((*st)._cout_h as *mut core::ffi::c_void, arg);
}

/// Print ASCII on the firmware console (index 1 = OutputString).
unsafe fn scr(st: *const EfiSystemTable, s: &str) {
    let mut buf = [0u16; 96];
    let mut i = 0;
    for &b in s.as_bytes() {
        if i >= buf.len() - 2 { buf[i] = 0; con_call1(st, 1, buf.as_ptr()); i = 0; }
        if b == b'\n' { buf[i] = b'\r' as u16; i += 1; }
        buf[i] = b as u16; i += 1;
    }
    buf[i] = 0;
    con_call1(st, 1, buf.as_ptr());
}

/// Decimal on the firmware console.
unsafe fn sdec(st: *const EfiSystemTable, v: usize) {
    let mut d = [0u8; 20]; let mut n = v; let mut i = 0;
    if n == 0 { d[0] = b'0'; i = 1; } else { while n > 0 { d[i] = b'0' + (n % 10) as u8; n /= 10; i += 1; } }
    let mut buf = [0u16; 21];
    for j in 0..i { buf[j] = d[i - 1 - j] as u16; }
    buf[i] = 0;
    con_call1(st, 1, buf.as_ptr());
}

/// SetAttribute (index 5). 0x0A = light green, 0x0C = light red, 0x07 = grey.
unsafe fn scr_attr(st: *const EfiSystemTable, attr: usize) {
    let con = (*st)._cout_h as *const *mut core::ffi::c_void;
    if con.is_null() { return; }
    let f: extern "efiapi" fn(*mut core::ffi::c_void, usize) -> EfiStatus =
        core::mem::transmute(*con.add(5));
    f((*st)._cout_h as *mut core::ffi::c_void, attr);
}

/// Visible failure: red message on screen + serial, then a 5 s pause so the
/// firmware's return-to-menu cannot hide what happened.
unsafe fn fail(st: *const EfiSystemTable, base: *const *mut core::ffi::c_void, msg: &str) {
    puts("[uefi] FAIL: "); puts(msg); puts("\n");
    scr_attr(st, 0x0C);
    scr(st, "\n [BMO-X] FAIL: "); scr(st, msg); scr(st, "\n");
    scr_attr(st, 0x07);
    let stall: extern "efiapi" fn(usize) -> EfiStatus = core::mem::transmute(*base.add(3 + 28));
    stall(5_000_000);
}

static mut PRELOAD: PreloadInfo = PreloadInfo { magic: 0, s2_size: 0, kernel_size: 0, kernel_src: 0 };

/// Reserve `slot` bytes at fixed `addr`, copy `blob` there and zero the
/// rest of the slot (the stages' .bss contract).
unsafe fn load_slot(
    st: *const EfiSystemTable,
    base: *const *mut core::ffi::c_void,
    addr: u64,
    slot: u64,
    blob: &[u8],
    name: &str,
) -> bool {
    if blob.len() as u64 > slot {
        fail(st, base, "embedded stage exceeds its slot");
        return false;
    }
    let alloc_p: extern "efiapi" fn(u32, u32, usize, &mut u64) -> EfiStatus =
        core::mem::transmute(*base.add(3 + 2));
    let mut alloc = addr;
    // 2 = AllocateAddress, 2 = EfiLoaderData
    if alloc_p(2, 2, ((slot as usize) + 0xFFF) / 0x1000, &mut alloc) != EFI_SUCCESS {
        fail(st, base, "cannot reserve a fixed stage address");
        return false;
    }
    let dst = addr as *mut u8;
    core::ptr::copy_nonoverlapping(blob.as_ptr(), dst, blob.len());
    core::ptr::write_bytes(dst.add(blob.len()), 0, (slot as usize) - blob.len());
    puts("[uefi] "); puts(name); puts("="); dec(blob.len()); puts(" bytes at 0x"); hex(addr); puts("\n");
    scr(st, "  "); scr(st, name); scr(st, ": "); sdec(st, blob.len()); scr(st, " bytes\n");
    true
}

#[unsafe(no_mangle)]
pub extern "efiapi" fn efi_main(image_handle: EfiHandle, system_table: *mut core::ffi::c_void) -> EfiStatus {
    init_serial();
    puts("\n[uefi] BMO-X unified shim\n");

    let st = system_table as *const EfiSystemTable;
    let bs = unsafe { (*st).bs };
    let base = unsafe { &(*bs).hdr } as *const EfiTableHeader as *const *mut core::ffi::c_void;

    // Green BMO-X banner: the boot is visible even without a serial cable.
    unsafe {
        scr_attr(st, 0x0A);
        scr(st, "\n  ==========================================\n");
        scr(st, "    B M O - X   |   Ultra Kernel x86-64\n");
        scr(st, "  ==========================================\n\n");
        scr_attr(st, 0x07);
    }

    // Everything this shim needs is already inside its own image: no
    // filesystem, no probing, no firmware quirks.
    if !unsafe { load_slot(st, base, S1_ADDR, S1_SLOT, S1_BIN, "s1_cpu") } { return 1; }
    if !unsafe { load_slot(st, base, S2_ADDR, S2_SLOT, S2_BIN, "s2_mem") } { return 2; }
    // The kernel is NOT copied to its 16 MiB slot here: firmwares keep
    // Boot Services allocations in that range (AllocateAddress fails on
    // the MSI A320M). It stays inside this image; s1 copies it to
    // 0x400000 immediately after ExitBootServices, when the range is ours.
    if KERNEL_BIN.len() as u64 > KERNEL_SLOT {
        unsafe { fail(st, base, "kernel exceeds its slot") };
        return 3;
    }
    let _ = KERNEL_ADDR;

    unsafe {
        scr(st, "  kernel: "); sdec(st, KERNEL_BIN.len()); scr(st, " bytes (staged)\n");
        PRELOAD = PreloadInfo {
            magic: PRELOAD_MAGIC,
            s2_size: S2_BIN.len() as u64,
            kernel_size: KERNEL_BIN.len() as u64,
            kernel_src: KERNEL_BIN.as_ptr() as u64,
        };
        scr_attr(st, 0x0A);
        scr(st, "  handoff -> s1_cpu\n");
        scr_attr(st, 0x07);
    }

    puts("[uefi] ===> JUMP s1_cpu 0x"); hex(S1_ADDR); puts("\n");
    unsafe { asm!("sfence", options(nostack, preserves_flags)); }

    // s1 still needs GOP and memory-map services. Keep Boot Services alive
    // and use a typed EFI call so the compiler emits the Microsoft x64
    // argument registers and stack alignment required by UEFI. The third
    // argument (r8) is the preload handoff.
    let entry: extern "efiapi" fn(EfiHandle, *mut core::ffi::c_void, *const PreloadInfo) -> ! =
        unsafe { core::mem::transmute(S1_ADDR as usize) };
    entry(image_handle, system_table, unsafe { core::ptr::addr_of!(PRELOAD) })
}

#[panic_handler]
fn panic(_info: &core::panic::PanicInfo) -> ! { loop { unsafe { asm!("hlt"); } } }
