//! **THE FIRMWARE SIDE** -- UEFI types, protocol GUIDs, and the four stages.
//!
//! === Why the types and the stages are one file ===
//!
//! Because they are one subject that happened not to be adjacent: the structs
//! at the top of the old file described exactly what the code 800 lines below
//! called. Reading either half alone told you nothing.
//!
//! === What this stage takes from the firmware, and then stops asking ===
//!
//! The memory map, the framebuffer through GOP, the payload, and then
//! `ExitBootServices`. After that call there is no firmware left to ask -- every
//! service, every allocation and every console goes away at once, so anything
//! not collected before it is gone for the rest of the machine's life.
//!
//! [!] The two GUIDs here were once WRONG (a corrupted `data4`), and the project
//! spent months believing the board had no GOP. A GUID is a 16-byte password:
//! it is exact or the universe answers "does not exist".

#[allow(unused_imports)]
use crate::*;

// ===================================================================
//  UEFI TYPES AND CONSTANTS
// ===================================================================

pub type EfiHandle = *mut core::ffi::c_void;
pub type EfiStatus = u64;
pub const EFI_SUCCESS: u64 = 0;
pub const EFI_CONVENTIONAL_MEMORY: u32 = 7;
pub const S2_ADDR: u64 = 0x200000;
pub const S2_RESERVE_SIZE: u64 = 2 * 1024 * 1024;
pub const KERNEL_RESERVE_SIZE: u64 = 16 * 1024 * 1024;
// Ring 3 init payload + kernel-owned workspace. Placed just past the
// kernel reserve (0x400000 + 16 MiB), inside the s2 identity map, and
// taken out of the UEFI map via AllocateAddress so no allocator can
// hand them out twice.
pub const RING3_PAYLOAD_ADDR: u64 = 0x1400000;
pub const RING3_PAYLOAD_MAX: u64 = 1024 * 1024;
pub const RING3_WORKSPACE_ADDR: u64 = 0x1500000;
pub const RING3_WORKSPACE_SIZE: u64 = 1024 * 1024;
pub const COM1: u16 = 0x3F8;

#[repr(C)] pub struct EfiTableHeader { signature: u64, revision: u32, header_size: u32, crc32: u32, _reserved: u32 }
#[repr(C)] pub struct EfiBootServices { hdr: EfiTableHeader, _pad: [u8; 44 * 8] }
#[repr(C)] pub struct EfiSystemTable {
    hdr: EfiTableHeader, _firmware: *mut core::ffi::c_void,
    _firmware_revision: u32, _firmware_pad: u32,
    _cin_handle: EfiHandle, _con_in: *mut core::ffi::c_void,
    _cout_handle: EfiHandle, _con_out: *mut core::ffi::c_void,
    _cerr_handle: EfiHandle, _con_err: *mut core::ffi::c_void,
    _runtime: *mut core::ffi::c_void,
    boot_services: *mut EfiBootServices, _num_tables: usize, _config_tables: *mut core::ffi::c_void,
}
#[repr(C)] pub struct EfiGuid { data1: u32, data2: u16, data3: u16, data4: [u8; 8] }
#[repr(C)] pub struct EfiSimpleFileSystemProtocol { revision: u64, open_volume: unsafe extern "efiapi" fn(*const Self, *mut *mut core::ffi::c_void) -> EfiStatus }
#[repr(C)] pub struct EfiFileProtocol {
    revision: u64,
    open: unsafe extern "efiapi" fn(*const Self, *mut *mut core::ffi::c_void, *const u16, u64, u64) -> EfiStatus,
    close: unsafe extern "efiapi" fn(*const Self) -> EfiStatus,
    _delete: *mut core::ffi::c_void, read: *mut core::ffi::c_void, write: *mut core::ffi::c_void,
    _get_position: *mut core::ffi::c_void, _set_position: *mut core::ffi::c_void,
    _get_info: *mut core::ffi::c_void, _set_info: *mut core::ffi::c_void, _flush: *mut core::ffi::c_void,
}
#[repr(C)] pub struct EfiMemoryDescriptor { mem_type: u32, _pad: u32, phys_start: u64, virt_start: u64, num_pages: u64, attrib: u64 }
#[repr(C)] pub struct EfiGraphicsOutputProtocolMode { max_mode: u32, mode: u32, info: *mut u8, size_of_info: usize, frame_buffer_base: u64, frame_buffer_size: usize }
#[repr(C)] pub struct EfiGraphicsOutputProtocol {
    query_mode: extern "efiapi" fn(*mut Self, u32, &mut usize, &mut *mut u8) -> EfiStatus,
    set_mode: extern "efiapi" fn(*mut Self, u32) -> EfiStatus,
    blt: *mut core::ffi::c_void, mode: *mut EfiGraphicsOutputProtocolMode,
}

