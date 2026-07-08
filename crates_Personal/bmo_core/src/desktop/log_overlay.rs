//! On-screen log overlay — captures serial_write calls and shows them
//! on the welcome screen (CABINA) so the user can diagnose issues
//! without needing a serial port.

const LOG_LINE_MAX: usize = 128;
const LOG_LINES_MAX: usize = 20;

static mut LOG_LINES: [[u8; LOG_LINE_MAX]; LOG_LINES_MAX] = [[0; LOG_LINE_MAX]; LOG_LINES_MAX];
static mut LOG_HEAD: usize = 0;
static mut LOG_COUNT: usize = 0;

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
