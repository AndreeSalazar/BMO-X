//! `cabina::panels` — Cada pestaña del HUD es un panel.
//!
//! Los paneles son funciones puras: toman un `Snapshot` y dibujan
//! sobre el framebuffer. No leen estado mutable (excepto el FB).
//!
//! ## Tabs actuales
//!
//! - 0: Overview (resumen de todo)
//! - 1: CPU (interrupts, faults, ticks)
//! - 2: Memory (allocs, frees, heap)
//! - 3: I/O (PCI, serial, PS/2)
//! - 4: Scheduler (ctx switches, procs, threads)
//! - 5: Syscall (todos los syscalls con count > 0)
//! - 6: Log (eventos filtrados)
//! - 7: GPU (futuro RDNA4)

#![allow(dead_code)]

use crate::cabina::snapshot::Snapshot;
use crate::cabina::filter::{EventFilter, apply};

/// Número de paneles (tabs).
pub const PANEL_COUNT: usize = 8;

/// Punto de entrada: renderiza el panel `tab`.
pub fn render(tab: u8, s: &Snapshot) {
    match tab {
        0 => overview::render(s),
        1 => cpu::render(s),
        2 => memory::render(s),
        3 => io::render(s),
        4 => scheduler::render(s),
        5 => syscall::render(s),
        6 => log::render(s),
        7 => gpu::render(s),
        _ => overview::render(s),
    }
}

// ─── Helpers de dibujo (thin wrappers sobre el framebuffer) ──────

/// Pinta un texto en (x, y) con color.
/// v1.8.8: si hay framebuffer disponible, dibuja; si no, va a serial.
fn draw_text(_x: u32, _y: u32, s: &str, _color: u32) {
    // v1.8.8: simplificación. Escribimos a serial (no FB) porque el
    // proyecto todavía no tiene draw_text consolidado.
    // TODO: usar FB cuando esté disponible.
    crate::dev::console::serial_write("[cabina] ");
    crate::dev::console::serial_write(s);
    crate::dev::console::serial_write("\n");
}

/// Pinta un rectángulo.
fn fill_rect(_x: u32, _y: u32, _w: u32, _h: u32, _color: u32) {
    // v1.8.8: simplificación.
}

/// Pinta un header de tab.
fn draw_header(tab: u8) {
    let tabs = ["Overview", "CPU", "Memory", "I/O", "Sched", "Syscall", "Log", "GPU"];
    fill_rect(0, 0, 1920, 32, 0xFF202020);
    let mut x = 8u32;
    for (i, name) in tabs.iter().enumerate() {
        let color = if i as u8 == tab { 0xFF00FF00 } else { 0xFFAAAAAA };
        draw_text(x, 8, name, color);
        x += name.len() as u32 * 8 + 24;
    }
    draw_text(1700, 8, "Cabina v1.0", 0xFF888888);
}

// ─── Overview ──────────────────────────────────────────────────────

pub mod overview {
    use super::*;
    pub fn render(s: &Snapshot) {
        draw_header(0);
        let mut y = 40u32;
        draw_text(8, y, &alloc::format!("Uptime:    {} ms", s.uptime_ns / 1_000_000), 0xFFFFFFFF); y += 16;
        draw_text(8, y, &alloc::format!("CPU ints:  {}", s.cpu.interrupts), 0xFFCCCCCC); y += 16;
        draw_text(8, y, &alloc::format!("Timer:     {} ticks", s.cpu.timer_ticks), 0xFFCCCCCC); y += 16;
        draw_text(8, y, &alloc::format!("Faults:    PF={} GP={} NM={} DF={} UD={} MC={}",
                  s.cpu.pf, s.cpu.gp, s.cpu.nm, s.cpu.df, s.cpu.ud, s.cpu.mc), 0xFFFFFF00); y += 16;
        draw_text(8, y, &alloc::format!("Memory:    allocs={} frees={} used={}B peak={}B free_pages={}",
                  s.memory.allocs, s.memory.frees, s.memory.heap_used, s.memory.heap_peak, s.memory.free_pages),
                  0xFFCCCCCC); y += 16;
        draw_text(8, y, &alloc::format!("Sched:     ctx={} procs={} threads={}",
                  s.scheduler.ctx_switches, s.scheduler.processes, s.scheduler.threads), 0xFFCCCCCC); y += 16;
        draw_text(8, y, &alloc::format!("I/O:       pci_r={} pci_w={} serial={}B ps2={}",
                  s.io.pci_reads, s.io.pci_writes, s.io.serial_bytes, s.io.ps2_scans), 0xFFCCCCCC); y += 16;
        draw_text(8, y, &alloc::format!("Syscalls:  {} total ({} active)",
                  crate::cabina::telemetry::syscall::get_total(), s.syscalls.len()), 0xFF00FF00); y += 16;
    }
}

// ─── CPU ───────────────────────────────────────────────────────────