// Standard UEFI protocol GUIDs. The two below were previously wrong,
// which is why LocateProtocol for the filesystem and the framebuffer
// always failed on real firmware (the project ran on serial output, so
// nobody noticed the framebuffer never came up).
//   EFI_SIMPLE_FILE_SYSTEM_PROTOCOL_GUID = 964e5b22-6409-11d2-8e39-00a0c969723b
//   EFI_GRAPHICS_OUTPUT_PROTOCOL_GUID    = 9042a9de-23dc-4a38-96fb-7aded080516a
pub static mut FILE_SYSTEM_GUID: EfiGuid = EfiGuid { data1: 0x964e5b22, data2: 0x6409, data3: 0x11d2, data4: [0x8e, 0x39, 0x00, 0xa0, 0xc9, 0x69, 0x72, 0x3b] };
pub static mut LOADED_IMAGE_GUID: EfiGuid = EfiGuid { data1: 0x5b1b31a1, data2: 0x9562, data3: 0x11d2, data4: [0x8e, 0x3f, 0x00, 0xa0, 0xc9, 0x69, 0x72, 0x3b] };
pub static mut GOP_GUID: EfiGuid = EfiGuid { data1: 0x9042a9de, data2: 0x23dc, data3: 0x4a38, data4: [0x96, 0xfb, 0x7a, 0xde, 0xd0, 0x80, 0x51, 0x6a] };


// ===================================================================
//  UEFI STAGES (memory map, GOP, load, ExitBootServices)
// ===================================================================

pub unsafe fn get_memory_map(bs: *mut EfiBootServices, buf: &mut [u8; 32768]) -> (usize, usize, usize, u32) {
    let mut map_size = buf.len();
    let mut map_key: usize = 0;
    let mut desc_size: usize = 0;
    let mut desc_ver: u32 = 0;
    let base = &(*bs).hdr as *const EfiTableHeader as *const *mut core::ffi::c_void;
    let fnptr: extern "efiapi" fn(*mut usize, *mut u8, *mut usize, *mut usize, *mut u32) -> u64 =
        core::mem::transmute(*base.add(3 + 4));
    let r = fnptr(&mut map_size, buf.as_mut_ptr(), &mut map_key, &mut desc_size, &mut desc_ver);
    if r != EFI_SUCCESS && r != 5 { return (0, 0, 0, 0); }
    (map_size, map_key, desc_size, desc_ver)
}

pub unsafe fn fill_memory_map(ctx: &mut BootContext, buf: &mut [u8; 32768], system_table: *mut EfiSystemTable) -> usize {
    let bs = (*system_table).boot_services;
    let (map_size, _key, desc_size, _ver) = get_memory_map(bs, buf);
    if map_size == 0 || desc_size == 0 { return 0; }
    let num = map_size / desc_size;
    let mut entries = [MemoryEntry { base: 0, size: 0, kind: 0 }; MAX_MEMORY_ENTRIES];
    let mut ec: usize = 0;
    for i in 0..num.min(MAX_MEMORY_ENTRIES) {
        let desc = &*(buf.as_ptr().add(i * desc_size) as *const EfiMemoryDescriptor);
        if desc.mem_type == EFI_CONVENTIONAL_MEMORY && desc.num_pages > 0 && ec < MAX_MEMORY_ENTRIES {
            entries[ec] = MemoryEntry { base: desc.phys_start, size: desc.num_pages * 4096, kind: 1 };
            ec += 1;
        }
    }
    ctx.set_memory_map(&entries[..ec]);
    ctx.memory_map_count = ec as u32;
    ec
}

