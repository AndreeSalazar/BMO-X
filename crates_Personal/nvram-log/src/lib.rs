//! NVRAM-Log — Real-time log via UEFI NVRAM.
//!
//! After ExitBootServices, the kernel cannot access block storage directly.
//! This crate writes log entries to UEFI NVRAM variables. On next boot, the
//! bootloader reads them and dumps everything to `\EFI\BOOT\crash.log`.
//!
//! ## Usage
//!
//! ```rust
//! nvram_log::init(system_table);
//! nvram_log::write_boot_stage("phase_0_to_4");
//! nvram_log::log("APIC timer configured");
//! ```

#![no_std]

extern crate alloc;

use core::fmt;

/// UEFI System Table physical address.
static mut SYSTEM_TABLE: u64 = 0;

/// GUID: FastOS vendor NVRAM (uuid v5 from "fastos-nvram").
#[repr(C)]
#[derive(Clone, Copy)]
struct EfiGuid {
    data1: u32,
    data2: u16,
    data3: u16,
    data4: [u8; 8],
}

static FASTOS_NVRAM_GUID: EfiGuid = EfiGuid {
    data1: 0xc22a_0b40,
    data2: 0x52b8,
    data3: 0x5f95,
    data4: [0xa6, 0x81, 0x4b, 0x5c, 0x42, 0xeb, 0x02, 0x9a],
};

type SetVariableFn = unsafe extern "efiapi" fn(
    name: *const u16,
    guid: *const EfiGuid,
    attributes: u32,
    data_size: usize,
    data: *const u8,
) -> u64;

type GetVariableFn = unsafe extern "efiapi" fn(
    name: *const u16,
    guid: *const EfiGuid,
    attributes: *mut u32,
    data_size: *mut usize,
    data: *mut u8,
) -> u64;

fn runtime_services_ptr() -> Option<u64> {
    let st = unsafe { SYSTEM_TABLE };
    if st == 0 { return None; }
    let rt_ptr = unsafe { core::ptr::read_volatile((st + 0x58) as *const u64) };
    if rt_ptr == 0 { return None; }
    Some(rt_ptr)
}

fn set_variable_ptr() -> Option<SetVariableFn> {
    let rt = runtime_services_ptr()?;
    let fp = unsafe { core::ptr::read_volatile((rt + 0x58) as *const u64) };
    if fp == 0 { return None; }
    Some(unsafe { core::mem::transmute(fp) })
}

fn get_variable_ptr() -> Option<GetVariableFn> {
    let rt = runtime_services_ptr()?;
    let fp = unsafe { core::ptr::read_volatile((rt + 0x48) as *const u64) };
    if fp == 0 { return None; }
    Some(unsafe { core::mem::transmute(fp) })
}

fn str_to_ucs2(name: &str) -> [u16; 64] {
    let mut ucs2 = [0u16; 64];
    let mut i = 0;
    for ch in name.bytes() {
        if i >= 63 { break; }
        ucs2[i] = ch as u16;
        i += 1;
    }
    ucs2[i] = 0;
    ucs2
}

/// NON_VOLATILE | BOOTSERVICE_ACCESS | RUNTIME_ACCESS
const NVRAM_ATTRS: u32 = 0x01 | 0x02 | 0x04;

fn nvram_set(name: &str, data: &[u8]) -> bool {
    let Some(set_var) = set_variable_ptr() else { return false; };
    let ucs2_name = str_to_ucs2(name);
    let status = unsafe {
        set_var(ucs2_name.as_ptr(), &FASTOS_NVRAM_GUID, NVRAM_ATTRS, data.len(), data.as_ptr())
    };
    status == 0
}

fn nvram_get(name: &str) -> Option<[u8; 256]> {
    let get_var = get_variable_ptr()?;
    let ucs2_name = str_to_ucs2(name);
    let mut attrs: u32 = 0;
    let mut data_size: usize = 256;
    let mut buf = [0u8; 256];
    let status = unsafe {
        get_var(ucs2_name.as_ptr(), &FASTOS_NVRAM_GUID, &mut attrs, &mut data_size, buf.as_mut_ptr())
    };
    if status != 0 { return None; }
    Some(buf)
}

/// Write raw bytes to a UEFI NVRAM variable.
pub fn set_variable(name: &str, data: &[u8]) -> bool {
    nvram_set(name, data)
}

/// Read raw bytes from a UEFI NVRAM variable.
pub fn get_variable(name: &str) -> Option<[u8; 256]> {
    nvram_get(name)
}

// ── Public API ───────────────────────────────────────────────────────────────

/// Initialize with the UEFI System Table address from BootInfo.
pub fn init(system_table: u64) {
    unsafe { SYSTEM_TABLE = system_table; }
}

/// Write the current boot stage to NVRAM.
pub fn write_boot_stage(stage: &str) {
    nvram_set("FastOSBootStage", stage.as_bytes());
}

