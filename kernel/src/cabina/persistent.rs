//! `cabina::persistent` — Spool persistente USB-ready.
//!
//! Objetivo final: escribir `/Datos/FASTOS-DIAG.LOG` en el USB/BMO-FS.
//!
//! Por ahora NO toca el disco en el camino crítico. El storage/USB todavía
//! puede congelar el kernel si se llama desde boot, IRQ o render. En vez
//! de eso, formateamos el log en RAM sin allocaciones y exponemos funciones
//! `copy/ack` para que un futuro worker de storage lo haga flush cuando
//! sea seguro.
//!
//! v1.8.8: adaptado desde `bmo_core::diag::persistent` para usar la API
//! moderna de `cabina::event`.

#![allow(dead_code)]

use super::event::{Event, Severity};

/// Ruta objetivo futura para el log persistente en USB.
pub const TARGET_PATH: &str = "/Datos/FASTOS-DIAG.LOG";

/// Capacidad del spool circular (bytes).
const SPOOL_CAP: usize = 16 * 1024; // 16 KB

static mut SPOOL: [u8; SPOOL_CAP] = [0; SPOOL_CAP];
static mut SPOOL_LEN: usize = 0;
static mut SPOOL_DROPPED: u64 = 0;

/// Llamado desde cabina::init(). No-op en v1.8.8.
pub fn init() {}

/// Formatea un evento y lo agrega al spool. Si está lleno, dropea bytes.
pub fn write_event(event: &Event) {
    let mut line_buf = [0u8; 256];
    let len = format_event(event, &mut line_buf);
    unsafe {
        if SPOOL_LEN + len + 1 < SPOOL_CAP {
            core::ptr::copy_nonoverlapping(
                line_buf.as_ptr(), SPOOL.as_mut_ptr().add(SPOOL_LEN), len);
            *SPOOL.as_mut_ptr().add(SPOOL_LEN + len) = b'\n';
            SPOOL_LEN += len + 1;
        } else {
            SPOOL_DROPPED += 1;
        }
    }
}

/// Bytes pendientes en el spool.
pub fn pending_bytes() -> usize { unsafe { SPOOL_LEN } }

/// Bytes dropeados por overflow.
pub fn dropped_bytes() -> u64 { unsafe { SPOOL_DROPPED } }

/// Copia hasta `out.len()` bytes del spool a `out`. Retorna cuántos se copiaron.
pub fn copy_pending(out: &mut [u8]) -> usize {
    unsafe {
        let n = core::cmp::min(out.len(), SPOOL_LEN);
        core::ptr::copy_nonoverlapping(SPOOL.as_ptr(), out.as_mut_ptr(), n);
        n
    }
}

/// Marca como escritos `bytes` bytes del spool (consume desde el frente).
pub fn ack(bytes: usize) {
    unsafe {
        let n = core::cmp::min(bytes, SPOOL_LEN);
        core::ptr::copy(
            SPOOL.as_ptr().add(n),
            SPOOL.as_mut_ptr(),
            SPOOL_LEN - n,
        );
        SPOOL_LEN -= n;
    }
}

// ── Privado ──────────────────────────────────────────────────────

fn format_event(event: &Event, buf: &mut [u8]) -> usize {
    // Formato: "[SEVERITY] module: msg (0xVALUE)"
    let mut i = 0;
    let sev = severity_name(event.severity);
    for b in sev.bytes() { if i < buf.len() { buf[i] = b; i += 1; } }
    if i < buf.len() { buf[i] = b' '; i += 1; }
    for b in event.module.bytes() { if i < buf.len() { buf[i] = b; i += 1; } }
    if i < buf.len() { buf[i] = b':'; i += 1; }
    if i < buf.len() { buf[i] = b' '; i += 1; }
    for b in event.msg.bytes() { if i < buf.len() { buf[i] = b; i += 1; } }
    if event.value != 0 {
        let prefix = b" (0x";
        for b in prefix { if i < buf.len() { buf[i] = *b; i += 1; } }
        i = write_hex(event.value, buf, i);
        if i < buf.len() { buf[i] = b')'; i += 1; }
    }
    i
}

fn severity_name(sev: Severity) -> &'static str {
    match sev {
        Severity::Info    => "INFO",
        Severity::Trace   => "TRACE",
        Severity::Warning => "WARN",
        Severity::Fault   => "FAULT",
        Severity::Panic   => "PANIC",
    }
}

fn write_hex(val: u64, buf: &mut [u8], mut i: usize) -> usize {
    let hex = b"0123456789ABCDEF";
    let mut started = false;
    for shift in (0..16).rev() {
        let nibble = ((val >> (shift * 4)) & 0xF) as usize;
        if nibble != 0 || started || shift == 0 {
            if i < buf.len() { buf[i] = hex[nibble]; i += 1; }
            started = true;
        }
    }
    i
}
