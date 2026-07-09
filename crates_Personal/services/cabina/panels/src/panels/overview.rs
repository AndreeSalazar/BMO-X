use crate::fb::{self, FrameBuffer};
use crate::panels::helpers as H;
use cabina_core::SystemSnapshot;

pub fn render(fb: &mut dyn FrameBuffer, s: &SystemSnapshot) {
    H::header(fb, "OVERVIEW", 0xFF00FFAA);
    let mut y = 40u32;

    y = H::section(fb, y, "System", 0xFF00FFAA);
    y = H::kv(fb, y, "Uptime", &alloc::format!("{} ms", s.telemetry.uptime_ns / 1_000_000), 0xFF00FF00);

    y = H::section(fb, y, "CPU", 0xFF00FFAA);
    y = H::kv_u64(fb, y, "Interrupts", s.telemetry.cpu.interrupts, 0xFFFFFFFF);
    y = H::kv_u64(fb, y, "Faults PF",  s.telemetry.cpu.page_faults, 0xFFFFFF00);
    y = H::kv_u64(fb, y, "Faults GP",  s.telemetry.cpu.general_protection, 0xFFFF8800);
    y = H::kv_u64(fb, y, "Faults DF",  s.telemetry.cpu.double_fault, 0xFFFF0000);

    y = H::section(fb, y, "Memory", 0xFF00FFAA);
    y = H::kv_size(fb, y, "Heap used",  s.telemetry.memory.heap_used, 0xFF00FF00);
    y = H::kv_size(fb, y, "Heap peak",  s.telemetry.memory.heap_peak, 0xFFFFFF00);
    y = H::kv_u64(fb, y, "Allocs",      s.telemetry.memory.allocations, 0xFFFFFFFF);
    y = H::kv_u64(fb, y, "Live",        s.telemetry.memory.allocations.saturating_sub(s.telemetry.memory.frees), 0xFF00FFFF);
    y = H::kv_u64(fb, y, "Free pages",  s.telemetry.memory.free_pages, 0xFFCCCCCC);

    y = H::section(fb, y, "Scheduler", 0xFF00FFAA);
    y = H::kv_u64(fb, y, "Ctx switches", s.telemetry.scheduler.context_switches, 0xFF00FF00);
    y = H::kv_u64(fb, y, "Processes",    s.telemetry.scheduler.processes, 0xFFFFFFFF);
    y = H::kv_u64(fb, y, "Threads",      s.telemetry.scheduler.threads, 0xFFCCCCCC);

    y = H::section(fb, y, "I/O", 0xFF00FFAA);
    y = H::kv_u64(fb, y, "Serial bytes", s.telemetry.io.serial_bytes, 0xFF00FF00);
    y = H::kv_u64(fb, y, "PCI reads",    s.telemetry.io.pci_reads, 0xFFCCCCCC);
    y = H::kv_u64(fb, y, "PCI writes",   s.telemetry.io.pci_writes, 0xFFCCCCCC);

    y = H::section(fb, y, "Events", 0xFF00FFAA);
    let count = s.event_count.min(32) as usize;
    let mut i = 0;
    for ev in s.events[..count].iter() {
        let line = if ev.value != 0 {
            alloc::format!("#{} {} {}: {} (0x{:x})", ev.seq, ev.severity.name(), ev.module_str(), ev.msg_str(), ev.value)
        } else {
            alloc::format!("#{} {} {}: {}", ev.seq, ev.severity.name(), ev.module_str(), ev.msg_str())
        };
        fb::draw_text(fb, 16, y, &line, ev.severity.color());
        y += 14;
        i += 1;
    }
    if i == 0 {
        fb::draw_text(fb, 16, y, "(no events)", 0xFF888888);
    }
}
