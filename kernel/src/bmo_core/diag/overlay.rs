//! Overlay visual de diag/ sobre framebuffer GOP — pestañas omniscientes.
//!
//! Regla importante: este HUD corre dentro del camino de diagnóstico y puede
//! pintarse durante boot, panic o render del escritorio. Por eso no usa
//! `alloc::format!`, `String` ni helpers que crezcan el heap.

use super::buffer;
use super::event::{severity_color, severity_tag, Event};
use super::telemetry;
use super::OverlayTab;
use crate::bmo_core::ui::font;
use core::sync::atomic::{AtomicBool, AtomicU32, Ordering};

const MIN_W: usize = 360;
const MIN_H: usize = 180;
const MAX_W: usize = 480;
const OVERLAY_H: usize = 190;
const CHAR_W: usize = 8;

static ENABLED: AtomicBool = AtomicBool::new(false);

/// Optional render target components: (base_ptr, width, height, stride).
static TARGET_BASE: core::sync::atomic::AtomicU64 = core::sync::atomic::AtomicU64::new(0);
static TARGET_W: AtomicU32 = AtomicU32::new(0);
static TARGET_H: AtomicU32 = AtomicU32::new(0);
static TARGET_S: AtomicU32 = AtomicU32::new(0);
static HAS_TARGET: AtomicBool = AtomicBool::new(false);

/// Set a temporary render target for the overlay.
pub fn set_target_override(target: Option<(*mut u32, usize, usize, usize)>) {
    match target {
        Some((base, w, h, s)) => {
            TARGET_BASE.store(base as u64, Ordering::Relaxed);
            TARGET_W.store(w as u32, Ordering::Relaxed);
            TARGET_H.store(h as u32, Ordering::Relaxed);
            TARGET_S.store(s as u32, Ordering::Relaxed);
            HAS_TARGET.store(true, Ordering::Relaxed);
        }
        None => {
            HAS_TARGET.store(false, Ordering::Relaxed);
        }
    }
}

pub fn set_enabled(enabled: bool) {
    ENABLED.store(enabled, Ordering::Relaxed);
}

pub fn is_enabled() -> bool {
    ENABLED.load(Ordering::Relaxed)
}

pub fn paint() {
    if !ENABLED.load(Ordering::Relaxed) { return; }

    let Some((base, width, height, stride)) = fb() else { return; };
    if width < MIN_W || height < MIN_H { return; }

    // v1.6.8: anchor overlay to the top-right corner of the screen.
    let w = MAX_W.min(width.saturating_sub(32)).max(MIN_W.min(width));
    let h = OVERLAY_H.min(height.saturating_sub(80));
    let x = width.saturating_sub(w + 16);
    let y = 64usize.min(height.saturating_sub(h + 1));
    if w < MIN_W || h < MIN_H { return; }

    // Background
    fill_rect(base, stride, width, height, x, y, w, h, 0xFF0B1224);
    draw_rect(base, stride, width, height, x, y, w, h, 0xFF56D4DD);
    if w > 4 && h > 4 {
        draw_rect(base, stride, width, height, x + 1, y + 1, w - 2, h - 2, 0xFF14344A);
    }

    // Tab bar
    draw_tab_bar(base, stride, width, height, x + 12, y + 6, x + w - 12);

    // Divider below tabs
    fill_rect(base, stride, width, height, x, y + 26, w, 1, 0xFF56D4DD);

    // Content area
    let tab = super::current_tab();
    let cx = x + 12;
    let cy = y + 32;
    let right = x + w - 12;

    match tab {
        OverlayTab::Overview => draw_overview(base, stride, width, height, cx, cy, right, w, h),
        OverlayTab::Cpu => draw_cpu_tab(base, stride, width, height, cx, cy, right, w, h),
        OverlayTab::Memory => draw_memory_tab(base, stride, width, height, cx, cy, right, w, h),
        OverlayTab::Io => draw_io_tab(base, stride, width, height, cx, cy, right, w, h),
        OverlayTab::Scheduler => draw_scheduler_tab(base, stride, width, height, cx, cy, right, w, h),
        OverlayTab::Log => draw_log_tab(base, stride, width, height, cx, cy, right, w, h),
    }

    // Footer
    let footer_y = y + h - 18;
    fill_rect(base, stride, width, height, x, footer_y - 2, w, 1, 0xFF14344A);
    draw_text(base, stride, width, height, cx, footer_y, b"Tab: F9  |  Ctrl+Alt: ocultar", 0xFF8B949E);
}