/// Boot-progress marker on the firmware console (visible without serial).
/// Only valid while Boot Services are alive.
pub unsafe fn con_mark(system_table: *mut EfiSystemTable, s: &str) {
    let con = (*system_table)._con_out as *const *mut core::ffi::c_void;
    if con.is_null() { return; }
    let out: extern "efiapi" fn(*mut core::ffi::c_void, *const u16) -> EfiStatus =
        core::mem::transmute(*con.add(1));
    let mut buf = [0u16; 64];
    let mut i = 0;
    for &b in s.as_bytes() {
        if i >= buf.len() - 2 { break; }
        if b == b'\n' { buf[i] = 13; i += 1; }
        buf[i] = b as u16; i += 1;
    }
    buf[i] = 0;
    out((*system_table)._con_out as *mut core::ffi::c_void, buf.as_ptr());
}

/// Extract a valid framebuffer from one GOP handle into `ctx`.
pub unsafe fn try_gop_handle(ctx: &mut BootContext, gop_handle: EfiHandle) -> bool {
    if gop_handle.is_null() { return false; }
    let gop = &*(gop_handle as *const EfiGraphicsOutputProtocol);
    if gop.mode.is_null() { return false; }
    let mode = &*gop.mode;
    if mode.info.is_null() { return false; }
    let info = &*(mode.info as *const [u32; 9]);
    let w = info[1]; let h = info[2]; let fmt = info[3]; let stride = info[8];
    if w == 0 || h == 0 || stride < w || fmt > 1 || mode.frame_buffer_base == 0 { return false; }
    ctx.fb_addr = mode.frame_buffer_base;
    ctx.fb_width = w; ctx.fb_height = h;
    ctx.fb_stride = stride; ctx.fb_pixel_format = fmt;
    true
}

/// Acquire the framebuffer, robustly. Some firmwares (MSI A320M AMI fast
/// path) render text through their own console yet never publish a GOP
/// protocol via LocateProtocol. We escalate: LocateProtocol -> enumerate
/// every GOP handle -> ConnectController connect-all then re-enumerate.
pub unsafe fn fill_gop(ctx: &mut BootContext, system_table: *mut EfiSystemTable) -> bool {
    let bs = (*system_table).boot_services;
    let base = &(*bs).hdr as *const EfiTableHeader as *const *mut core::ffi::c_void;
    let locate_protocol: extern "efiapi" fn(*mut EfiGuid, *mut core::ffi::c_void, &mut EfiHandle) -> EfiStatus =
        core::mem::transmute(*base.add(3 + 37));
    let locate_handle: extern "efiapi" fn(u32, *mut EfiGuid, *mut core::ffi::c_void, &mut usize, *mut EfiHandle) -> EfiStatus =
        core::mem::transmute(*base.add(3 + 19));
    let handle_protocol: extern "efiapi" fn(EfiHandle, *mut EfiGuid, &mut *mut core::ffi::c_void) -> EfiStatus =
        core::mem::transmute(*base.add(3 + 16));
    let connect_controller: extern "efiapi" fn(EfiHandle, *mut EfiHandle, *mut core::ffi::c_void, u8) -> EfiStatus =
        core::mem::transmute(*base.add(3 + 30));

    // 1) LocateProtocol (fast path, works on normal firmwares).
    let mut gop_handle: EfiHandle = core::ptr::null_mut();
    if locate_protocol(&raw mut GOP_GUID, core::ptr::null_mut(), &mut gop_handle) == EFI_SUCCESS
        && try_gop_handle(ctx, gop_handle)
    {
        con_mark(system_table, "gopLP ");
        return finish_gop(ctx);
    }

    // 2) Enumerate every handle that carries GOP (harmless, read-only).
    //    We deliberately do NOT ConnectController connect-all here: it
    //    arms device drivers (timers/IRQs) that would trigger a fault
    //    during the later GDT swap, and on this firmware it did not
    //    surface a GOP handle anyway.
    let _ = &connect_controller; // reserved; not used on this path
    let mut nbytes = ALL_GOP.len() * core::mem::size_of::<EfiHandle>();
    let all = &mut *core::ptr::addr_of_mut!(ALL_GOP);
    // 2 = ByProtocol
    if locate_handle(2, &raw mut GOP_GUID, core::ptr::null_mut(), &mut nbytes, all.as_mut_ptr()) == EFI_SUCCESS {
        let n = (nbytes / core::mem::size_of::<EfiHandle>()).min(all.len());
        for i in 0..n {
            let mut iface: *mut core::ffi::c_void = core::ptr::null_mut();
            if handle_protocol(all[i], &raw mut GOP_GUID, &mut iface) == EFI_SUCCESS
                && try_gop_handle(ctx, iface as EfiHandle)
            {
                con_mark(system_table, "gopEN ");
                return finish_gop(ctx);
            }
        }
    }
    false
}

