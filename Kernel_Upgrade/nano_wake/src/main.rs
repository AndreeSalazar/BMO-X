#![no_std]
#![no_main]

use core::panic::PanicInfo;
use boot_context::{BootContext, MemoryEntry, MAGIC};

// ── UEFI types ──────────────────────────────────────────────

type EfiHandle = *mut core::ffi::c_void;
type EfiStatus = u64;

const EFI_SUCCESS: EfiStatus = 0;
const EFI_BUFFER_TOO_SMALL: EfiStatus = 5;
const EFI_NOT_FOUND: EfiStatus = 14;

const EFI_CONVENTIONAL_MEMORY: u32 = 7;

// Physical addresses for each stage
const STAGE1_ADDR: u64 = 0x100000;
const STAGE2_ADDR: u64 = 0x200000;
const STAGE3_ADDR: u64 = 0x300000;
const KERNEL_ADDR: u64 = 0x400000;

const MAX_FILE_SIZE: usize = 256 * 1024; // 256KB max per stage

// ── UEFI structs ────────────────────────────────────────────

#[repr(C)]
struct EfiTableHeader {
    signature: u64,
    revision: u32,
    header_size: u32,
    crc32: u32,
    _reserved: u32,
}

#[repr(C)]
struct EfiBootServices {
    hdr: EfiTableHeader,
    // function table: indices 0..43 from start of functions
    // Index  3+x: func[x]
    // GetMemoryMap    = func[4]  → hdr_u64[3+4]=7
    // ExitBootServices = func[26] → hdr_u64[3+26]=29
    // LocateProtocol  = func[37] → hdr_u64[3+37]=40
    _pad: [u8; 44 * 8],
}

#[repr(C)]
struct EfiSystemTable {
    hdr: EfiTableHeader,
    _firmware: *mut core::ffi::c_void,
    _cin_handle: EfiHandle,
    _con_in: *mut core::ffi::c_void,
    _cout_handle: EfiHandle,
    console_out: *mut EfiSimpleTextOutput,
    _cerr_handle: EfiHandle,
    _con_err: *mut core::ffi::c_void,
    _runtime: *mut core::ffi::c_void,
    boot_services: *mut EfiBootServices,
    _num_tables: usize,
    _config_tables: *mut core::ffi::c_void,
}

#[repr(C)]
struct EfiSimpleTextOutput {
    _pad: [u8; 64],
}

#[repr(C)]
struct EfiMemoryDescriptor {
    mem_type: u32,
    _pad: u32,
    phys_start: u64,
    virt_start: u64,
    num_pages: u64,
    attrib: u64,
}

#[repr(C)]
struct EfiGuid {
    data1: u32,
    data2: u16,
    data3: u16,
    data4: [u8; 8],
}

#[repr(C)]
struct EfiGraphicsOutputProtocolMode {
    max_mode: u32,
    mode: u32,
    info: *mut u8,
    size_of_info: usize,
    frame_buffer_base: u64,
    frame_buffer_size: usize,
}

#[repr(C)]
struct EfiGraphicsOutputProtocol {
    query_mode: extern "efiapi" fn(*mut Self, u32, &mut usize, &mut *mut u8) -> EfiStatus,
    set_mode: extern "efiapi" fn(*mut Self, u32) -> EfiStatus,
    blt: *mut core::ffi::c_void,
    mode: *mut EfiGraphicsOutputProtocolMode,
}

// ── GUIDs ───────────────────────────────────────────────────

static mut FILE_SYSTEM_GUID: EfiGuid = EfiGuid {
    data1: 0x964e5b22, data2: 0x6409, data3: 0x47ef,
    data4: [0x97, 0xa2, 0xff, 0x06, 0xff, 0x38, 0xb0, 0xdf],
};
static mut GOP_GUID: EfiGuid = EfiGuid {
    data1: 0x9042a9de, data2: 0x23dc, data3: 0x4a38,
    data4: [0x96, 0xfb, 0x72, 0xde, 0x52, 0xfe, 0xc4, 0x49],
};

// ── Boot services helpers ───────────────────────────────────

/// Access a function pointer from BootServices by its index
/// (0-based from start of function table, after Hdr).
unsafe fn bs_func(bs: *mut EfiBootServices, idx: usize) -> *mut core::ffi::c_void {
    let base = &(*bs).hdr as *const EfiTableHeader as *const *mut core::ffi::c_void;
    // Hdr occupies 3 slots (24 bytes)
    *base.add(3 + idx)
}