// ── Tab bar ────────────────────────────────────────────────────────

fn draw_tab_bar(
    base: *mut u32, stride: usize, width: usize, height: usize,
    x: usize, y: usize, right: usize,
) {
    let tabs: &[(OverlayTab, &[u8])] = &[
        (OverlayTab::Overview, b"Overview"),
        (OverlayTab::Cpu,      b"CPU"),
        (OverlayTab::Memory,   b"Memory"),
        (OverlayTab::Io,       b"I/O"),
        (OverlayTab::Scheduler,b"Sched"),
        (OverlayTab::Log,      b"Log"),
    ];

    let current = super::current_tab();
    let mut cx = x;

    for (tab, label) in tabs {
        let is_active = *tab == current;
        let bg = if is_active { 0xFF56D4DD } else { 0xFF0B1224 };
        let fg = if is_active { 0xFF0B1224 } else { 0xFF8B949E };
        let tab_w = label.len() * CHAR_W + 8;

        // Tab background
        fill_rect(base, stride, width, height, cx, y - 2, tab_w, 16, bg);
        draw_text(base, stride, width, height, cx + 4, y, label, fg);

        cx += tab_w + 4;
        if cx > right { break; }
    }
}

// ── Overview tab ───────────────────────────────────────────────────

fn draw_overview(
    base: *mut u32, stride: usize, width: usize, height: usize,
    x: usize, y: usize, right: usize, _w: usize, _h: usize,
) {
    let t = telemetry::t();
    let mut cy = y;

    // Left: system info
    draw_text(base, stride, width, height, x, cy, b"CPU    : Ryzen 5 5600X (Zen 3)", 0xFFE6EDF3);
    cy += 18;
    draw_text(base, stride, width, height, x, cy, b"Exts   : SSE | AVX | AVX2 | FMA3 | AES", 0xFF56D4DD);
    cy += 18;

    // Ring 3 status — dynamically check if any syscall has been received
    let ring3_active = crate::arch::syscall::ring3_alive();
    if ring3_active {
        draw_text(base, stride, width, height, x, cy, b"Ring   : 0 Supervisor | Ring3 active", 0xFF76B900);
    } else {
        draw_text(base, stride, width, height, x, cy, b"Ring   : 0 Supervisor | Ring3 pending", 0xFFE6EDF3);
    }
    cy += 18;

    // Uptime
    let uptime = crate::bmo_core::desktop::state::uptime_sec();
    draw_text(base, stride, width, height, x, cy, b"Uptime : ", 0xFFE6EDF3);
    draw_two_digits(base, stride, width, height, x + 72, cy, uptime / 3600, 0xFFE6EDF3);
    draw_text(base, stride, width, height, x + 88, cy, b":", 0xFFE6EDF3);
    draw_two_digits(base, stride, width, height, x + 96, cy, (uptime % 3600) / 60, 0xFFE6EDF3);
    draw_text(base, stride, width, height, x + 112, cy, b":", 0xFFE6EDF3);
    draw_two_digits(base, stride, width, height, x + 120, cy, uptime % 60, 0xFFE6EDF3);
    cy += 18;

    // Memory
    let free_pages = unsafe { crate::mem::phys::free_count() };
    let free_mb = (free_pages * 4) / 1024;
    draw_text(base, stride, width, height, x, cy, b"Memory : ", 0xFFE6EDF3);
    draw_dec(base, stride, width, height, x + 72, cy, free_mb as u64, 0xFF76B900);
    draw_text(base, stride, width, height, x + 128, cy, b" MB free", 0xFFE6EDF3);
    cy += 18;

    // Heap
    let heap_used_kb = crate::mem::heap::heap_used() / 1024;
    let heap_total_kb = crate::mem::heap::heap_total() / 1024;
    let heap_color = if heap_used_kb > heap_total_kb * 3 / 4 { 0xFFFF7B72 } else { 0xFF76B900 };
    draw_text(base, stride, width, height, x, cy, b"Heap   : ", 0xFFE6EDF3);
    draw_dec(base, stride, width, height, x + 72, cy, heap_used_kb as u64, heap_color);
    draw_text(base, stride, width, height, x + 128, cy, b" / ", 0xFFE6EDF3);
    draw_dec(base, stride, width, height, x + 160, cy, heap_total_kb as u64, 0xFFE6EDF3);
    draw_text(base, stride, width, height, x + 208, cy, b" KB", 0xFFE6EDF3);
    cy += 18;

    // Tasks
    draw_text(base, stride, width, height, x, cy, b"Tasks  : P", 0xFFE6EDF3);
    draw_dec(base, stride, width, height, x + 88, cy, crate::proc::process::process_count() as u64, 0xFFE6EDF3);
    draw_text(base, stride, width, height, x + 120, cy, b" T", 0xFFE6EDF3);
    draw_dec(base, stride, width, height, x + 144, cy, crate::proc::task::ready_count() as u64, 0xFFE6EDF3);
    draw_text(base, stride, width, height, x + 176, cy, b" Ctx:", 0xFF8B949E);
    draw_dec(base, stride, width, height, x + 216, cy, t.sched.context_switches.load(core::sync::atomic::Ordering::Relaxed), 0xFF8B949E);
    cy += 18;

    // GOP
    draw_text(base, stride, width, height, x, cy, b"GOP    : ", 0xFFE6EDF3);
    draw_dec(base, stride, width, height, x + 72, cy, width as u64, 0xFFE6EDF3);
    draw_text(base, stride, width, height, x + 112, cy, b"x", 0xFFE6EDF3);
    draw_dec(base, stride, width, height, x + 120, cy, height as u64, 0xFFE6EDF3);
    draw_text(base, stride, width, height, x + 168, cy, b" stride ", 0xFF8B949E);
    draw_dec(base, stride, width, height, x + 232, cy, stride as u64, 0xFF8B949E);

    // Right side: quick status
    if right > x + 400 {
        let rx = x + 400;
        draw_text(base, stride, width, height, rx, y, b"Live Status", 0xFF58A6FF);
        let mut ry = y + 18;

        draw_text(base, stride, width, height, rx, ry, b"PCI    : ", 0xFFE6EDF3);
        draw_dec(base, stride, width, height, rx + 72, ry, crate::dev::pcie::device_count() as u64, 0xFFE6EDF3);
        draw_text(base, stride, width, height, rx + 104, ry, b" devices", 0xFFE6EDF3);
        ry += 18;
        draw_bool_row(base, stride, width, height, rx, ry, b"NVMe   : ", crate::dev::pcie::has_nvme());
        ry += 18;
        draw_bool_row(base, stride, width, height, rx, ry, b"AHCI   : ", crate::dev::pcie::has_ahci());
        ry += 18;
        draw_bool_row(base, stride, width, height, rx, ry, b"xHCI   : ", crate::dev::pcie::has_xhci());
        ry += 18;
        draw_text(base, stride, width, height, rx, ry, b"Interrupts: ", 0xFFE6EDF3);
        draw_dec(base, stride, width, height, rx + 96, ry, t.cpu.interrupts.load(core::sync::atomic::Ordering::Relaxed), 0xFF76B900);
        ry += 18;
        draw_text(base, stride, width, height, rx, ry, b"Page Flts: ", 0xFFE6EDF3);
        draw_dec(base, stride, width, height, rx + 96, ry, t.cpu.page_faults.load(core::sync::atomic::Ordering::Relaxed), 0xFFFFBD2E);
        ry += 18;
        draw_text(base, stride, width, height, rx, ry, b"USB Log : ", 0xFFE6EDF3);
        draw_dec(base, stride, width, height, rx + 96, ry, super::persistent_pending_bytes() as u64, 0xFF76B900);
        draw_text(base, stride, width, height, rx + 152, ry, b" B pending", 0xFF8B949E);
    }
}

