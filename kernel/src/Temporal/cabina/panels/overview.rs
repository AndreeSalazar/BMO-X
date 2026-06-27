//! `cabina::panels::overview` — Panel Overview (resumen general).

#![allow(dead_code)]

use crate::cabina::snapshot::Snapshot;
use crate::cabina::panels::helpers as H;
use crate::cabina::telemetry;
use crate::cabina::paint;

pub fn render(s: &Snapshot) {
    H::header("OVERVIEW", 0xFF00FFAA);

    let mut y = 40u32;
    y = H::section(y, "System", 0xFF00FFAA);
    y = H::kv(y, "Uptime",     &alloc::format!("{} ms", s.uptime_ns / 1_000_000), 0xFF00FF00);
    y = H::kv(y, "Boot phase", "Welcome", 0xFFCCCCCC);
    let ready = crate::cabina::is_ready();
    y = H::kv(y, "Ready",     if ready { "yes" } else { "no" },
                 if ready { 0xFF00FF00 } else { 0xFFFF8800 });

    y = H::section(y, "CPU", 0xFF00FFAA);
    y = H::kv_u64(y, "Interrupts", s.cpu.interrupts, 0xFFFFFFFF);
    y = H::kv_u64(y, "Faults PF",  s.cpu.pf,  0xFFFFFF00);
    y = H::kv_u64(y, "Faults GP",  s.cpu.gp,  0xFFFF8800);
    y = H::kv_u64(y, "Faults DF",  s.cpu.df,  0xFFFF0000);

    y = H::section(y, "Memory", 0xFF00FFAA);
    y = H::kv_size(y, "Heap used",  s.memory.heap_used, 0xFF00FF00);
    y = H::kv_size(y, "Heap peak",  s.memory.heap_peak, 0xFFFFFF00);
    y = H::kv_u64 (y, "Allocs",     s.memory.allocs, 0xFFFFFFFF);
    y = H::kv_u64 (y, "Live",       s.memory.allocs.saturating_sub(s.memory.frees), 0xFF00FFFF);
    y = H::kv_u64 (y, "Free pages", s.memory.free_pages, 0xFFCCCCCC);

    y = H::section(y, "Scheduler", 0xFF00FFAA);
    y = H::kv_u64(y, "Ctx switches", s.scheduler.ctx_switches, 0xFF00FF00);
    y = H::kv_u64(y, "Processes",    s.scheduler.processes, 0xFFFFFFFF);
    y = H::kv_u64(y, "Threads",      s.scheduler.threads, 0xFFCCCCCC);

    y = H::section(y, "I/O", 0xFF00FFAA);
    y = H::kv_u64(y, "Serial bytes",  s.io.serial_bytes, 0xFF00FF00);
    y = H::kv_u64(y, "PCI reads",     s.io.pci_reads,    0xFFCCCCCC);
    y = H::kv_u64(y, "PCI writes",    s.io.pci_writes,   0xFFCCCCCC);
    y = H::kv_u64(y, "PS/2 scancodes", s.io.ps2_scans,     0xFFCCCCCC);

    y = H::section(y, "Syscalls", 0xFF00FFAA);
    y = H::kv_u64(y, "Total",         telemetry::syscall::get_total(), 0xFF00FF00);
    y = H::kv_u64(y, "Unique active", s.syscalls.len() as u64, 0xFFFFFFFF);

    y = H::section(y, "Last events", 0xFF00FFAA);
    let mut i = 0;
    for ev in s.last_events.iter().take(8) {
        let line = if ev.value != 0 {
            alloc::format!("#{} {} {}: {} (0x{:x})", ev.seq, ev.severity.name(), ev.module, ev.msg, ev.value)
        } else {
            alloc::format!("#{} {} {}: {}", ev.seq, ev.severity.name(), ev.module, ev.msg)
        };
        paint::draw_text(16, y, &line, ev.severity.color());
        y += 14;
        i += 1;
    }
    if i == 0 {
        paint::draw_text(16, y, "(no events)", 0xFF888888);
    }
    let _ = y; // suppress
}