unsafe fn locate_protocol(
    bs: *mut EfiBootServices,
    guid: *mut EfiGuid,
    handle: &mut EfiHandle,
) -> EfiStatus {
    let fnptr: extern "efiapi" fn(*mut EfiGuid, *mut core::ffi::c_void, &mut EfiHandle) -> EfiStatus =
        core::mem::transmute(bs_func(bs, 37)); // LocateProtocol = func[37]
    fnptr(guid, core::ptr::null_mut(), handle)
}

unsafe fn get_memory_map(
    bs: *mut EfiBootServices,
    map_size: &mut usize,
    buf: *mut u8,
    map_key: &mut usize,
    desc_size: &mut usize,
    desc_ver: &mut u32,
) -> EfiStatus {
    let fnptr: extern "efiapi" fn(&mut usize, *mut u8, &mut usize, &mut usize, &mut u32) -> EfiStatus =
        core::mem::transmute(bs_func(bs, 4)); // GetMemoryMap = func[4]
    fnptr(map_size, buf, map_key, desc_size, desc_ver)
}

unsafe fn exit_boot_services(bs: *mut EfiBootServices, handle: EfiHandle, key: usize) -> EfiStatus {
    let fnptr: extern "efiapi" fn(EfiHandle, usize) -> EfiStatus =
        core::mem::transmute(bs_func(bs, 26)); // ExitBootServices = func[26]
    fnptr(handle, key)
}

// ── Serial / Console ────────────────────────────────────────

unsafe fn serial_init() {
    core::arch::asm!("out dx, al", in("dx") 0x3F8u16 + 1u16, in("al") 0u8);
    core::arch::asm!("out dx, al", in("dx") 0x3F8u16 + 3u16, in("al") 0x80u8);
    core::arch::asm!("out dx, al", in("dx") 0x3F8u16, in("al") 1u8);
    core::arch::asm!("out dx, al", in("dx") 0x3F8u16 + 1u16, in("al") 0u8);
    core::arch::asm!("out dx, al", in("dx") 0x3F8u16 + 3u16, in("al") 3u8);
    core::arch::asm!("out dx, al", in("dx") 0x3F8u16 + 2u16, in("al") 0xC7u8);
    core::arch::asm!("out dx, al", in("dx") 0x3F8u16 + 4u16, in("al") 0x0Bu8);
}

unsafe fn serial_byte(b: u8) {
    let mut timeout = 100_000u32;
    while {
        let mut s: u8;
        core::arch::asm!("in al, dx", out("al") s, in("dx") 0x3F8u16 + 5u16);
        s & 0x20 == 0
    } {
        timeout = timeout.saturating_sub(1);
        if timeout == 0 { return; }
    }
    core::arch::asm!("out dx, al", in("dx") 0x3F8u16, in("al") b);
}

fn serial_puts(s: &str) {
    for &b in s.as_bytes() {
        if b == b'\n' { unsafe { serial_byte(b'\r'); } }
        unsafe { serial_byte(b); }
    }
}

fn serial_hex(mut v: u64) {
    if v == 0 { unsafe { serial_byte(b'0'); } return; }
    let mut buf = [0u8; 16];
    let mut i = 0;
    while v > 0 {
        buf[i] = b"0123456789abcdef"[(v & 0xF) as usize];
        v >>= 4; i += 1;
    }
    for j in (0..i).rev() { unsafe { serial_byte(buf[j]); } }
}

fn serial_dec(mut v: usize) {
    if v == 0 { unsafe { serial_byte(b'0'); } return; }
    let mut buf = [0u8; 20];
    let mut i = 0;
    while v > 0 {
        buf[i] = b'0' + (v % 10) as u8;
        v /= 10; i += 1;
    }
    for j in (0..i).rev() { unsafe { serial_byte(buf[j]); } }
}

fn ucs2(s: &str, buf: &mut [u16; 260]) -> *const u16 {
    let mut idx = 0;
    for c in s.bytes() {
        if idx >= 258 { break; }
        buf[idx] = c as u16;
        idx += 1;
    }
    buf[idx] = 0;
    buf.as_ptr()
}

unsafe fn uefi_print(s: &str, con: *mut EfiSimpleTextOutput) {
    let mut buf = [0u16; 260];
    let ptr = ucs2(s, &mut buf);
    let f: extern "efiapi" fn(*mut EfiSimpleTextOutput, *const u16) -> EfiStatus =
        core::mem::transmute(*((con as *const *mut core::ffi::c_void).add(4)));
    f(con, ptr);
}

// ── File loading from ESP ───────────────────────────────────