// ── CPU tab ────────────────────────────────────────────────────────

fn draw_cpu_tab(
    base: *mut u32, stride: usize, width: usize, height: usize,
    x: usize, y: usize, _right: usize, _w: usize, _h: usize,
) {
    let t = telemetry::t();
    let mut cy = y;

    draw_text(base, stride, width, height, x, cy, b"CPU Telemetry", 0xFF58A6FF);
    cy += 22;

    draw_text(base, stride, width, height, x, cy, b"Interrupts   : ", 0xFFE6EDF3);
    draw_dec(base, stride, width, height, x + 120, cy, t.cpu.interrupts.load(core::sync::atomic::Ordering::Relaxed), 0xFF76B900);
    cy += 18;

    draw_text(base, stride, width, height, x, cy, b"Timer Ticks  : ", 0xFFE6EDF3);
    draw_dec(base, stride, width, height, x + 120, cy, t.cpu.timer_ticks.load(core::sync::atomic::Ordering::Relaxed), 0xFF76B900);
    cy += 18;

    draw_text(base, stride, width, height, x, cy, b"Page Faults  : ", 0xFFE6EDF3);
    draw_dec(base, stride, width, height, x + 120, cy, t.cpu.page_faults.load(core::sync::atomic::Ordering::Relaxed), 0xFFFFBD2E);
    cy += 18;

    draw_text(base, stride, width, height, x, cy, b"#GP Faults   : ", 0xFFE6EDF3);
    draw_dec(base, stride, width, height, x + 120, cy, t.cpu.gp_faults.load(core::sync::atomic::Ordering::Relaxed), 0xFFFF7B72);
    cy += 18;

    draw_text(base, stride, width, height, x, cy, b"#NM Faults   : ", 0xFFE6EDF3);
    draw_dec(base, stride, width, height, x + 120, cy, t.cpu.nm_faults.load(core::sync::atomic::Ordering::Relaxed), 0xFFFFBD2E);
    cy += 18;

    draw_text(base, stride, width, height, x, cy, b"#DF Faults   : ", 0xFFE6EDF3);
    draw_dec(base, stride, width, height, x + 120, cy, t.cpu.df_faults.load(core::sync::atomic::Ordering::Relaxed), 0xFFFF7B72);
    cy += 18;

    draw_text(base, stride, width, height, x, cy, b"#UD Faults   : ", 0xFFE6EDF3);
    draw_dec(base, stride, width, height, x + 120, cy, t.cpu.ud_faults.load(core::sync::atomic::Ordering::Relaxed), 0xFFFFBD2E);
    cy += 18;

    draw_text(base, stride, width, height, x, cy, b"#MC Faults   : ", 0xFFE6EDF3);
    draw_dec(base, stride, width, height, x + 120, cy, t.cpu.mc_faults.load(core::sync::atomic::Ordering::Relaxed), 0xFFFF7B72);
    cy += 18;

    draw_text(base, stride, width, height, x, cy, b"Other Faults : ", 0xFFE6EDF3);
    draw_dec(base, stride, width, height, x + 120, cy, t.cpu.other_faults.load(core::sync::atomic::Ordering::Relaxed), 0xFF8B949E);
    cy += 18;

    // TSC frequency estimate
    draw_text(base, stride, width, height, x, cy, b"Est. TSC freq: ~3.7 GHz (Zen 3)", 0xFF8B949E);
}

