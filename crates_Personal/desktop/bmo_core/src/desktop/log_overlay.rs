//! On-screen log overlay — captures serial_write calls and shows them
//! on the welcome screen (CABINA) so the user can diagnose issues
//! without needing a serial port.
//!
//! Uses HAL function pointers for storage and filesystem access.

const LOG_LINE_MAX: usize = 128;
const LOG_LINES_MAX: usize = 20;

static mut LOG_LINES: [[u8; LOG_LINE_MAX]; LOG_LINES_MAX] = [[0; LOG_LINE_MAX]; LOG_LINES_MAX];
static mut LOG_HEAD: usize = 0;
static mut LOG_COUNT: usize = 0;
static mut DISK_FLUSH_COUNT: usize = 0;

/// Build the full log as a single byte buffer (newline-separated lines).
fn build_log_blob() -> ([u8; 4096], usize) {
    let mut buf = [0u8; 4096];
    let mut off = 0usize;
    unsafe {
        let start = if LOG_COUNT < LOG_LINES_MAX { 0 } else { LOG_HEAD };
        let n = LOG_COUNT.min(LOG_LINES_MAX);
        for k in 0..n {
            let idx = (start + k) % LOG_LINES_MAX;
            let len = LOG_LINES[idx].iter().position(|&b| b == 0).unwrap_or(LOG_LINE_MAX);
            if off + len + 1 >= buf.len() { break; }
            buf[off..off + len].copy_from_slice(&LOG_LINES[idx][..len]);
            buf[off + len] = b'\n';
            off += len + 1;
        }
    }
    (buf, off)
}

/// Flush the current log buffer to the boot drive.
/// Uses HAL function pointers for storage and filesystem access.
pub fn flush_to_disk() {
    use crate::hal;
    use crate::dev::console::{serial_write, serial_write_u64};

    let (buf, len) = build_log_blob();
    if len == 0 { return; }

    let mut name_8_3 = [b' '; 11];
    let name = b"CABINA.LOG";
    for (i, &b) in name.iter().enumerate() { name_8_3[i] = b; }

    serial_write("[flush] start\n");

    let h = match unsafe { hal::HAL.as_ref() } {
        Some(h) => h,
        None => { serial_write("[flush] no HAL\n"); return; }
    };

    let port_count = (h.storage_port_count)();
    serial_write("[flush] ports=");
    serial_write_u64(port_count as u64, 10);
    serial_write("\n");

    for i in 0..port_count {
        if !(h.storage_port_active)(i) { continue; }
        serial_write("[flush] port ");
        serial_write_u64(i as u64, 10);
        serial_write(" active\n");

        // ALWAYS write raw sector first (guaranteed to work if AHCI works)
        let mut raw_buf = [0u8; 512];
        let copy_len = len.min(500);
    unsafe {
        raw_buf[0..4].copy_from_slice(b"BMOL");
        raw_buf[4..8].copy_from_slice(&(copy_len as u32).to_le_bytes());
        raw_buf[8..12].copy_from_slice(&(DISK_FLUSH_COUNT as u32).to_le_bytes());
        raw_buf[12..16].copy_from_slice(&[0u8; 4]);
        raw_buf[16..16+copy_len].copy_from_slice(&buf[..copy_len]);
    }
    let raw_result = (h.storage_write_sectors)(i, 6, 1, raw_buf.as_ptr());
        serial_write("[flush] raw sector 6 write=");
        serial_write_u64(raw_result as u64, 10);
        serial_write("\n");

        // Try FAT32 mount via HAL
        let mounted = (h.fs_mount)(i);
        serial_write("[flush] mount on port ");
        serial_write_u64(i as u64, 10);
        serial_write(if mounted { "= OK\n" } else { "= FAIL\n" });
        if !mounted { continue; }

        // Try to find EFI\BOOT directory
        let efi_cl = (h.fs_find_subdir)(i, "EFI");
        serial_write("[flush] EFI=");
        serial_write_u64(efi_cl.unwrap_or(0) as u64, 16);
        serial_write("\n");

        if let Some(_efi_cl) = efi_cl {
            if let Some(_boot_cl) = (h.fs_find_subdir)(i, "EFI\\BOOT") {
                // Try to write CABINA.LOG to EFI\BOOT
                let written = (h.fs_write_file)(i, "EFI\\BOOT\\CABINA.LOG", &buf[..len]);
                if written {
                    unsafe { DISK_FLUSH_COUNT += 1; }
                    serial_write("[flush] WROTE EFI/BOOT/cabina.log\n");
                    return;
                }
            }
        }

        // Fallback: raw sector was already written
        serial_write("[flush] file write failed, raw sector was written\n");
    }
}