/// Load a binary file from the ESP into a buffer.
unsafe fn load_file(
    bs: *mut EfiBootServices,
    image_handle: EfiHandle,
    filename: &str,
    buf: &mut [u8],
) -> Option<usize> {
    // 1. Locate Simple File System Protocol
    let mut fs_handle: EfiHandle = core::ptr::null_mut();
    if locate_protocol(bs, &mut FILE_SYSTEM_GUID, &mut fs_handle) != EFI_SUCCESS || fs_handle.is_null() {
        return None;
    }

    // 2. OpenVolume to get root directory
    // Simple File System Protocol: [0]=Revision, [1]=OpenVolume
    let sfsp = fs_handle as *const *mut core::ffi::c_void;
    let open_vol: extern "efiapi" fn(EfiHandle, &mut *mut core::ffi::c_void) -> EfiStatus =
        core::mem::transmute(*sfsp.add(1));
    let mut root: *mut core::ffi::c_void = core::ptr::null_mut();
    if open_vol(fs_handle, &mut root) != EFI_SUCCESS || root.is_null() {
        return None;
    }

    // 3. Build full path: \EFI\BOOT\filename
    let mut path = [0u16; 260];
    path[0] = b'\\' as u16;
    let prefix = "EFI\\BOOT\\";
    let mut idx = 1;
    for c in prefix.bytes() { path[idx] = c as u16; idx += 1; }
    for c in filename.bytes() { path[idx] = c as u16; idx += 1; }
    path[idx] = 0;

    // 4. Open the file (File Protocol: [0]=Revision, [1]=Open)
    let file_base = root as *const *mut core::ffi::c_void;
    let open_fn: extern "efiapi" fn(*mut core::ffi::c_void, &mut *mut core::ffi::c_void, *const u16, u64, u64, *mut core::ffi::c_void) -> EfiStatus =
        core::mem::transmute(*file_base.add(1));
    let mut file: *mut core::ffi::c_void = core::ptr::null_mut();
    if open_fn(root, &mut file, path.as_ptr(), 0, 0, core::ptr::null_mut()) != EFI_SUCCESS || file.is_null() {
        return None;
    }

    // 5. Read file (File Protocol: [0]=Revision, [1]=Open, [2]=Close, [3]=Delete, [4]=Read)
    let read_fn: extern "efiapi" fn(*mut core::ffi::c_void, &mut usize, *mut u8) -> EfiStatus =
        core::mem::transmute(*file_base.add(4));
    let mut size = buf.len();
    if read_fn(file, &mut size, buf.as_mut_ptr()) != EFI_SUCCESS {
        return None;
    }
    Some(size)
}

// ── GOP Framebuffer ─────────────────────────────────────────

unsafe fn get_framebuffer(bs: *mut EfiBootServices) -> (u64, u32, u32, u32, u32) {
    let mut gop_handle: EfiHandle = core::ptr::null_mut();
    if locate_protocol(bs, &mut GOP_GUID, &mut gop_handle) != EFI_SUCCESS || gop_handle.is_null() {
        return (0, 0, 0, 0, 0);
    }
    let gop = &*(gop_handle as *const EfiGraphicsOutputProtocol);
    let mode = &*gop.mode;
    let info = &*(mode.info as *const [u32; 8]);
    let fb = mode.frame_buffer_base;
    let w = info[0];
    let h = info[1];
    let fmt = info[2];
    let stride = (mode.frame_buffer_size as u32) / (h * 4).max(1);
    (fb, w, h, stride, fmt)
}

// ── Memory helpers ──────────────────────────────────────────

unsafe fn copy_to_phys(dst: u64, src: *const u8, len: usize) {
    let dst_ptr = dst as *mut u8;
    for i in 0..len {
        dst_ptr.add(i).write(src.add(i).read());
    }
}

// ── Entry point ─────────────────────────────────────────────

const MEM_BUF_SIZE: usize = 32768;