// ── Memory tab ─────────────────────────────────────────────────────

fn draw_memory_tab(
    base: *mut u32, stride: usize, width: usize, height: usize,
    x: usize, y: usize, _right: usize, _w: usize, _h: usize,
) {
    let t = telemetry::t();
    let mut cy = y;

    draw_text(base, stride, width, height, x, cy, b"Memory Telemetry", 0xFF58A6FF);
    cy += 22;

    let free_pages = unsafe { crate::mem::phys::free_count() };
    let total_pages = crate::mem::phys::total_pages();
    let used_pages = total_pages - free_pages;
    let free_mb = (free_pages * 4) / 1024;
    let total_mb = (total_pages * 4) / 1024;

    draw_text(base, stride, width, height, x, cy, b"Free Pages   : ", 0xFFE6EDF3);
    draw_dec(base, stride, width, height, x + 120, cy, free_pages as u64, 0xFF76B900);
    draw_text(base, stride, width, height, x + 180, cy, b"(", 0xFF8B949E);
    draw_dec(base, stride, width, height, x + 188, cy, free_mb as u64, 0xFF76B900);
    draw_text(base, stride, width, height, x + 236, cy, b"MB)", 0xFF8B949E);
    cy += 18;

    draw_text(base, stride, width, height, x, cy, b"Used Pages   : ", 0xFFE6EDF3);
    draw_dec(base, stride, width, height, x + 120, cy, used_pages as u64, 0xFFFFBD2E);
    draw_text(base, stride, width, height, x + 180, cy, b"(", 0xFF8B949E);
    draw_dec(base, stride, width, height, x + 188, cy, ((used_pages * 4) / 1024) as u64, 0xFFFFBD2E);
    draw_text(base, stride, width, height, x + 236, cy, b"MB)", 0xFF8B949E);
    cy += 18;

    draw_text(base, stride, width, height, x, cy, b"Total Pages  : ", 0xFFE6EDF3);
    draw_dec(base, stride, width, height, x + 120, cy, total_pages as u64, 0xFFE6EDF3);
    draw_text(base, stride, width, height, x + 180, cy, b"(", 0xFF8B949E);
    draw_dec(base, stride, width, height, x + 188, cy, total_mb as u64, 0xFFE6EDF3);
    draw_text(base, stride, width, height, x + 236, cy, b"MB)", 0xFF8B949E);
    cy += 18;

    // Usage bar
    let bar_w = 200;
    let bar_x = x + 120;
    fill_rect(base, stride, width, height, bar_x, cy, bar_w, 12, 0xFF1A1A2E);
    let used_w = if total_pages > 0 { (used_pages * bar_w) / total_pages } else { 0 };
    let bar_color = if used_w > bar_w * 3 / 4 { 0xFFFF7B72 } else { 0xFF76B900 };
    fill_rect(base, stride, width, height, bar_x, cy, used_w, 12, bar_color);
    draw_rect(base, stride, width, height, bar_x, cy, bar_w, 12, 0xFF56D4DD);
    cy += 20;

    let heap_used_kb = crate::mem::heap::heap_used() / 1024;
    let heap_total_kb = crate::mem::heap::heap_total() / 1024;
    draw_text(base, stride, width, height, x, cy, b"Heap Used    : ", 0xFFE6EDF3);
    draw_dec(base, stride, width, height, x + 120, cy, heap_used_kb as u64, 0xFF76B900);
    draw_text(base, stride, width, height, x + 180, cy, b"KB / ", 0xFF8B949E);
    draw_dec(base, stride, width, height, x + 216, cy, heap_total_kb as u64, 0xFFE6EDF3);
    draw_text(base, stride, width, height, x + 264, cy, b"KB", 0xFF8B949E);
    cy += 18;

    draw_text(base, stride, width, height, x, cy, b"Heap Peak    : ", 0xFFE6EDF3);
    draw_dec(base, stride, width, height, x + 120, cy, t.mem.heap_peak.load(core::sync::atomic::Ordering::Relaxed) / 1024, 0xFFFFBD2E);
    draw_text(base, stride, width, height, x + 180, cy, b"KB", 0xFF8B949E);
    cy += 18;

    draw_text(base, stride, width, height, x, cy, b"Alloc Calls  : ", 0xFFE6EDF3);
    draw_dec(base, stride, width, height, x + 120, cy, t.mem.allocs.load(core::sync::atomic::Ordering::Relaxed), 0xFF76B900);
    cy += 18;

    draw_text(base, stride, width, height, x, cy, b"Free Calls   : ", 0xFFE6EDF3);
    draw_dec(base, stride, width, height, x + 120, cy, t.mem.frees.load(core::sync::atomic::Ordering::Relaxed), 0xFF76B900);
}

