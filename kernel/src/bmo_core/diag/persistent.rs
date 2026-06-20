//! Spool persistente USB-ready para diag/.
//!
//! Objetivo final: escribir `/Datos/FASTOS-DIAG.LOG` en el USB/BMO-FS.
//!
//! Por ahora NO toca el disco en el camino crítico. El storage/USB todavía puede
//! congelar el kernel si se llama desde boot, IRQ o render. En vez de eso,
//! formateamos el log en RAM sin allocaciones y exponemos funciones `copy/ack`
//! para que un futuro worker de storage lo haga flush cuando sea seguro.

use core::sync::atomic::{AtomicU64, AtomicUsize, Ordering};

use super::event::{severity_name, Event};

pub const TARGET_DIR: &str = "/Datos";
pub const TARGET_PATH: &str = "/Datos/FASTOS-DIAG.LOG";

const SPOOL_SIZE: usize = 128 * 1024;

static mut SPOOL: [u8; SPOOL_SIZE] = [0; SPOOL_SIZE];
static START: AtomicUsize = AtomicUsize::new(0);
static LEN: AtomicUsize = AtomicUsize::new(0);
static DROPPED_BYTES: AtomicU64 = AtomicU64::new(0);

pub fn init() {
    write_bytes(b"# FastOS diag persistent spool\n");
    write_bytes(b"# target: /Datos/FASTOS-DIAG.LOG\n");
    write_bytes(b"# mode: RAM-first; USB flush pending safe file writer\n\n");
}

pub fn write_event(event: Event) {
    write_bytes(b"[");
    write_dec(event.seq);
    write_bytes(b"][");
    write_bytes(severity_name(event.severity).as_bytes());
    write_bytes(b"][");
    write_bytes(event.module.as_bytes());
    write_bytes(b"] ");
    write_bytes(event.message.as_bytes());
    if event.has_value {
        write_bytes(b" = 0x");
        write_hex(event.value);
    }
    write_bytes(b"\n");
}

pub fn pending_bytes() -> usize {
    LEN.load(Ordering::Relaxed)
}

pub fn dropped_bytes() -> u64 {
    DROPPED_BYTES.load(Ordering::Relaxed)
}

pub fn copy_pending(out: &mut [u8]) -> usize {
    let start = START.load(Ordering::Relaxed);
    let len = LEN.load(Ordering::Relaxed);
    let to_copy = len.min(out.len());

    for i in 0..to_copy {
        out[i] = unsafe { SPOOL[(start + i) % SPOOL_SIZE] };
    }

    to_copy
}

pub fn ack(bytes: usize) {
    let len = LEN.load(Ordering::Relaxed);
    let n = bytes.min(len);
    if n == 0 { return; }

    let start = START.load(Ordering::Relaxed);
    START.store((start + n) % SPOOL_SIZE, Ordering::Relaxed);
    LEN.store(len - n, Ordering::Relaxed);
}

fn write_bytes(bytes: &[u8]) {
    for &b in bytes {
        push_byte(b);
    }
}

fn push_byte(b: u8) {
    let start = START.load(Ordering::Relaxed);
    let len = LEN.load(Ordering::Relaxed);

    if len >= SPOOL_SIZE {
        DROPPED_BYTES.fetch_add(1, Ordering::Relaxed);
        return;
    }

    let pos = (start + len) % SPOOL_SIZE;
    unsafe { SPOOL[pos] = b; }
    LEN.store(len + 1, Ordering::Relaxed);
}

fn write_dec(mut value: u64) {
    let mut buf = [0u8; 20];
    let mut i = buf.len();
    if value == 0 {
        i -= 1;
        buf[i] = b'0';
    } else {
        while value > 0 && i > 0 {
            i -= 1;
            buf[i] = b'0' + (value % 10) as u8;
            value /= 10;
        }
    }
    write_bytes(&buf[i..]);
}

fn write_hex(value: u64) {
    let hex = b"0123456789ABCDEF";
    for i in (0..16).rev() {
        push_byte(hex[((value >> (i * 4)) & 0xF) as usize]);
    }
}
