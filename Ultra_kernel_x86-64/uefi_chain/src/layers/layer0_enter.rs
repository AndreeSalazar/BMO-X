//! Layer 0 — `uefi_enter`
//!
//! Responsibilities (only these, nothing else):
//! 1. Receive the UEFI handoff (`ImageHandle`, `SystemTable*`).
//! 2. Bring up COM1 serial for the rest of the chain.
//! 3. Build the `BootContext` skeleton, stamp `MAGIC` and `version`.
//! 4. Dump previous boot logs from NVRAM to crash.log on ESP.
//! 5. Initialize NVRAM logging for this boot.
//! 6. Jump to layer 1 (`uefi_efi_getmem`).
//!
//! This layer MUST NOT touch memory map, GOP, ESP, or boot services
//! directly. That's the next layer's job.

#![allow(dead_code)]

use boot_context::BootContext;

type EfiHandle = *mut core::ffi::c_void;
type EfiStatus = u64;

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
    _pad: [u8; 44 * 8],
}

#[repr(C)]
struct EfiSystemTable {
    hdr: EfiTableHeader,
    _firmware: *mut core::ffi::c_void,
    _cin_handle: EfiHandle,
    _con_in: *mut core::ffi::c_void,
    _cout_handle: EfiHandle,
    _con_out: *mut core::ffi::c_void,
    _cerr_handle: EfiHandle,
    _con_err: *mut core::ffi::c_void,
    _runtime: *mut core::ffi::c_void,
    boot_services: *mut EfiBootServices,
    _num_tables: usize,
    _config_tables: *mut core::ffi::c_void,
}

#[repr(C)]
struct EfiGuid {
    data1: u32,
    data2: u16,
    data3: u16,
    data4: [u8; 8],
}

const EFI_SIMPLE_FILE_SYSTEM_PROTOCOL_GUID: EfiGuid = EfiGuid {
    data1: 0x964e5b22,
    data2: 0x6409,
    data3: 0x47ef,
    data4: [0x97, 0xa2, 0xff, 0x06, 0xff, 0x38, 0xb0, 0xdf],
};

#[repr(C)]
struct EfiSimpleFileSystemProtocol {
    revision: u64,
    open_volume: unsafe extern "efiapi" fn(
        this: *const EfiSimpleFileSystemProtocol,
        root: *mut *mut core::ffi::c_void,
    ) -> EfiStatus,
}

#[repr(C)]
struct EfiFileProtocol {
    revision: u64,
    open: unsafe extern "efiapi" fn(
        this: *const EfiFileProtocol,
        new_handle: *mut *mut core::ffi::c_void,
        filename: *const u16,
        open_mode: u64,
        attributes: u64,
    ) -> EfiStatus,
    close: unsafe extern "efiapi" fn(this: *const EfiFileProtocol) -> EfiStatus,
    _delete: *mut core::ffi::c_void,
    read: *mut core::ffi::c_void,
    write: unsafe extern "efiapi" fn(
        this: *const EfiFileProtocol,
        buffer_size: *mut usize,
        buffer: *const u8,
    ) -> EfiStatus,
    _get_position: *mut core::ffi::c_void,
    _set_position: *mut core::ffi::c_void,
    _get_info: *mut core::ffi::c_void,
    _set_info: *mut core::ffi::c_void,
    _flush: *mut core::ffi::c_void,
}

extern "C" {
    /// Layer 1 entry point. Resolved at link time within the same EFI
    /// binary.
    fn l1_entry(ctx: *mut BootContext, ih: EfiHandle, st: *mut core::ffi::c_void) -> !;
}

#[no_mangle]
pub extern "efiapi" fn layer0_efi_main(
    image_handle: EfiHandle,
    system_table: *mut core::ffi::c_void,
) -> EfiStatus {
    crate::serial::init();
    crate::serial::puts("\n[L0 uefi_enter] BMO Ultra Kernel\n");

    // Initialize NVRAM logging for this boot
    nvram_log::init(system_table as u64);
    nvram_log::write_boot_stage("layer0_enter");

    // Dump previous boot logs to crash.log on ESP
    // This helps debug boot failures without serial cable
    dump_previous_logs_to_crash_log(image_handle, system_table);

    let mut ctx = BootContext::new();
    ctx.magic = boot_context::MAGIC;
    ctx.version = 2;

    crate::serial::puts("[L0] magic=");
    crate::serial::hex(ctx.magic);
    crate::serial::puts(" version=");
    crate::serial::dec(ctx.version as usize);
    crate::serial::puts("\n");

    crate::serial::puts("[L0] jump -> layer1_getmem\n");
    nvram_log::log("jumping to layer1_getmem");

    unsafe { l1_entry(&mut ctx, image_handle, system_table) }
}

