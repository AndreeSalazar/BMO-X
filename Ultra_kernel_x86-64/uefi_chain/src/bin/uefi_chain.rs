#![no_std]
#![no_main]
#![allow(unsafe_op_in_unsafe_fn)]

use core::arch::asm;

type EfiHandle = *mut core::ffi::c_void;
type EfiStatus = u64;

const EFI_SUCCESS: u64 = 0;
const COM1: u16 = 0x3F8;
const S1_ADDR: u64 = 0x100000;
// s1_cpu's .bss is ~296 KB (BootContext + GDT + IDT + TSS + FILE_BUF 256KB
// + stacks + bitmap). Allocate 512 KB and zero the ENTIRE slot so .bss is clean.
const S1_SLOT: u64 = 512 * 1024;
const FILE_BUF_SIZE: usize = 64 * 1024;
static mut FILE_BUF: [u8; FILE_BUF_SIZE] = [0; FILE_BUF_SIZE];

#[repr(C)] struct EfiTableHeader { signature: u64, revision: u32, header_size: u32, crc32: u32, _r: u32 }
#[repr(C)] struct EfiBootServices { hdr: EfiTableHeader, _pad: [u8; 44 * 8] }
#[repr(C)] struct EfiSystemTable { hdr: EfiTableHeader, _firmware: *mut core::ffi::c_void, _firmware_revision: u32, _firmware_pad: u32, _cin: EfiHandle, _cin_h: *mut core::ffi::c_void, _cout: EfiHandle, _cout_h: *mut core::ffi::c_void, _cerr: EfiHandle, _cerr_h: *mut core::ffi::c_void, _rt: *mut core::ffi::c_void, bs: *mut EfiBootServices, _nt: usize, _ct: *mut core::ffi::c_void }
#[repr(C)] struct EfiGuid { d1: u32, d2: u16, d3: u16, d4: [u8; 8] }
#[repr(C)] struct EfiSimpleFileSystemProtocol { revision: u64, open_volume: unsafe extern "efiapi" fn(*const Self, *mut *mut core::ffi::c_void) -> EfiStatus }
#[repr(C)] struct EfiFileProtocol { revision: u64, open: unsafe extern "efiapi" fn(*const Self, *mut *mut core::ffi::c_void, *const u16, u64, u64) -> EfiStatus, close: unsafe extern "efiapi" fn(*const Self) -> EfiStatus, _del: *mut core::ffi::c_void, read: *mut core::ffi::c_void, _w: *mut core::ffi::c_void, _gp: *mut core::ffi::c_void, _sp: *mut core::ffi::c_void, _gi: *mut core::ffi::c_void, _si: *mut core::ffi::c_void, _f: *mut core::ffi::c_void }

static mut FS_GUID: EfiGuid = EfiGuid { d1: 0x964e5b22, d2: 0x6409, d3: 0x47ef, d4: [0x97, 0xa2, 0xff, 0x06, 0xff, 0x38, 0xb0, 0xdf] };

#[inline] unsafe fn outb(p: u16, v: u8) { asm!("out dx, al", in("dx") p, in("al") v); }
#[inline] unsafe fn inb(p: u16) -> u8 { let v: u8; asm!("in al, dx", in("dx") p, out("al") v); v }
unsafe fn put(b: u8) { let mut t = 100_000u32; while inb(COM1+5) & 0x20 == 0 { t = t.saturating_sub(1); if t == 0 { return; } } outb(COM1, b); }
fn puts(s: &str) { unsafe { for &b in s.as_bytes() { if b == b'\n' { put(b'\r'); } put(b); } } }
fn hex(mut v: u64) { unsafe { if v == 0 { put(b'0'); return; } let mut b = [0u8; 16]; let mut i = 0; while v > 0 { b[i] = b"0123456789abcdef"[(v & 0xF) as usize]; v >>= 4; i += 1; } for j in (0..i).rev() { put(b[j]); } } }
fn dec(mut v: usize) { unsafe { if v == 0 { put(b'0'); return; } let mut b = [0u8; 20]; let mut i = 0; while v > 0 { b[i] = b'0' + (v % 10) as u8; v /= 10; i += 1; } for j in (0..i).rev() { put(b[j]); } } }
fn init_serial() { unsafe { outb(COM1+1, 0); outb(COM1+3, 0x80); outb(COM1+0, 1); outb(COM1+1, 0); outb(COM1+3, 3); outb(COM1+2, 0xC7); outb(COM1+4, 0xB); } }