#[no_mangle]
pub extern "efiapi" fn efi_main(
    image_handle: EfiHandle,
    system_table: *mut EfiSystemTable,
) -> EfiStatus {
    unsafe { serial_init(); }

    let st = unsafe { &*system_table };
    let bs = st.boot_services;
    let con = st.console_out;
    let mut ctx = BootContext::new();
    ctx.magic = MAGIC;
    ctx.version = 2;

    serial_puts("\n[stage0] BMO Nano Wake\n");

    // ── Memory map ───────────────────────────────────────────
    let mut mem_buf = [0u8; MEM_BUF_SIZE];
    let mut map_key: usize = 0;
    let mut map_size = MEM_BUF_SIZE;
    let mut desc_size: usize = 0;
    let mut desc_ver: u32 = 0;

    unsafe {
        let _ = get_memory_map(bs, &mut map_size, mem_buf.as_mut_ptr(), &mut map_key, &mut desc_size, &mut desc_ver);
    }

    if desc_size > 0 {
        let mut entries = [MemoryEntry { base: 0, size: 0, kind: 0 }; 64];
        let mut ec = 0;
        let num = map_size / desc_size;
        for i in 0..num.min(64) {
            let desc = &*(mem_buf.as_ptr().add(i * desc_size) as *const EfiMemoryDescriptor);
            if desc.mem_type == EFI_CONVENTIONAL_MEMORY && desc.num_pages > 0 {
                entries[ec] = MemoryEntry { base: desc.phys_start, size: desc.num_pages * 4096, kind: 1 };
                ec += 1;
            }
        }
        ctx.set_memory_map(&entries[..ec]);
        serial_puts("[stage0] Memory map: "); serial_dec(ec); serial_puts(" entries\n");
    }

    // ── Framebuffer ─────────────────────────────────────────
    let (fb, w, h, stride, fmt) = unsafe { get_framebuffer(bs) };
    ctx.fb_addr = fb;
    ctx.fb_width = w;
    ctx.fb_height = h;
    ctx.fb_stride = stride;
    ctx.fb_pixel_format = fmt;
    if fb != 0 {
        serial_puts("[stage0] GOP framebuffer at 0x"); serial_hex(fb);
        serial_puts(" "); serial_dec(w as usize); serial_puts("x"); serial_dec(h as usize); serial_puts("\n");
    }

    // ── Load stages ─────────────────────────────────────────
    struct LoadEntry {
        name: &'static str,
        addr: u64,
    }
    let stages = [
        LoadEntry { name: "stage1.bin", addr: STAGE1_ADDR },
        LoadEntry { name: "stage2.bin", addr: STAGE2_ADDR },
        LoadEntry { name: "stage3.bin", addr: STAGE3_ADDR },
        LoadEntry { name: "kernel.bin", addr: KERNEL_ADDR },
    ];

    let mut file_buf = [0u8; MAX_FILE_SIZE];
    let mut ok = true;

    for (i, s) in stages.iter().enumerate() {
        serial_puts("[stage0] Load "); serial_puts(s.name);
        serial_puts(" -> 0x"); serial_hex(s.addr); serial_puts(" ... ");

        match unsafe { load_file(bs, image_handle, s.name, &mut file_buf) } {
            Some(sz) if sz > 0 => {
                unsafe { copy_to_phys(s.addr, file_buf.as_ptr(), sz.min(MAX_FILE_SIZE)); }
                ctx.stage_base[i] = s.addr;
                ctx.stage_size[i] = sz as u64;
                ctx.stage_entry[i] = s.addr;
                serial_dec(sz); serial_puts(" bytes -> entry 0x"); serial_hex(s.addr); serial_puts("\n");
            }
            _ => {
                serial_puts("FAILED\n");
                unsafe { uefi_print("[stage0] ERROR: ", con); }
                unsafe { uefi_print(s.name, con); }
                unsafe { uefi_print(" not found on ESP\n", con); }
                ok = false;
            }
        }
    }

    if !ok {
        serial_puts("[stage0] Halting: missing stage files\n");
        loop { unsafe { core::arch::asm!("hlt"); } }
    }

    // ── Exit boot services ──────────────────────────────────
    serial_puts("[stage0] ExitBootServices ... ");
    // Re-fetch memory map for a fresh map key
    let mut map_key2: usize = 0;
    let mut map_sz = MEM_BUF_SIZE;
    let mut desc_sz: usize = 0;
    let mut desc_v: u32 = 0;
    unsafe {
        let _ = get_memory_map(bs, &mut map_sz, mem_buf.as_mut_ptr(), &mut map_key2, &mut desc_sz, &mut desc_v);
        let r = exit_boot_services(bs, image_handle, map_key2);
        if r == EFI_SUCCESS {
            serial_puts("OK\n");
        } else {
            serial_puts("status="); serial_hex(r); serial_puts("\n");
        }
    }

    // ── Jump to stage 1 ─────────────────────────────────────
    let entry = ctx.stage_entry[0];
    serial_puts("[stage0] Jump to 0x"); serial_hex(entry); serial_puts("\n");

    if entry != 0 {
        let f: extern "C" fn(*mut BootContext) -> ! = core::mem::transmute(entry);
        f(&mut ctx as *mut BootContext);
    }

    loop { unsafe { core::arch::asm!("hlt"); } }
}

#[panic_handler]
fn panic(_info: &PanicInfo) -> ! {
    loop { unsafe { core::arch::asm!("hlt"); } }
}