/// Read the last boot stage from NVRAM.
pub fn read_boot_stage() -> Option<alloc::string::String> {
    let buf = nvram_get("FastOSBootStage")?;
    let len = buf.iter().position(|&b| b == 0).unwrap_or(256);
    if len == 0 { return None; }
    Some(alloc::string::String::from_utf8_lossy(&buf[..len]).into_owned())
}

/// Write a crash reason to NVRAM.
pub fn write_crash(reason: &str) {
    nvram_set("FastOSCrash", reason.as_bytes());
}

/// Read the crash reason from NVRAM (if any).
pub fn read_crash() -> Option<alloc::string::String> {
    let buf = nvram_get("FastOSCrash")?;
    let len = buf.iter().position(|&b| b == 0).unwrap_or(256);
    if len == 0 { return None; }
    Some(alloc::string::String::from_utf8_lossy(&buf[..len]).into_owned())
}

/// Clear the crash reason (call on clean boot).
pub fn clear_crash() {
    nvram_set("FastOSCrash", b"");
}

// ── Log ring buffer in NVRAM ─────────────────────────────────────────────────

/// Max log entries stored in NVRAM (256 bytes per variable, 8 variables).
const MAX_LOG_VARS: usize = 8;
const LOG_VAR_PREFIX: &str = "FastOSLog";

static mut LOG_INDEX: usize = 0;

/// Append a log entry to NVRAM ring buffer. Returns true on success.
pub fn log(msg: &str) -> bool {
    let idx = unsafe {
        let i = LOG_INDEX;
        LOG_INDEX = LOG_INDEX.wrapping_add(1);
        i
    };

    let mut entry = [0u8; 256];
    let mut pos = 0;

    // Prefix: [N]
    entry[pos] = b'['; pos += 1;
    let n = idx % MAX_LOG_VARS;
    entry[pos] = b'0' + n as u8; pos += 1;
    entry[pos] = b']'; pos += 1;
    entry[pos] = b' '; pos += 1;

    // Message
    for &b in msg.as_bytes() {
        if pos >= 254 { break; }
        entry[pos] = b;
        pos += 1;
    }
    entry[pos] = b'\n';
    pos += 1;

    // Variable name: FastOSLog0..FastOSLog7
    let mut var_name = [0u8; 16];
    let prefix_bytes = LOG_VAR_PREFIX.as_bytes();
    var_name[..prefix_bytes.len()].copy_from_slice(prefix_bytes);
    var_name[prefix_bytes.len()] = b'0' + n as u8;
    let name = core::str::from_utf8(&var_name[..prefix_bytes.len() + 1])
        .unwrap_or("FastOSLog0");

    nvram_set(name, &entry[..pos])
}

/// Read all log entries from NVRAM.
pub fn read_log() -> alloc::vec::Vec<alloc::string::String> {
    let mut entries = alloc::vec::Vec::new();
    let prefix_bytes = LOG_VAR_PREFIX.as_bytes();
    for i in 0..MAX_LOG_VARS {
        let mut var_name = [0u8; 16];
        var_name[..prefix_bytes.len()].copy_from_slice(prefix_bytes);
        var_name[prefix_bytes.len()] = b'0' + i as u8;
        let name = core::str::from_utf8(&var_name[..prefix_bytes.len() + 1])
            .unwrap_or("FastOSLog0");

        if let Some(buf) = nvram_get(name) {
            let len = buf.iter().position(|&b| b == 0).unwrap_or(256);
            if len > 0 {
                let s = alloc::string::String::from_utf8_lossy(&buf[..len]).into_owned();
                if !s.is_empty() {
                    entries.push(s);
                }
            }
        }
    }
    entries
}

/// Clear all log entries.
pub fn clear_log() {
    let prefix_bytes = LOG_VAR_PREFIX.as_bytes();
    for i in 0..MAX_LOG_VARS {
        let mut var_name = [0u8; 16];
        var_name[..prefix_bytes.len()].copy_from_slice(prefix_bytes);
        var_name[prefix_bytes.len()] = b'0' + i as u8;
        let name = core::str::from_utf8(&var_name[..prefix_bytes.len() + 1])
            .unwrap_or("FastOSLog0");
        nvram_set(name, b"");
    }
}

// ── fmt::Write for use with write!() ─────────────────────────────────────────

/// Wrapper to use `write!(LogWriter, ...)` for structured log entries.
pub struct LogWriter;

impl fmt::Write for LogWriter {
    fn write_str(&mut self, s: &str) -> fmt::Result {
        log(s);
        Ok(())
    }
}

/// Log a formatted message.
pub fn log_fmt(args: fmt::Arguments) {
    use fmt::Write;
    let _ = LogWriter.write_fmt(args);
}

/// Log a formatted message (macro for ergonomic use).
#[macro_export]
macro_rules! log {
    ($($arg:tt)*) => {
        $crate::log_fmt(format_args!($($arg)*))
    };
}
