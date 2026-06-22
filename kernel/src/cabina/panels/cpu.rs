//! `cabina::panels::cpu` — Panel de CPU con detalle granular.

#![allow(dead_code)]

use crate::cabina::snapshot::Snapshot;
use crate::cabina::panels::helpers as H;
use crate::cabina::paint;

pub fn render(s: &Snapshot) {
    H::header("CPU", 0xFF00FFAA);

    let mut y = 40u32;
    let c = &s.cpu;

    y = H::section(y, "Interrupts", 0xFF00FFAA);
    y = H::kv_u64(y, "Total",   c.interrupts, 0xFFFFFFFF);
    y = H::kv_u64(y, "Timer",   c.timer_ticks, 0xFFCCCCCC);
    y = H::line(y, "Per-IRQ (v1.9: per_irq table)", 0xFF888888);

    y = H::section(y, "Faults", 0xFF00FFAA);
    y = H::kv_u64(y, "Page (#PF)",     c.pf,  0xFFFFFF00);
    y = H::kv_u64(y, "General (#GP)", c.gp,  0xFFFF8800);
    y = H::kv_u64(y, "NMI",            c.nm,  0xFFFF4400);
    y = H::kv_u64(y, "Double (#DF)",   c.df,  0xFFFF0000);
    y = H::kv_u64(y, "Invalid (#UD)",  c.ud,  0xFFFF8800);
    y = H::kv_u64(y, "Machine (#MC)",  c.mc,  0xFFFF0000);

    y = H::section(y, "TSC / CPU", 0xFF00FFAA);
    y = H::kv_u64(y, "TSC ticks",  c.timer_ticks, 0xFFCCCCCC);
    y = H::kv(y, "TSC freq",  "5600X ~3.7 GHz", 0xFF00FF00);
    y = H::kv(y, "Vendor",   "AMD",  0xFFFFFFFF);
    y = H::kv(y, "Family",   "Zen 3 (0x19)", 0xFFCCCCCC);
    y = H::kv(y, "Model",    "0x21", 0xFFCCCCCC);
    let _ = y;
    let _ = paint::fill_rect; // keep
}
