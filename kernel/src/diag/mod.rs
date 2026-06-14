//! FastOS diag/ — diagnóstico visual y serial integrado desde Ring 0.
//!
//! Objetivo inmediato:
//! - no depender de drivers externos,
//! - funcionar con `no_std`,
//! - guardar una pequeña caja negra en RAM,
//! - pintar los últimos eventos sobre el framebuffer GOP,
//! - dejar preparada la futura persistencia BMO-FS/USB.

#![allow(dead_code)]

use crate::boot_info;
use crate::drivers::serial;
use crate::font;

const MAX_EVENTS: usize = 64;
const OVERLAY_LINES: usize = 8;
const OVERLAY_W: usize = 760;
const OVERLAY_H: usize = 18 + OVERLAY_LINES * 18;

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Severity {
    Info,
    Warn,
    Fault,
    Panic,
}

#[derive(Clone, Copy)]
struct Event {
    seq: u64,
    severity: Severity,
    module: &'static str,
    message: &'static str,
    value: u64,
    has_value: bool,
}

impl Event {
    const fn empty() -> Self {
        Self {
            seq: 0,
            severity: Severity::Info,
            module: "",
            message: "",
            value: 0,
            has_value: false,
        }
    }
}

static mut EVENTS: [Event; MAX_EVENTS] = [Event::empty(); MAX_EVENTS];
static mut NEXT_SEQ: u64 = 1;
static mut OVERLAY_ENABLED: bool = true;

pub fn init() {
    event(Severity::Info, "diag", "diag online: serial + GOP overlay + RAM blackbox");
}

pub fn info(module: &'static str, message: &'static str) {
    event(Severity::Info, module, message);
}

pub fn warn(module: &'static str, message: &'static str) {
    event(Severity::Warn, module, message);
}

pub fn fault(module: &'static str, message: &'static str) {
    event(Severity::Fault, module, message);
}

pub fn panic_event(module: &'static str, message: &'static str) {
    event(Severity::Panic, module, message);
}

pub fn info_u64(module: &'static str, message: &'static str, value: u64) {
    event_u64(Severity::Info, module, message, value);
}

pub fn warn_u64(module: &'static str, message: &'static str, value: u64) {
    event_u64(Severity::Warn, module, message, value);
}

pub fn fault_u64(module: &'static str, message: &'static str, value: u64) {
    event_u64(Severity::Fault, module, message, value);
}

pub fn set_overlay_enabled(enabled: bool) {
    unsafe { OVERLAY_ENABLED = enabled; }
}

pub fn event(severity: Severity, module: &'static str, message: &'static str) {
    push(Event {
        seq: 0,
        severity,
        module,
        message,
        value: 0,
        has_value: false,
    });
}

pub fn event_u64(severity: Severity, module: &'static str, message: &'static str, value: u64) {
    push(Event {
        seq: 0,
        severity,
        module,
        message,
        value,
        has_value: true,
    });
}

fn push(mut event: Event) {
    unsafe {
        event.seq = NEXT_SEQ;
        NEXT_SEQ = NEXT_SEQ.wrapping_add(1).max(1);
        EVENTS[(event.seq as usize - 1) % MAX_EVENTS] = event;
    }

    write_serial(event);
    paint_overlay();
}

fn write_serial(event: Event) {
    serial::serial_write("[DIAG][");
    serial::serial_write(severity_name(event.severity));
    serial::serial_write("][");
    serial::serial_write(event.module);
    serial::serial_write("] ");
    serial::serial_write(event.message);
    if event.has_value {
        serial::serial_write(" = ");
        serial_hex(event.value);
    }
    serial::serial_write("\n");
}

fn severity_name(severity: Severity) -> &'static str {
    match severity {
        Severity::Info => "INFO",
        Severity::Warn => "WARN",
        Severity::Fault => "FAULT",
        Severity::Panic => "PANIC",
    }
}

fn severity_tag(severity: Severity) -> &'static [u8] {
    match severity {
        Severity::Info => b"INFO ",
        Severity::Warn => b"WARN ",
        Severity::Fault => b"FAULT",
        Severity::Panic => b"PANIC",
    }
}

fn severity_color(severity: Severity) -> u32 {
    match severity {
        Severity::Info => 0xFF58A6FF,
        Severity::Warn => 0xFFFFBD2E,
        Severity::Fault => 0xFFFF7B72,
        Severity::Panic => 0xFFFF2A2A,
    }
}

fn serial_hex(val: u64) {
    let hex = b"0123456789ABCDEF";
    serial::serial_write("0x");
    for i in (0..16).rev() {
        serial::serial_write_byte(hex[((val >> (i * 4)) & 0xF) as usize]);
    }
}

