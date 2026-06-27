//! CABINA_0 — Ring 0 omniscient backbone.
//!
//! Minimal, zero-heap, zero-allocation diagnostic emitter for the kernel.
//! Every event goes to COM1 serial (always) + static ring buffer (64 slots).
//! Survives kernel crashes — serial output is the primary diagnostic.
//!
//! CABINA_0 works BEFORE init_heap, BEFORE GDT/IDT, BEFORE anything.
//! Only depends on: COM1 port (0x3F8) and static memory.

use core::sync::atomic::{AtomicUsize, Ordering};
use core::arch::asm;

// ── Severity ───────────────────────────────────────────────────────

#[derive(Clone, Copy, PartialEq)]
#[repr(u8)]
pub enum Severity {
    Info  = 0,
    Warn  = 1,
    Fault = 2,
}

impl Severity {
    fn tag(self) -> &'static [u8] {
        match self {
            Severity::Info  => b"INF",
            Severity::Warn  => b"WRN",
            Severity::Fault => b"FLT",
        }
    }
}

// ── COM1 direct output ─────────────────────────────────────────────

const COM1_DATA: u16 = 0x3F8;
const COM1_LSR: u16 = 0x3FD;
const LSR_THRE: u8 = 0x20;

#[inline]
fn serial_byte(b: u8) {
    unsafe {
        loop {
            let lsr: u8;
            asm!("in al, dx", out("al") lsr, in("dx") COM1_LSR, options(nostack));
            if lsr & LSR_THRE != 0 { break; }
        }
        asm!("out dx, al", in("dx") COM1_DATA, in("al") b, options(nostack));
    }
}

fn serial_bytes(s: &[u8]) {
    for &b in s { serial_byte(b); }
}

fn serial_hex8(mut v: u8) {
    const HEX: [u8; 16] = *b"0123456789ABCDEF";
    serial_byte(HEX[(v >> 4) as usize]);
    serial_byte(HEX[(v & 0xF) as usize]);
}

fn serial_hex64(mut v: u64) {
    serial_byte(b'0'); serial_byte(b'x');
    let mut started = false;
    let mut i: i64 = 60;
    loop {
        let nib = ((v >> i) & 0xF) as u8;
        if nib != 0 || started || i == 0 {
            serial_byte(HEX[nib as usize]);
            started = true;
        }
        if i == 0 { break; }
        i -= 4;
    }
    if !started { serial_byte(b'0'); }
}

const HEX: [u8; 16] = *b"0123456789ABCDEF";

// ── Event ──────────────────────────────────────────────────────────

#[derive(Clone, Copy)]
struct Event {
    severity: Severity,
    module: &'static str,
    msg: &'static str,
    timestamp: u64,
}

// ── Ring buffer (64 slots, lock-free via single-writer atomic) ─────

const RING_CAP: usize = 64;

static mut RING: [Event; RING_CAP] = [Event {
    severity: Severity::Info,
    module: "",
    msg: "",
    timestamp: 0,
}; RING_CAP];

static RING_HEAD: AtomicUsize = AtomicUsize::new(0);
static RING_COUNT: AtomicUsize = AtomicUsize::new(0);

// ── Init ───────────────────────────────────────────────────────────

pub fn init() {
    RING_HEAD.store(0, Ordering::Relaxed);
    RING_COUNT.store(0, Ordering::Relaxed);
}

// ── Core emit ──────────────────────────────────────────────────────

fn rdtsc() -> u64 {
    unsafe {
        let lo: u32;
        let hi: u32;
        asm!("rdtsc", out("eax") lo, out("edx") hi);
        ((hi as u64) << 32) | (lo as u64)
    }
}

/// Emit an event. Writes to serial immediately + stores in ring buffer.
pub fn emit(severity: Severity, module: &'static str, msg: &'static str) {
    let ts = rdtsc();

    // 1. Serial output — always works, even before anything else
    serial_bytes(b"[C0 ");
    serial_bytes(severity.tag());
    serial_bytes(b"] ");
    serial_bytes(module.as_bytes());
    serial_bytes(b": ");
    serial_bytes(msg.as_bytes());
    serial_bytes(b" @t=");
    // Write timestamp as hex
    {
        let mut tmp = [0u8; 16];
        let mut pos = 16;
        let mut v = ts;
        if v == 0 { pos = 15; tmp[15] = b'0'; }
        else {
            while v > 0 { pos -= 1; tmp[pos] = HEX[(v & 0xF) as usize]; v >>= 4; }
        }
        serial_bytes(&tmp[pos..]);
    }
    serial_bytes(b"\r\n");

    // 2. Ring buffer — for later dump or CABINA_3 consumption
    unsafe {
        let head = RING_HEAD.load(Ordering::Relaxed);
        let count = RING_COUNT.load(Ordering::Relaxed);
        let idx = if count < RING_CAP { head } else { (head + 1) % RING_CAP };
        RING[idx] = Event { severity, module, msg, timestamp: ts };
        RING_HEAD.store((idx + 1) % RING_CAP, Ordering::Relaxed);
        if count < RING_CAP {
            RING_COUNT.store(count + 1, Ordering::Relaxed);
        }
    }
}

// ── Convenience API ────────────────────────────────────────────────

pub fn info(module: &'static str, msg: &'static str) {
    emit(Severity::Info, module, msg);
}

pub fn warn(module: &'static str, msg: &'static str) {
    emit(Severity::Warn, module, msg);
}

pub fn fault(module: &'static str, msg: &'static str) {
    emit(Severity::Fault, module, msg);
}

// ── Dump ring buffer to serial ─────────────────────────────────────

pub fn dump_serial() {
    let count = RING_COUNT.load(Ordering::Relaxed);
    let head = RING_HEAD.load(Ordering::Relaxed);
    if count == 0 {
        serial_bytes(b"[C0] ring buffer empty\r\n");
        return;
    }
    let start = if count < RING_CAP { 0 } else { head };
    serial_bytes(b"\r\n=== CABINA_0 DUMP (");
    // write count as decimal
    {
        let mut buf = [0u8; 10];
        let mut pos = 10;
        let mut v = count;
        if v == 0 { pos = 9; buf[9] = b'0'; }
        else { while v > 0 { pos -= 1; buf[pos] = b'0' + (v % 10) as u8; v /= 10; } }
        serial_bytes(&buf[pos..]);
    }
    serial_bytes(b" events) ===\r\n");
    let n = count.min(RING_CAP);
    for i in 0..n {
        let idx = (start + i) % RING_CAP;
        unsafe {
            let ev = &RING[idx];
            serial_bytes(b"  [");
            serial_bytes(ev.severity.tag());
            serial_bytes(b"] ");
            serial_bytes(ev.module.as_bytes());
            serial_bytes(b": ");
            serial_bytes(ev.msg.as_bytes());
            serial_bytes(b"\r\n");
        }
    }
    serial_bytes(b"=== END CABINA_0 DUMP ===\r\n");
}

// ── Query (for CABINA_3 or crash analysis) ─────────────────────────

pub fn ring_count() -> usize {
    RING_COUNT.load(Ordering::Relaxed)
}

pub fn ring_peek(index: usize) -> Option<(Severity, &'static str, &'static str)> {
    let count = RING_COUNT.load(Ordering::Relaxed);
    if index >= count { return None; }
    let cap = count.min(RING_CAP);
    let start = if count < RING_CAP { 0 } else {
        RING_HEAD.load(Ordering::Relaxed)
    };
    let idx = (start + index) % RING_CAP;
    unsafe {
        let ev = &RING[idx];
        Some((ev.severity, ev.module, ev.msg))
    }
}