// ── I/O tab ────────────────────────────────────────────────────────

fn draw_io_tab(
    base: *mut u32, stride: usize, width: usize, height: usize,
    x: usize, y: usize, _right: usize, _w: usize, _h: usize,
) {
    let t = telemetry::t();
    let mut cy = y;

    draw_text(base, stride, width, height, x, cy, b"I/O Telemetry", 0xFF58A6FF);
    cy += 22;

    draw_text(base, stride, width, height, x, cy, b"PCI Devices  : ", 0xFFE6EDF3);
    draw_dec(base, stride, width, height, x + 120, cy, crate::dev::pcie::device_count() as u64, 0xFFE6EDF3);
    cy += 18;

    draw_text(base, stride, width, height, x, cy, b"PCI Reads    : ", 0xFFE6EDF3);
    draw_dec(base, stride, width, height, x + 120, cy, t.io.pci_reads.load(core::sync::atomic::Ordering::Relaxed), 0xFF76B900);
    cy += 18;

    draw_text(base, stride, width, height, x, cy, b"PCI Writes   : ", 0xFFE6EDF3);
    draw_dec(base, stride, width, height, x + 120, cy, t.io.pci_writes.load(core::sync::atomic::Ordering::Relaxed), 0xFF76B900);
    cy += 18;

    draw_text(base, stride, width, height, x, cy, b"Serial Bytes : ", 0xFFE6EDF3);
    draw_dec(base, stride, width, height, x + 120, cy, t.io.serial_bytes.load(core::sync::atomic::Ordering::Relaxed), 0xFF76B900);
    cy += 18;

    draw_text(base, stride, width, height, x, cy, b"PS/2 Codes   : ", 0xFFE6EDF3);
    draw_dec(base, stride, width, height, x + 120, cy, t.io.ps2_scancodes.load(core::sync::atomic::Ordering::Relaxed), 0xFF76B900);
    cy += 22;

    draw_bool_row(base, stride, width, height, x, cy, b"NVMe   : ", crate::dev::pcie::has_nvme());
    cy += 18;
    draw_bool_row(base, stride, width, height, x, cy, b"AHCI   : ", crate::dev::pcie::has_ahci());
    cy += 18;
    draw_bool_row(base, stride, width, height, x, cy, b"xHCI   : ", crate::dev::pcie::has_xhci());
}

