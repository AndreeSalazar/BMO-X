//! `cabina::panels::gateway` — Panel del desktop3 (cúpula Ring 0/3).
//!
//! Muestra las estadísticas de la única puerta entre Ring 0 y BMO Core:
//! - Total syscalls
//! - Allowed (ejecutados correctamente)
//! - Denied (bloqueados por ByteDefender)
//! - Unknown (NR fuera de rango)
//! - Lista de syscalls más usados

#![allow(dead_code)]

use crate::cabina::snapshot::Snapshot;
use crate::cabina::panels::helpers as H;
use crate::cabina::paint;
use crate::bmo_core::desktop3;

pub fn render(_s: &Snapshot) {
    H::header("GATEWAY", 0xFFFF0080);

    let mut y = 40u32;

    y = H::section(y, "Stats (acumuladas desde boot)", 0xFFFF0080);
    y = H::kv_u64(y, "Total",       desktop3::total(),   0xFFFFFFFF);
    y = H::kv_u64(y, "Allowed",     desktop3::allowed(), 0xFF00FF00);
    y = H::kv_u64(y, "Denied",      desktop3::denied(),  0xFFFF8800);
    y = H::kv_u64(y, "Unknown",     desktop3::unknown(), 0xFFFF0000);

    y = H::section(y, "Rate", 0xFFFF0080);
    let total = desktop3::total();
    let allowed = desktop3::allowed();
    let denied = desktop3::denied();
    let unknown = desktop3::unknown();
    if total > 0 {
        y = H::kv(y, "Allowed %",
                  &alloc::format!("{}%", (allowed * 100) / total),
                  0xFF00FF00);
        y = H::kv(y, "Denied  %",
                  &alloc::format!("{}%", (denied * 100) / total),
                  0xFFFF8800);
        y = H::kv(y, "Unknown %",
                  &alloc::format!("{}%", (unknown * 100) / total),
                  0xFFFF0000);
    } else {
        y = H::line(y, "(no syscalls yet)", 0xFF888888);
    }

    y = H::section(y, "Pipeline (por syscall)", 0xFFFF0080);
    paint::draw_text(16, y, "1. Validate range (0x100..0x1FF)", 0xFF00FFFF);
    y += 16;
    paint::draw_text(16, y, "2. ByteDefender: capabilities", 0xFFFF00FF);
    y += 16;
    paint::draw_text(16, y, "3. Cabina: trace_u64(name, nr)", 0xFF00FFAA);
    y += 16;
    paint::draw_text(16, y, "4. bmo_api::dispatch_syscall", 0xFFAAFF00);
    y += 16;
    paint::draw_text(16, y, "5. Return rax to Ring 3 (iretq)", 0xFFFFFFFF);
    y += 24;

    y = H::section(y, "About", 0xFFFF0080);
    y = H::line(y, "bmo_core::desktop3 is the only door", 0xFFCCCCCC);
    y += 14;
    y = H::line(y, "between Ring 0 and BMO Core.", 0xFFCCCCCC);
    y += 14;
    y = H::line(y, "All 86 BMO ABI syscalls pass through here.", 0xFFCCCCCC);
    y += 14;
    y = H::line(y, "See: bmo_core::desktop3::mod", 0xFF888888);
    y += 14;
    let _ = y;
}
