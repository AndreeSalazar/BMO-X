//! UEFI Runtime Services — direct FFI for NVRAM variable access.
//!
//! After ExitBootServices, only Runtime Services are available. The kernel
//! calls them via raw function pointers from the UEFI System Table.
//! Used for persistent crash logging that survives AMD watchdog resets.

/// UEFI System Table physical address (set from BootInfo).
static mut SYSTEM_TABLE: u64 = 0;

/// GUID: UEFI Global Variable (standard, same as bootloader uses)
/// {8be4df61-93ca-11d2-aa0d-00e098032b8c}
#[repr(C)]
#[derive(Clone, Copy)]
struct EfiGuid {
    data1: u32,
    data2: u16,
    data3: u16,
    data4: [u8; 8],
}

static GLOBAL_VAR_GUID: EfiGuid = EfiGuid {
    data1: 0x8BE4_DF61,
    data2: 0x93CA,
    data3: 0x11D2,
    data4: [0xAA, 0x0D, 0x00, 0xE0, 0x98, 0x03, 0x2B, 0x8C],
};

/// Initialize with the UEFI System Table address from BootInfo.
pub fn init(system_table: u64) {
    unsafe { SYSTEM_TABLE = system_table; }
    if system_table != 0 {
        crate::dev::console::serial_write("[uefi_rt] init: system_table=0x");
        crate::dev::console::serial_write_u64(system_table, 16);
        crate::dev::console::serial_write("\n");
    }
}

/// Get pointer to EFI_RUNTIME_SERVICES from the System Table.
/// System Table layout: ... offset 0x58 = RuntimeServices pointer ...
fn runtime_services_ptr() -> Option<u64> {
    let st = unsafe { SYSTEM_TABLE };
    if st == 0 { return None; }
    // EFI_SYSTEM_TABLE.RuntimeServices is at offset 0x58
    let rt_ptr = unsafe { core::ptr::read_volatile((st + 0x58) as *const u64) };
    if rt_ptr == 0 { return None; }
    Some(rt_ptr)
}

/// EFI_RUNTIME_SERVICES.SetVariable is at offset 0x58 in the RT table.
/// Signature (8) + Revision (4) + HeaderSize (4) + CRC32 (4) + Reserved (4)
/// + GetTime (8) + SetTime (8) + GetWakeupTime (8) + SetWakeupTime (8)
/// + SetVirtualAddressMap (8) + ConvertPointer (8) + GetVariable (8)
/// + GetNextVariableName (8) = offset 0x58 for SetVariable

type SetVariableFn = unsafe extern "efiapi" fn(
    name: *const u16,
    guid: *const EfiGuid,
    attributes: u32,
    data_size: usize,
    data: *const u8,
) -> u64; // EFI_STATUS

type GetVariableFn = unsafe extern "efiapi" fn(
    name: *const u16,
    guid: *const EfiGuid,
    attributes: *mut u32,
    data_size: *mut usize,
    data: *mut u8,
) -> u64;

fn set_variable_ptr() -> Option<SetVariableFn> {
    let rt = runtime_services_ptr()?;
    // SetVariable at offset 0x58 in EFI_RUNTIME_SERVICES
    let fp = unsafe { core::ptr::read_volatile((rt + 0x58) as *const u64) };
    if fp == 0 { return None; }
    Some(unsafe { core::mem::transmute(fp) })
}

fn get_variable_ptr() -> Option<GetVariableFn> {
    let rt = runtime_services_ptr()?;
    // GetVariable at offset 0x48 in EFI_RUNTIME_SERVICES
    let fp = unsafe { core::ptr::read_volatile((rt + 0x48) as *const u64) };
    if fp == 0 { return None; }
    Some(unsafe { core::mem::transmute(fp) })
}

/// Write a string value to a UEFI NVRAM variable.
/// Returns true on success, false on failure.
pub fn set_variable(name: &str, data: &[u8]) -> bool {
    let Some(set_var) = set_variable_ptr() else {
        crate::dev::console::serial_write("[uefi_rt] SetVariable: no function pointer\n");
        return false;
    };

    // Convert name to UCS-2 (null-terminated)
    let mut ucs2_name = [0u16; 64];
    let mut i = 0;
    for ch in name.bytes() {
        if i >= 63 { break; }
        ucs2_name[i] = ch as u16;
        i += 1;
    }
    ucs2_name[i] = 0; // null terminator

    // UEFI variable attributes: BOOTSERVICE_ACCESS | RUNTIME_ACCESS
    const ATTRS: u32 = 0x01 | 0x02; // EFI_VARIABLE_BOOTSERVICE_ACCESS | EFI_VARIABLE_RUNTIME_ACCESS

    let status = unsafe {
        set_var(
            ucs2_name.as_ptr(),
            &GLOBAL_VAR_GUID as *const EfiGuid,
            ATTRS,
            data.len(),
            data.as_ptr(),
        )
    };

    if status != 0 {
        crate::dev::console::serial_write("[uefi_rt] SetVariable failed: status=0x");
        crate::dev::console::serial_write_u64(status, 16);
        crate::dev::console::serial_write("\n");
        return false;
    }
    true
}

/// Read a string value from a UEFI NVRAM variable.
/// Returns the data if found, None otherwise.
pub fn get_variable(name: &str) -> Option<[u8; 256]> {
    let Some(get_var) = get_variable_ptr() else { return None; };

    // Convert name to UCS-2 (null-terminated)
    let mut ucs2_name = [0u16; 64];
    let mut i = 0;
    for ch in name.bytes() {
        if i >= 63 { break; }
        ucs2_name[i] = ch as u16;
        i += 1;
    }
    ucs2_name[i] = 0;

    let mut attrs: u32 = 0;
    let mut data_size: usize = 256;
    let mut buf = [0u8; 256];

    let status = unsafe {
        get_var(
            ucs2_name.as_ptr(),
            &GLOBAL_VAR_GUID as *const EfiGuid,
            &mut attrs,
            &mut data_size,
            buf.as_mut_ptr(),
        )
    };

    if status != 0 { return None; }
    Some(buf)
}

/// Convenience: write a boot stage string to NVRAM.
pub fn write_boot_stage(stage: &str) {
    crate::dev::console::serial_write("[uefi_rt] write_boot_stage: ");
    crate::dev::console::serial_write(stage);
    crate::dev::console::serial_write("\n");
    set_variable("FastOSBootStage", stage.as_bytes());
}

/// Convenience: read the last boot stage from NVRAM.
pub fn read_boot_stage() -> Option<alloc::string::String> {
    let buf = get_variable("FastOSBootStage")?;
    // Find null terminator
    let mut len = 0;
    while len < 256 && buf[len] != 0 { len += 1; }
    if len == 0 { return None; }
    let slice = &buf[..len];
    Some(alloc::string::String::from_utf8_lossy(slice).into_owned())
}