// ── Scheduler tab ──────────────────────────────────────────────────

fn draw_scheduler_tab(
    base: *mut u32, stride: usize, width: usize, height: usize,
    x: usize, y: usize, _right: usize, _w: usize, _h: usize,
) {
    let t = telemetry::t();
    let mut cy = y;

    draw_text(base, stride, width, height, x, cy, b"Scheduler Telemetry", 0xFF58A6FF);
    cy += 22;

    draw_text(base, stride, width, height, x, cy, b"Context Switches: ", 0xFFE6EDF3);
    draw_dec(base, stride, width, height, x + 144, cy, t.sched.context_switches.load(core::sync::atomic::Ordering::Relaxed), 0xFF76B900);
    cy += 18;

    draw_text(base, stride, width, height, x, cy, b"Processes Created: ", 0xFFE6EDF3);
    draw_dec(base, stride, width, height, x + 144, cy, t.sched.processes_created.load(core::sync::atomic::Ordering::Relaxed), 0xFF76B900);
    cy += 18;

    draw_text(base, stride, width, height, x, cy, b"Threads Created  : ", 0xFFE6EDF3);
    draw_dec(base, stride, width, height, x + 144, cy, t.sched.threads_created.load(core::sync::atomic::Ordering::Relaxed), 0xFF76B900);
    cy += 18;

    draw_text(base, stride, width, height, x, cy, b"Active Processes : ", 0xFFE6EDF3);
    draw_dec(base, stride, width, height, x + 144, cy, crate::proc::process::process_count() as u64, 0xFFE6EDF3);
    cy += 18;

    draw_text(base, stride, width, height, x, cy, b"Ready Threads    : ", 0xFFE6EDF3);
    draw_dec(base, stride, width, height, x + 144, cy, crate::proc::task::ready_count() as u64, 0xFFE6EDF3);
    cy += 22;

    draw_text(base, stride, width, height, x, cy, b"Syscalls Total   : ", 0xFFE6EDF3);
    draw_dec(base, stride, width, height, x + 144, cy, t.syscall.total.load(core::sync::atomic::Ordering::Relaxed), 0xFF76B900);
}