pub fn paint_overlay() {
    if unsafe { !OVERLAY_ENABLED } { return; }

    let Some((base, width, height, stride)) = fb() else { return; };
    if width < 320 || height < 220 { return; }

    let x = 12usize;
    let y = height.saturating_sub(OVERLAY_H + 12);
    let w = OVERLAY_W.min(width.saturating_sub(x + 1));
    let h = OVERLAY_H.min(height.saturating_sub(y + 1));

    fill_rect(base, stride, width, height, x, y, w, h, 0xCC050810);
    draw_rect(base, stride, width, height, x, y, w, h, 0xFF56D4DD);
    draw_text(base, stride, width, height, x + 10, y + 6, b"FastOS diag/ live", 0xFFE6EDF3);

    let next = unsafe { NEXT_SEQ };
    let first = next.saturating_sub(OVERLAY_LINES as u64);
    let mut row = 0usize;
    let mut seq = first;
    while seq < next && row < OVERLAY_LINES {
        if let Some(ev) = event_by_seq(seq) {
            draw_event_line(base, stride, width, height, x + 10, y + 28 + row * 18, ev);
            row += 1;
        }
        seq += 1;
    }
}

fn event_by_seq(seq: u64) -> Option<Event> {
    if seq == 0 { return None; }
    let ev = unsafe { EVENTS[(seq as usize - 1) % MAX_EVENTS] };
    if ev.seq == seq { Some(ev) } else { None }
}

fn draw_event_line(
    base: *mut u32,
    stride: usize,
    width: usize,
    height: usize,
    x: usize,
    y: usize,
    ev: Event,
) {
    let color = severity_color(ev.severity);
    draw_text(base, stride, width, height, x, y, b"[", 0xFFE6EDF3);
    draw_text(base, stride, width, height, x + 8, y, severity_tag(ev.severity), color);
    draw_text(base, stride, width, height, x + 48, y, b"] ", 0xFFE6EDF3);
    draw_text_str(base, stride, width, height, x + 64, y, ev.module, 0xFF76B900);
    draw_text(base, stride, width, height, x + 160, y, b": ", 0xFFE6EDF3);
    draw_text_str(base, stride, width, height, x + 176, y, ev.message, 0xFFE6EDF3);
    if ev.has_value {
        draw_text(base, stride, width, height, x + 600, y, b"0x", 0xFF8B949E);
        draw_hex(base, stride, width, height, x + 616, y, ev.value, 0xFF8B949E);
    }
}

fn fb() -> Option<(*mut u32, usize, usize, usize)> {
    let (addr, w, h, s) = unsafe {
        (
            boot_info::FB_ADDR,
            boot_info::FB_WIDTH as usize,
            boot_info::FB_HEIGHT as usize,
            boot_info::FB_STRIDE as usize,
        )
    };
    if addr == 0 || w == 0 || h == 0 || s == 0 { return None; }
    Some((addr as *mut u32, w, h, s))
}

fn fill_rect(
    base: *mut u32,
    stride: usize,
    width: usize,
    height: usize,
    x: usize,
    y: usize,
    w: usize,
    h: usize,
    color: u32,
) {
    let x1 = (x + w).min(width);
    let y1 = (y + h).min(height);
    for yy in y..y1 {
        for xx in x..x1 {
            unsafe { base.add(yy * stride + xx).write_volatile(color); }
        }
    }
}

fn draw_rect(
    base: *mut u32,
    stride: usize,
    width: usize,
    height: usize,
    x: usize,
    y: usize,
    w: usize,
    h: usize,
    color: u32,
) {
    fill_rect(base, stride, width, height, x, y, w, 1, color);
    fill_rect(base, stride, width, height, x, y + h.saturating_sub(1), w, 1, color);
    fill_rect(base, stride, width, height, x, y, 1, h, color);
    fill_rect(base, stride, width, height, x + w.saturating_sub(1), y, 1, h, color);
}

fn draw_text_str(
    base: *mut u32,
    stride: usize,
    width: usize,
    height: usize,
    x: usize,
    y: usize,
    text: &str,
    color: u32,
) {
    draw_text(base, stride, width, height, x, y, text.as_bytes(), color);
}

fn draw_text(
    base: *mut u32,
    stride: usize,
    width: usize,
    height: usize,
    x: usize,
    y: usize,
    text: &[u8],
    color: u32,
) {
    let mut cx = x;
    for &ch in text {
        if cx + 8 >= width || y + 16 >= height { break; }
        let glyph = font::get_glyph(ch);
        for gy in 0..16 {
            let row = glyph[gy];
            for gx in 0..8 {
                if (row & (0x80 >> gx)) != 0 {
                    unsafe { base.add((y + gy) * stride + cx + gx).write_volatile(color); }
                }
            }
        }
        cx += 8;
    }
}

fn draw_hex(
    base: *mut u32,
    stride: usize,
    width: usize,
    height: usize,
    x: usize,
    y: usize,
    value: u64,
    color: u32,
) {
    let hex = b"0123456789ABCDEF";
    let mut buf = [0u8; 16];
    for i in 0..16 {
        let shift = (15 - i) * 4;
        buf[i] = hex[((value >> shift) & 0xF) as usize];
    }
    draw_text(base, stride, width, height, x, y, &buf, color);
}

#[macro_export]
macro_rules! diag_info {
    ($module:expr, $message:expr) => {
        $crate::diag::info($module, $message)
    };
}

#[macro_export]
macro_rules! diag_warn {
    ($module:expr, $message:expr) => {
        $crate::diag::warn($module, $message)
    };
}

#[macro_export]
macro_rules! diag_fault {
    ($module:expr, $message:expr) => {
        $crate::diag::fault($module, $message)
    };
}