/// Clear the framebuffer to the BMO dark background once acquired.
pub unsafe fn finish_gop(ctx: &mut BootContext) -> bool {
    ser_print!("[s1_cpu] GOP fb=0x"); ser_hex!(ctx.fb_addr);
    ser_print!(" "); ser_dec!(ctx.fb_width as usize); ser_print!("x"); ser_dec!(ctx.fb_height as usize); ser_print!("\n");
    let fb = ctx.fb_addr as *mut u32;
    let total = (ctx.fb_stride as usize) * (ctx.fb_height as usize);
    for i in 0..total { fb.add(i).write_volatile(0xFF0A_0F1Du32); }
    asm!("mfence", options(nostack, preserves_flags));
    true
}

pub unsafe fn load_from_esp(ctx: &mut BootContext, system_table: *mut EfiSystemTable, image_handle: EfiHandle) -> bool {
    let bs = (*system_table).boot_services;
    let base = &(*bs).hdr as *const EfiTableHeader as *const *mut core::ffi::c_void;
    // Volume resolution, strongest form (mirrors the UEFI shim): probe the
    // LoadedImage device first, then EVERY SimpleFS handle, and claim the
    // first volume that actually contains the chain's s2_mem.bin. Immune
    // to firmwares that enumerate several disks/ESPs.
    let handle_protocol: extern "efiapi" fn(EfiHandle, *mut EfiGuid, &mut *mut core::ffi::c_void) -> EfiStatus =
        core::mem::transmute(*base.add(3 + 16));
    let locate_handle: extern "efiapi" fn(u32, *mut EfiGuid, *mut core::ffi::c_void, &mut usize, *mut EfiHandle) -> EfiStatus =
        core::mem::transmute(*base.add(3 + 19));

    const MAX_FS: usize = 32;
    let mut candidates: [EfiHandle; MAX_FS + 1] = [core::ptr::null_mut(); MAX_FS + 1];
    let mut count: usize = 0;
    let mut li: *mut core::ffi::c_void = core::ptr::null_mut();
    if handle_protocol(image_handle, &raw mut LOADED_IMAGE_GUID, &mut li) == EFI_SUCCESS && !li.is_null() {
        // EFI_LOADED_IMAGE_PROTOCOL: DeviceHandle at byte offset 24.
        let device = *(li as *const EfiHandle).add(3);
        if !device.is_null() { candidates[count] = device; count += 1; }
    }
    let first_fs = count;
    let mut bytes = MAX_FS * core::mem::size_of::<EfiHandle>();
    // 2 = ByProtocol
    if locate_handle(2, &raw mut FILE_SYSTEM_GUID, core::ptr::null_mut(), &mut bytes, candidates.as_mut_ptr().add(first_fs)) == EFI_SUCCESS {
        count += bytes / core::mem::size_of::<EfiHandle>();
        if count > candidates.len() { count = candidates.len(); }
    }
    ser_print!("[s1_cpu] FS candidates="); ser_dec!(count); ser_print!("\n");

    // Probe marker: the stage this loader must find next
    // ("\EFI\BOOT\ring0\faggin\s2_mem.bin").
    let mut probe_full = [0u16; 36];
    for (i, &c) in b"\\EFI\\BOOT\\ring0\\faggin\\s2_mem.bin".iter().enumerate() {
        probe_full[i] = c as u16;
    }
    let mut root: *mut core::ffi::c_void = core::ptr::null_mut();
    for c in 0..count {
        let mut fs_if: *mut core::ffi::c_void = core::ptr::null_mut();
        if handle_protocol(candidates[c], &raw mut FILE_SYSTEM_GUID, &mut fs_if) != EFI_SUCCESS || fs_if.is_null() { continue; }
        let sfsp = fs_if as *const *mut core::ffi::c_void;
        let open_vol: extern "efiapi" fn(*mut core::ffi::c_void, &mut *mut core::ffi::c_void) -> EfiStatus =
            core::mem::transmute(*sfsp.add(1));
        let mut vol_root: *mut core::ffi::c_void = core::ptr::null_mut();
        if open_vol(fs_if, &mut vol_root) != EFI_SUCCESS || vol_root.is_null() { continue; }
        let fb = vol_root as *const *mut core::ffi::c_void;
        let ofn: extern "efiapi" fn(*mut core::ffi::c_void, &mut *mut core::ffi::c_void, *const u16, u64, u64) -> EfiStatus =
            core::mem::transmute(*fb.add(1));
        let mut f: *mut core::ffi::c_void = core::ptr::null_mut();
        if ofn(vol_root, &mut f, probe_full.as_ptr(), 1, 0) == EFI_SUCCESS && !f.is_null() {
            ser_print!("[s1_cpu] chain volume = candidate "); ser_dec!(c); ser_print!("\n");
            root = vol_root;
            break;
        }
    }
    if root.is_null() { return false; }
    let file_base = root as *const *mut core::ffi::c_void;
    let open_fn: extern "efiapi" fn(*mut core::ffi::c_void, &mut *mut core::ffi::c_void, *const u16, u64, u64) -> EfiStatus =
        core::mem::transmute(*file_base.add(1));
    let alloc_pages: extern "efiapi" fn(u32, u32, usize, &mut u64) -> EfiStatus =
        core::mem::transmute(*base.add(3 + 2));

    let stages: [(&str, u64); MAX_STAGES] = [
        ("", 0), ("s2_mem.bin", 0x200000),
        ("", 0), ("", 0), ("", 0), ("", 0), ("", 0),
        ("", 0), ("", 0), ("", 0), ("", 0), ("", 0),
        ("kernel.bin", 0x400000),
    ];
    // s1 is already loaded and reserved by the UEFI shim.
    ctx.stage_base[0] = 0x100000;
    ctx.stage_entry[0] = 0x100000;
    let mut ok = true;
    for (i, &(name, addr)) in stages.iter().enumerate() {
        if name.is_empty() { continue; }
        let mut path = [0u16; 260];
        path[0] = b'\\' as u16;
        let prefix: &[u8] = if i == KERNEL_STAGE_INDEX { b"EFI\\BOOT\\ring0\\" } else { b"EFI\\BOOT\\ring0\\faggin\\" };
        let mut idx = 1;
        for &c in prefix { path[idx] = c as u16; idx += 1; }
        for &c in name.as_bytes() { path[idx] = c as u16; idx += 1; }
        path[idx] = 0;
        let mut file: *mut core::ffi::c_void = core::ptr::null_mut();
        if open_fn(root, &mut file, path.as_ptr(), 1, 0) != EFI_SUCCESS { ok = false; continue; }
        let opened_file = file as *const *mut core::ffi::c_void;
        let read_fn: extern "efiapi" fn(*mut core::ffi::c_void, &mut usize, *mut u8) -> EfiStatus =
            core::mem::transmute(*opened_file.add(4));
        let reserve_size = if i + 1 == MAX_STAGES { KERNEL_RESERVE_SIZE } else { S2_RESERVE_SIZE };
        let mut allocation = addr;
        let pages = (reserve_size as usize + 4095) / 4096;
        if alloc_pages(2, 2, pages, &mut allocation) != EFI_SUCCESS { ok = false; continue; }
        let dst = addr as *mut u8;
        let mut size = reserve_size as usize;
        if read_fn(file, &mut size, dst) != EFI_SUCCESS || size == 0 { ok = false; continue; }
        let bss_end = addr + reserve_size;
        for j in size as u64..(bss_end - addr) { dst.add(j as usize).write(0); }
        ctx.stage_base[i] = addr;
        ctx.stage_size[i] = size as u64;
        ctx.stage_entry[i] = addr;
        ser_print!("[s1_cpu] loaded "); ser_print!(name);
        ser_print!(" -> 0x"); ser_hex!(addr);
        ser_print!(" ("); ser_dec!(size); ser_print!(" bytes)\n");
    }
    ok
}

