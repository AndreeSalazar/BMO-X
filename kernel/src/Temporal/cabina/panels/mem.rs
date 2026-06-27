//! `cabina::panels::mem` — Panel de memoria con detalle granular.

#![allow(dead_code)]

use crate::cabina::snapshot::Snapshot;
use crate::cabina::panels::helpers as H;
use crate::cabina::paint;

pub fn render(s: &Snapshot) {
    H::header("MEM", 0xFFAAFF00);

    let mut y = 40u32;
    let m = &s.memory;

    y = H::section(y, "Heap", 0xFFAAFF00);
    y = H::kv_size(y, "Used",      m.heap_used, 0xFF00FF00);
    y = H::kv_size(y, "Peak",      m.heap_peak, 0xFFFFFF00);
    y = H::kv_u64 (y, "Allocs",    m.allocs, 0xFFFFFFFF);
    y = H::kv_u64 (y, "Frees",     m.frees,  0xFFCCCCCC);
    y = H::kv_u64 (y, "Live",      m.allocs.saturating_sub(m.frees), 0xFF00FFFF);
    y = H::kv     (y, "Fragment.", "0% (bump allocator v1.8.8)", 0xFF888888);

    y = H::section(y, "Pages", 0xFFAAFF00);
    y = H::kv_u64 (y, "Free",     m.free_pages, 0xFF00FF00);
    y = H::kv_size(y, "Free KB",  m.free_pages * 4, 0xFFCCCCCC);
    y = H::kv     (y, "Used",  "(v1.9: total - free)", 0xFF888888);

    y = H::section(y, "Alloc buckets (v1.9)", 0xFFAAFF00);
    let buckets = [("8 B", 0u64), ("16 B", 0), ("32 B", 0), ("64 B", 0), ("128 B", 0),
                    ("256 B", 0), ("512 B", 0), ("1 KB", 0), ("4 KB+", 0)];
    for (name, count) in &buckets {
        y = H::kv(y, name, &alloc::format!("{}", count), 0xFF888888);
    }

    y = H::section(y, "Layout (v1.9)", 0xFFAAFF00);
    y = H::kv(y, "0x0000_0000", "Reserved (low 1MB)", 0xFFCCCCCC);
    y = H::kv(y, "0x0010_0000", "Kernel text/data/bss", 0xFF00FF00);
    y = H::kv(y, "0x0100_0000", "Kernel heap", 0xFFFFFF00);
    y = H::kv(y, "0x1000_0000", "Framebuffer GOP", 0xFF00FFFF);
    y = H::kv(y, "0x4000_0000", "User (Ring 3)", 0xFFCCCCCC);
    let _ = y;
    let _ = paint::fill_rect;
}