/// Read logs from previous boot (stored in NVRAM) and write them to
/// `\EFI\BOOT\crash.log` on the ESP. This allows debugging boot failures
/// without a serial cable - just reboot to Windows and read the file.
fn dump_previous_logs_to_crash_log(
    image_handle: EfiHandle,
    system_table: *mut core::ffi::c_void,
) {
    // Try to read logs from NVRAM
    let log_entries = nvram_log::read_log();
    
    if log_entries.is_empty() {
        crate::serial::puts("[L0] no previous logs in NVRAM\n");
        return;
    }

    // Concatenate all log entries into a single buffer
    let mut log_buf = [0u8; 4096];
    let mut pos = 0;
    for entry in log_entries {
        let bytes = entry.as_bytes();
        let copy_len = bytes.len().min(log_buf.len() - pos);
        log_buf[pos..pos + copy_len].copy_from_slice(&bytes[..copy_len]);
        pos += copy_len;
        if pos >= log_buf.len() { break; }
    }

    crate::serial::puts("[L0] dumping ");
    crate::serial::dec(pos);
    crate::serial::puts(" bytes from NVRAM to crash.log\n");

    // Open the ESP filesystem
    let st = unsafe { &*(system_table as *const EfiSystemTable) };
    let bs = st.boot_services;

    let mut fs_handle: EfiHandle = core::ptr::null_mut();

    let status = unsafe {
        let locate = core::mem::transmute::<_, unsafe extern "efiapi" fn(
            *const EfiGuid, *mut EfiHandle
        ) -> EfiStatus>(
            *((bs as *const u8).add(3 + 37) as *const usize)
        );
        locate(&EFI_SIMPLE_FILE_SYSTEM_PROTOCOL_GUID, &mut fs_handle)
    };

    if status != 0 {
        crate::serial::puts("[L0] cannot locate filesystem protocol\n");
        return;
    }

    // Open volume
    let sfsp = unsafe { &*(fs_handle as *const EfiSimpleFileSystemProtocol) };
    let mut root: *mut core::ffi::c_void = core::ptr::null_mut();

    let status = unsafe { (sfsp.open_volume)(sfsp, &mut root) };
    if status != 0 {
        crate::serial::puts("[L0] cannot open volume\n");
        return;
    }

    // Open or create crash.log
    let file_proto = unsafe { &*(root as *const EfiFileProtocol) };
    let mut file: *mut core::ffi::c_void = core::ptr::null_mut();
    let filename = [
        'c' as u16, 'r' as u16, 'a' as u16, 's' as u16, 'h' as u16,
        '.' as u16, 'l' as u16, 'o' as u16, 'g' as u16, 0
    ];

    let status = unsafe {
        (file_proto.open)(
            file_proto,
            &mut file,
            filename.as_ptr(),
            0x8000000000000003, // EFI_FILE_MODE_READ | EFI_FILE_MODE_WRITE | EFI_FILE_MODE_CREATE
            0,
        )
    };

    if status != 0 {
        crate::serial::puts("[L0] cannot open crash.log\n");
        return;
    }

    // Write logs
    let file_proto = unsafe { &*(file as *const EfiFileProtocol) };
    let mut size = pos;
    let status = unsafe { (file_proto.write)(file_proto, &mut size, log_buf.as_ptr()) };

    if status != 0 {
        crate::serial::puts("[L0] cannot write crash.log\n");
        return;
    }

    crate::serial::puts("[L0] crash.log written successfully\n");

    // Close file
    unsafe { (file_proto.close)(file as *const EfiFileProtocol) };

    // Clear NVRAM logs for next boot
    nvram_log::clear_log();
}