// ── Log tab ────────────────────────────────────────────────────────

fn draw_log_tab(
    base: *mut u32, stride: usize, width: usize, height: usize,
    x: usize, y: usize, right: usize, _w: usize, h: usize,
) {
    draw_text(base, stride, width, height, x, y, b"Event Log (256 circular buffer)", 0xFF58A6FF);

    let log_lines = (h - 52) / 18; // Fill available space
    let next = buffer::next_seq();
    let first = next.saturating_sub(log_lines as u64);
    let mut seq = first;
    let mut cy = y + 22;
    while seq < next {
        if let Some(ev) = buffer::event_by_seq(seq) {
            draw_event_line(base, stride, width, height, x, cy, right, ev);
            cy += 18;
        }
        seq += 1;
    }
}

// ── Shared helpers ─────────────────────────────────────────────────

fn draw_bool_row(
    base: *mut u32, stride: usize, width: usize, height: usize,
    x: usize, y: usize, label: &[u8], value: bool,
) {
    draw_text(base, stride, width, height, x, y, label, 0xFFE6EDF3);
    if value {
        draw_text(base, stride, width, height, x + 72, y, b"detectado", 0xFF76B900);
    } else {
        draw_text(base, stride, width, height, x + 72, y, b"no activo", 0xFF8B949E);
    }
}

fn draw_event_line(
    base: *mut u32, stride: usize, width: usize, height: usize,
    x: usize, y: usize, right: usize, ev: Event,
) {
    let color = severity_color(ev.severity);
    draw_text(base, stride, width, height, x, y, b"[", 0xFFE6EDF3);
    draw_text(base, stride, width, height, x + 8, y, severity_tag(ev.severity), color);
    draw_text(base, stride, width, height, x + 48, y, b"] ", 0xFFE6EDF3);
    draw_text_clipped(base, stride, width, height, x + 64, y, ev.module.as_bytes(), 0xFF76B900, 8);
    draw_text(base, stride, width, height, x + 128, y, b": ", 0xFFE6EDF3);
    let msg_x = x + 144;
    let value_cols = if ev.has_value { 19 } else { 0 };
    let max_cols = right.saturating_sub(msg_x).saturating_div(CHAR_W).saturating_sub(value_cols);
    draw_text_clipped(base, stride, width, height, msg_x, y, ev.message.as_bytes(), 0xFFE6EDF3, max_cols);

    if ev.has_value && right > 18 * CHAR_W {
        let vx = right.saturating_sub(18 * CHAR_W);
        draw_text(base, stride, width, height, vx, y, b"0x", 0xFF8B949E);
        draw_hex(base, stride, width, height, vx + 16, y, ev.value, 0xFF8B949E);
    }
}

// ── Drawing primitives ─────────────────────────────────────────────