pub fn log_line(s: &str) {
    let bytes = s.as_bytes();
    unsafe {
        let line = &mut LOG_LINES[LOG_HEAD];
        let n = bytes.len().min(LOG_LINE_MAX);
        line[..n].copy_from_slice(&bytes[..n]);
        for i in n..LOG_LINE_MAX { line[i] = 0; }
        LOG_HEAD = (LOG_HEAD + 1) % LOG_LINES_MAX;
        if LOG_COUNT < LOG_LINES_MAX { LOG_COUNT += 1; }
    }
}

pub fn log_line_bytes(s: &[u8]) {
    unsafe {
        let line = &mut LOG_LINES[LOG_HEAD];
        let n = s.len().min(LOG_LINE_MAX);
        line[..n].copy_from_slice(&s[..n]);
        for i in n..LOG_LINE_MAX { line[i] = 0; }
        LOG_HEAD = (LOG_HEAD + 1) % LOG_LINES_MAX;
        if LOG_COUNT < LOG_LINES_MAX { LOG_COUNT += 1; }
    }
}

pub fn log_line_u64(prefix: &str, val: u64, suffix: &str) {
    let mut buf = [0u8; LOG_LINE_MAX];
    let p = prefix.as_bytes().len().min(LOG_LINE_MAX);
    buf[..p].copy_from_slice(&prefix.as_bytes()[..p]);
    let mut i = p;
    if i < LOG_LINE_MAX {
        let s = if suffix.is_empty() { b"" } else { suffix.as_bytes() };
        let v = if val == 0 {
            buf[i] = b'0'; i += 1;
            s
        } else {
            let mut tmp = [0u8; 20];
            let mut n = val;
            let mut j = 0;
            while n > 0 && j < 20 {
                tmp[j] = b'0' + (n % 10) as u8;
                n /= 10; j += 1;
            }
            while j > 0 && i < LOG_LINE_MAX {
                j -= 1;
                buf[i] = tmp[j];
                i += 1;
            }
            s
        };
        let sl = v.len().min(LOG_LINE_MAX - i);
        buf[i..i + sl].copy_from_slice(&v[..sl]);
        i += sl;
    }
    log_line_bytes(&buf[..i]);
}

pub fn snapshot() -> ([(usize, &'static [u8]); LOG_LINES_MAX], usize) {
    let mut out: [(usize, &'static [u8]); LOG_LINES_MAX] = [(0, &[]); LOG_LINES_MAX];
    let count = unsafe { LOG_COUNT };
    if count == 0 {
        return (out, 0);
    }
    let head = unsafe { LOG_HEAD };
    let start = if count < LOG_LINES_MAX { 0 } else { head };
    unsafe {
        for k in 0..count {
            let idx = (start + k) % LOG_LINES_MAX;
            let len = LOG_LINES[idx].iter().position(|&b| b == 0).unwrap_or(LOG_LINE_MAX);
            out[k] = (idx, &LOG_LINES[idx][..len]);
        }
    }
    (out, count)
}

pub const fn max_lines() -> usize { LOG_LINES_MAX }