pub unsafe fn exit_boot_services_and_jump(ctx_ptr: *mut BootContext, system_table: *mut EfiSystemTable, image_handle: EfiHandle, entry: u64) -> ! {
    let bs = (*system_table).boot_services;
    let base = &(*bs).hdr as *const EfiTableHeader as *const *mut core::ffi::c_void;
    let get_mm: extern "efiapi" fn(*mut usize, *mut u8, *mut usize, *mut usize, *mut u32) -> EfiStatus =
        core::mem::transmute(*base.add(3 + 4));
    let exit_bs: extern "efiapi" fn(EfiHandle, usize) -> EfiStatus =
        core::mem::transmute(*base.add(3 + 26));
    let mut buf = [0u8; 32768];
    let mut map_size = buf.len();
    let mut map_key: usize = 0;
    let mut desc_size: usize = 0;
    let mut desc_ver: u32 = 0;
    if get_mm(&mut map_size, buf.as_mut_ptr(), &mut map_key, &mut desc_size, &mut desc_ver) != EFI_SUCCESS { loop { asm!("hlt"); } }

    // Publish the final map, after reserving s1/s2/kernel.  The earlier map
    // still marked those physical ranges as conventional and would let the
    // kernel allocator overwrite its own boot images.
    let ctx = &mut *ctx_ptr;
    let mut entries = [MemoryEntry { base: 0, size: 0, kind: 0 }; MAX_MEMORY_ENTRIES];
    let mut count = 0;
    if desc_size != 0 {
        for i in 0..(map_size / desc_size) {
            let desc = &*(buf.as_ptr().add(i * desc_size) as *const EfiMemoryDescriptor);
            if desc.mem_type == EFI_CONVENTIONAL_MEMORY && desc.num_pages > 0 && count < MAX_MEMORY_ENTRIES {
                entries[count] = MemoryEntry { base: desc.phys_start, size: desc.num_pages * 4096, kind: 1 };
                count += 1;
            }
        }
    }
    ctx.set_memory_map(&entries[..count]);
    ctx.memory_map_count = count as u32;

    if exit_bs(image_handle, map_key) != EFI_SUCCESS { loop { asm!("hlt"); } }

    // Firmware is dead: 0x400000..+16 MiB is ours now. Place the kernel
    // that the unified shim kept inside its own image (AllocateAddress on
    // this range fails on firmwares that park Boot Services data there --
    // observed on MSI A320M; post-EBS the range is conventional memory).
    let ksrc = PRELOAD_KERNEL_SRC;
    let ksize = PRELOAD_KERNEL_SIZE as usize;
    if ksrc != 0 && ksize != 0 {
        let dst = 0x400000 as *mut u8;
        core::ptr::copy_nonoverlapping(ksrc as *const u8, dst, ksize);
        core::ptr::write_bytes(dst.add(ksize), 0, KERNEL_RESERVE_SIZE as usize - ksize);
        ser_print!("[s1_cpu] kernel placed at 0x400000 (");
        ser_dec!(ksize);
        ser_print!(" bytes, post-EBS)\n");
    }

    // Visual bisect marker (no serial needed): GREEN bar rows 0..8 =
    // "s1 survived ExitBootServices + kernel placement, jumping to s2".
    // s2 paints CYAN below it, the kernel MAGENTA below that.
    if ctx.fb_addr != 0 {
        let fb = ctx.fb_addr as *mut u32;
        for y in 0..8u64 {
            for x in 0..ctx.fb_width as u64 {
                fb.add((y * ctx.fb_stride as u64 + x) as usize).write_volatile(0xFF00_CC44);
            }
        }
    }

    ser_print!("[s1_cpu] ===> JUMP s2_mem 0x");
    ser_hex!(entry);
    ser_print!("\n");
    asm!("sfence", options(nostack, preserves_flags));
    asm!(
        "mov rdi, {ctx}",
        "xor rbp, rbp",
        "jmp {entry}",
        ctx = in(reg) ctx_ptr,
        entry = in(reg) entry,
        options(noreturn)
    );
}

//  BOOTCONTEXT (statically allocated in .bss)