fn fb() -> Option<(*mut u32, usize, usize, usize)> {
    if HAS_TARGET.load(Ordering::Relaxed) {
        let base = TARGET_BASE.load(Ordering::Relaxed) as *mut u32;
        let w = TARGET_W.load(Ordering::Relaxed) as usize;
        let h = TARGET_H.load(Ordering::Relaxed) as usize;
        let s = TARGET_S.load(Ordering::Relaxed) as usize;
        if base.is_null() || w == 0 || h == 0 || s == 0 { return None; }
        return Some((base, w, h, s));
    }
    let (addr, w, h, s) = unsafe {
        (
            crate::boot::info::FB_ADDR,
            crate::boot::info::FB_WIDTH as usize,
            crate::boot::info::FB_HEIGHT as usize,
            crate::boot::info::FB_STRIDE as usize,
        )
    };
    if addr == 0 || w == 0 || h == 0 || s == 0 { return None; }
    Some((addr as *mut u32, w, h, s))
}

fn fill_rect(
    base: *mut u32, stride: usize, width: usize, height: usize,
    x: usize, y: usize, w: usize, h: usize, color: u32,
) {
    let x1 = x.saturating_add(w).min(width);
    let y1 = y.saturating_add(h).min(height);
    for yy in y..y1 {
        for xx in x..x1 {
            unsafe { base.add(yy * stride + xx).write_volatile(color); }
        }
    }
}

fn draw_rect(
    base: *mut u32, stride: usize, width: usize, height: usize,
    x: usize, y: usize, w: usize, h: usize, color: u32,
) {
    if w == 0 || h == 0 { return; }
    fill_rect(base, stride, width, height, x, y, w, 1, color);
    fill_rect(base, stride, width, height, x, y + h.saturating_sub(1), w, 1, color);
    fill_rect(base, stride, width, height, x, y, 1, h, color);
    fill_rect(base, stride, width, height, x + w.saturating_sub(1), y, 1, h, color);
}

fn draw_text(
    base: *mut u32, stride: usize, width: usize, height: usize,
    x: usize, y: usize, text: &[u8], color: u32,
) {
    let mut cx = x;
    for &ch in text {
        if cx + CHAR_W > width || y + 16 > height { break; }
        let glyph = font::get_glyph(ch);
        for gy in 0..16 {
            let row = glyph[gy];
            for gx in 0..8 {
                if (row & (0x80 >> gx)) != 0 {
                    unsafe { base.add((y + gy) * stride + cx + gx).write_volatile(color); }
                }
            }
        }
        cx += CHAR_W;
    }
}

fn draw_text_right(
    base: *mut u32, stride: usize, width: usize, height: usize,
    right: usize, y: usize, text: &[u8], color: u32,
) {
    let px = text.len().saturating_mul(CHAR_W);
    draw_text(base, stride, width, height, right.saturating_sub(px), y, text, color);
}

fn draw_text_clipped(
    base: *mut u32, stride: usize, width: usize, height: usize,
    x: usize, y: usize, text: &[u8], color: u32, max_cols: usize,
) {
    let cols = text.len().min(max_cols);
    if cols == 0 { return; }
    draw_text(base, stride, width, height, x, y, &text[..cols], color);
}

fn draw_dec(
    base: *mut u32, stride: usize, width: usize, height: usize,
    x: usize, y: usize, mut value: u64, color: u32,
) {
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
    draw_text(base, stride, width, height, x, y, &buf[i..], color);
}

fn draw_two_digits(
    base: *mut u32, stride: usize, width: usize, height: usize,
    x: usize, y: usize, value: u64, color: u32,
) {
    let v = value % 100;
    let buf = [b'0' + (v / 10) as u8, b'0' + (v % 10) as u8];
    draw_text(base, stride, width, height, x, y, &buf, color);
}

fn draw_hex(
    base: *mut u32, stride: usize, width: usize, height: usize,
    x: usize, y: usize, value: u64, color: u32,
) {
    let hex = b"0123456789ABCDEF";
    let mut buf = [0u8; 16];
    for (i, item) in buf.iter_mut().enumerate() {
        let shift = (15 - i) * 4;
        *item = hex[((value >> shift) & 0xF) as usize];
    }
    draw_text(base, stride, width, height, x, y, &buf, color);
}