pub mod cpu {
    use super::*;
    pub fn render(s: &Snapshot) {
        draw_header(1);
        let mut y = 40u32;
        let c = &s.cpu;
        draw_text(8, y, &alloc::format!("Interrupts:    {}", c.interrupts), 0xFFFFFFFF); y += 16;
        draw_text(8, y, &alloc::format!("Timer ticks:   {}", c.timer_ticks), 0xFFCCCCCC); y += 16;
        draw_text(8, y, "Faults:", 0xFFCCCCCC); y += 16;
        draw_text(16, y, &alloc::format!("Page faults (PF):  {}", c.pf), 0xFFFFFF00); y += 16;
        draw_text(16, y, &alloc::format!("General (GP):     {}", c.gp), 0xFFFFFF00); y += 16;
        draw_text(16, y, &alloc::format!("NMI:              {}", c.nm), 0xFFFF8800); y += 16;
        draw_text(16, y, &alloc::format!("Double (DF):      {}", c.df), 0xFFFF0000); y += 16;
        draw_text(16, y, &alloc::format!("Invalid (UD):     {}", c.ud), 0xFFFF8800); y += 16;
        draw_text(16, y, &alloc::format!("Machine (MC):     {}", c.mc), 0xFFFF0000); y += 16;
    }
}

// ─── Memory ────────────────────────────────────────────────────────

pub mod memory {
    use super::*;
    pub fn render(s: &Snapshot) {
        draw_header(2);
        let m = &s.memory;
        draw_text(8, 40, &alloc::format!("Allocations:    {}", m.allocs), 0xFFFFFFFF);
        draw_text(8, 56, &alloc::format!("Frees:          {}", m.frees), 0xFFCCCCCC);
        draw_text(8, 72, &alloc::format!("Heap used:      {} B", m.heap_used), 0xFF00FF00);
        draw_text(8, 88, &alloc::format!("Heap peak:      {} B", m.heap_peak), 0xFFFFFF00);
        draw_text(8, 104, &alloc::format!("Free pages:     {}", m.free_pages), 0xFFCCCCCC);
        // Bar
        let total = m.heap_used + m.free_pages * 4096;
        if total > 0 {
            let pct = (m.heap_used * 100) / total;
            draw_text(8, 130, &alloc::format!("Used: {}%", pct), 0xFF00FF00);
        }
    }
}

// ─── I/O ───────────────────────────────────────────────────────────

pub mod io {
    use super::*;
    pub fn render(s: &Snapshot) {
        draw_header(3);
        let i = &s.io;
        draw_text(8, 40, &alloc::format!("PCI reads:    {}", i.pci_reads), 0xFFFFFFFF);
        draw_text(8, 56, &alloc::format!("PCI writes:   {}", i.pci_writes), 0xFFFFFFFF);
        draw_text(8, 72, &alloc::format!("Serial bytes: {} B", i.serial_bytes), 0xFF00FF00);
        draw_text(8, 88, &alloc::format!("PS/2 scancodes: {}", i.ps2_scans), 0xFFCCCCCC);
    }
}

// ─── Scheduler ─────────────────────────────────────────────────────

pub mod scheduler {
    use super::*;
    pub fn render(s: &Snapshot) {
        draw_header(4);
        let sc = &s.scheduler;
        draw_text(8, 40, &alloc::format!("Context switches: {}", sc.ctx_switches), 0xFFFFFFFF);
        draw_text(8, 56, &alloc::format!("Processes:        {}", sc.processes), 0xFFCCCCCC);
        draw_text(8, 72, &alloc::format!("Threads:          {}", sc.threads), 0xFFCCCCCC);
    }
}

// ─── Syscall ──────────────────────────────────────────────────────

pub mod syscall {
    use super::*;
    use crate::cabina::telemetry::syscall;
    pub fn render(s: &Snapshot) {
        draw_header(5);
        draw_text(8, 40, &alloc::format!("Total syscalls: {}", syscall::get_total()), 0xFFFFFFFF);
        let mut y = 60u32;
        for &(nr, count) in &s.syscalls {
            draw_text(8, y, &alloc::format!("0x{:03X} {:30} {}", nr, syscall::name(nr), count), 0xFFCCCCCC);
            y += 14;
            if y > 1000 { break; }
        }
    }
}

// ─── Log (eventos filtrados) ─────────────────────────────────────

pub mod log {
    use super::*;
    pub fn render(s: &Snapshot) {
        draw_header(6);
        // Por defecto: solo PANIC y FAULT.
        let filter = EventFilter::only_critical();
        let events = apply(&s.last_events, &filter);
        let mut y = 40u32;
        for ev in &events {
            let color = ev.severity.color();
            draw_text(8, y, &format_event_brief(ev), color);
            y += 14;
            if y > 1000 { break; }
        }
        if events.is_empty() {
            draw_text(8, 40, "(no critical events)", 0xFF888888);
        }
    }

    fn format_event_brief(ev: &crate::cabina::event::Event) -> alloc::string::String {
        let val = if ev.value != 0 { alloc::format!(" (0x{:x})", ev.value) } else { alloc::string::String::new() };
        alloc::format!("#{} [{}] {}: {}{}", ev.seq, ev.severity.name(), ev.module, ev.msg, val)
    }
}

// ─── GPU (futuro RDNA4) ───────────────────────────────────────────

pub mod gpu {
    use super::*;
    pub fn render(_s: &Snapshot) {
        draw_header(7);
        draw_text(8, 40, "GPU: RDNA4 (not yet initialized)", 0xFF888888);
        draw_text(8, 56, "Will be available after BMO GPU phase 3.", 0xFF666666);
    }
}