#[unsafe(no_mangle)]
pub extern "efiapi" fn efi_main(image_handle: EfiHandle, system_table: *mut core::ffi::c_void) -> EfiStatus {
    init_serial();
    puts("\n[uefi] BMO shim\n");

    // Locate simple filesystem
    let st = system_table as *const EfiSystemTable;
    let bs = unsafe { (*st).bs };
    let base = unsafe { &(*bs).hdr } as *const EfiTableHeader as *const *mut core::ffi::c_void;
    let locate: extern "efiapi" fn(*mut EfiGuid, *mut core::ffi::c_void, &mut EfiHandle) -> EfiStatus =
        unsafe { core::mem::transmute(*base.add(3 + 37)) };
    let mut fs_h: EfiHandle = core::ptr::null_mut();
    if unsafe { locate(&raw mut FS_GUID, core::ptr::null_mut(), &mut fs_h) } != EFI_SUCCESS {
        puts("[uefi] no FS\n");
        return 1;
    }

    // Open volume
    let sfsp = fs_h as *const *mut core::ffi::c_void;
    let open_vol: extern "efiapi" fn(EfiHandle, &mut *mut core::ffi::c_void) -> EfiStatus =
        unsafe { core::mem::transmute(*sfsp.add(1)) };
    let mut root: *mut core::ffi::c_void = core::ptr::null_mut();
    if unsafe { open_vol(fs_h, &mut root) } != EFI_SUCCESS { return 2; }

    // Open \EFI\BOOT\ring0\faggin\s1_cpu.bin
    let file_base = root as *const *mut core::ffi::c_void;
    let open_fn: extern "efiapi" fn(*mut core::ffi::c_void, &mut *mut core::ffi::c_void, *const u16, u64, u64) -> EfiStatus =
        unsafe { core::mem::transmute(*file_base.add(1)) };

    let path: [u16; 36] = [
        b'\\' as u16, b'E' as u16, b'F' as u16, b'I' as u16, b'\\' as u16, b'B' as u16, b'O' as u16, b'O' as u16, b'T' as u16,
        b'\\' as u16, b'r' as u16, b'i' as u16, b'n' as u16, b'g' as u16, b'0' as u16, b'\\' as u16, b'f' as u16, b'a' as u16,
        b'g' as u16, b'g' as u16, b'i' as u16, b'n' as u16, b'\\' as u16, b's' as u16, b'1' as u16, b'_' as u16, b'c' as u16,
        b'p' as u16, b'u' as u16, b'.' as u16, b'b' as u16, b'i' as u16, b'n' as u16, 0, 0, 0,
    ];
    let mut file: *mut core::ffi::c_void = core::ptr::null_mut();
    if unsafe { open_fn(root, &mut file, path.as_ptr(), 1, 0) } != EFI_SUCCESS {
        puts("[uefi] open s1_cpu.bin FAIL\n");
        return 3;
    }
    let opened_file = file as *const *mut core::ffi::c_void;
    let read_fn: extern "efiapi" fn(*mut core::ffi::c_void, &mut usize, *mut u8) -> EfiStatus =
        unsafe { core::mem::transmute(*opened_file.add(4)) };
    let mut size = FILE_BUF_SIZE;
    let file_buf = unsafe { &mut *core::ptr::addr_of_mut!(FILE_BUF) };
    if unsafe { read_fn(file, &mut size, file_buf.as_mut_ptr()) } != EFI_SUCCESS || size == 0 {
        puts("[uefi] read s1_cpu.bin FAIL\n");
        return 4;
    }
    if size as u64 > S1_SLOT { return 5; }
    puts("[uefi] s1_cpu.bin="); dec(size); puts(" bytes\n");

    // Allocate fixed address 0x100000
    let alloc_p: extern "efiapi" fn(u32, u32, usize, &mut u64) -> EfiStatus =
        unsafe { core::mem::transmute(*base.add(3 + 2)) };
    let mut alloc = S1_ADDR;
    if unsafe { alloc_p(2, 2, (S1_SLOT as usize + 0xFFF) / 0x1000, &mut alloc) } != EFI_SUCCESS { return 6; }

    // Copy + zero BSS
    let dst = S1_ADDR as *mut u8;
    for i in 0..size { unsafe { dst.add(i).write(file_buf[i]); } }
    for i in size as u64..S1_SLOT { unsafe { dst.add(i as usize).write(0); } }
    puts("[uefi] s1_cpu loaded at 0x"); hex(S1_ADDR); puts("\n");

    puts("[uefi] ===> JUMP s1_cpu 0x"); hex(S1_ADDR); puts("\n");
    unsafe { asm!("sfence", options(nostack, preserves_flags)); }

    // s1 still needs GOP, file and memory-map services to load s2 + kernel.
    // Keep Boot Services alive and use a typed EFI call so the compiler emits
    // the Microsoft x64 argument registers and stack alignment required by UEFI.
    let entry: extern "efiapi" fn(EfiHandle, *mut core::ffi::c_void) -> ! =
        unsafe { core::mem::transmute(S1_ADDR as usize) };
    entry(image_handle, system_table)
}

#[panic_handler]
fn panic(_info: &core::panic::PanicInfo) -> ! { loop { unsafe { asm!("hlt"); } } }
