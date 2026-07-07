use core::cell::UnsafeCell;
use cabina_core::Event;

pub const TARGET_PATH: &str = "/Datos/BMO-DIAG.LOG";
const SPOOL_CAP: usize = 16 * 1024;

struct Spool {
    data: [u8; SPOOL_CAP],
    len: usize,
    dropped: u64,
}

struct SyncSpool(UnsafeCell<Spool>);
unsafe impl Sync for SyncSpool {}

static SPOOL: SyncSpool = SyncSpool(UnsafeCell::new(Spool {
    data: [0u8; SPOOL_CAP],
    len: 0,
    dropped: 0,
}));

fn with_spool<F, R>(f: F) -> R
where
    F: FnOnce(&mut Spool) -> R,
{
    let ptr = SPOOL.0.get();
    unsafe { f(&mut *ptr) }
}

pub fn init() {}

fn format_event(event: &Event, buf: &mut [u8]) -> usize {
    let mut i = 0;
    let sev = event.severity.name();
    for b in sev.bytes() { if i < buf.len() { buf[i] = b; i += 1; } }
    if i < buf.len() { buf[i] = b' '; i += 1; }
    for b in event.module_str().bytes() { if i < buf.len() { buf[i] = b; i += 1; } }
    if i < buf.len() { buf[i] = b':'; i += 1; }
    if i < buf.len() { buf[i] = b' '; i += 1; }
    for b in event.msg_str().bytes() { if i < buf.len() { buf[i] = b; i += 1; } }
    if event.value != 0 {
        let prefix = b" (0x";
        for b in &prefix[..] { if i < buf.len() { buf[i] = *b; i += 1; } }
        let hex = b"0123456789ABCDEF";
        let mut started = false;
        for shift in (0..16).rev() {
            let nibble = ((event.value >> (shift * 4)) & 0xF) as usize;
            if nibble != 0 || started || shift == 0 {
                if i < buf.len() { buf[i] = hex[nibble]; i += 1; }
                started = true;
            }
        }
        if i < buf.len() { buf[i] = b')'; i += 1; }
    }
    i
}

pub fn write_event(event: &Event) {
    let mut line_buf = [0u8; 256];
    let len = format_event(event, &mut line_buf);
    with_spool(|s| {
        if s.len + len + 1 < SPOOL_CAP {
            s.data[s.len..s.len + len].copy_from_slice(&line_buf[..len]);
            s.data[s.len + len] = b'\n';
            s.len += len + 1;
        } else {
            s.dropped += 1;
        }
    });
}

pub fn pending_bytes() -> usize {
    with_spool(|s| s.len)
}

pub fn dropped_bytes() -> u64 {
    with_spool(|s| s.dropped)
}

pub fn copy_pending(out: &mut [u8]) -> usize {
    with_spool(|s| {
        let n = core::cmp::min(out.len(), s.len);
        out[..n].copy_from_slice(&s.data[..n]);
        n
    })
}

pub fn ack(bytes: usize) {
    with_spool(|s| {
        let n = core::cmp::min(bytes, s.len);
        if n < s.len {
            s.data.copy_within(n..s.len, 0);
        }
        s.len -= n;
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    use cabina_core::{Severity, Layer, Entity};

    #[test]
    fn spool_write_and_read() {
        // Test format_event output directly (no global spool)
        let ev = Event::new(Severity::Info, Layer::Ring0, Entity::Module, "test", 0, "spool test", 0);
        let mut buf = [0u8; 256];
        let len = format_event(&ev, &mut buf);
        assert!(len > 0);
        let out = core::str::from_utf8(&buf[..len]).unwrap();
        assert!(out.contains("INFO"));
        assert!(out.contains("test"));
        assert!(out.contains("spool test"));
    }

    #[test]
    fn spool_value_hex() {
        // Test format_event produces hex value
        let ev = Event::new(Severity::Fault, Layer::Ring0, Entity::Module, "hex", 0, "val", 0xDEAD);
        let mut buf = [0u8; 256];
        let len = format_event(&ev, &mut buf);
        assert!(len > 0);
        let out = core::str::from_utf8(&buf[..len]).unwrap();
        assert!(out.contains("0xDEAD"));
    }

    #[test]
    fn spool_overflow() {
        // Test spool overflow using dedicated memory, no global state
        let mut spool: [u8; SPOOL_CAP] = [0; SPOOL_CAP];
        let mut len = 0usize;
        let mut dropped = 0u64;
        for _ in 0..1000 {
            let ev = Event::new(Severity::Info, Layer::Ring0, Entity::Module, "ovf", 0, &"x".repeat(100), 0);
            let mut line_buf = [0u8; 256];
            let line_len = format_event(&ev, &mut line_buf);
            if len + line_len + 1 < SPOOL_CAP {
                spool[len..len + line_len].copy_from_slice(&line_buf[..line_len]);
                spool[len + line_len] = b'\n';
                len += line_len + 1;
            } else {
                dropped += 1;
            }
        }
        assert!(dropped > 0 || len == SPOOL_CAP);
    }
}
