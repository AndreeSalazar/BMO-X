//! `cabina::panels::gpu` — Panel GPU (placeholder, RDNA4 en v1.9+).

#![allow(dead_code)]

use crate::cabina::panels::helpers as H;
use crate::cabina::paint;

pub fn render(_s: &crate::cabina::snapshot::Snapshot) {
    H::header("GPU", 0xFF00FFFF);

    let mut y = 80u32;
    y = H::line(y, "GPU driver en v1.9+ (RDNA4).", 0xFFFFFF00);
    y = H::line(y, "Estado actual: skeleton.", 0xFFCCCCCC);
    y = H::line(y, "BMO_ABI::GPU se valida con el BEF header", 0xFFCCCCCC);
    y = H::line(y, "(el backend GPU completo no es objetivo v1.8.8).", 0xFF888888);
    y = H::section(y, "Counter planned (v1.9)", 0xFF00FFFF);
    let counters = [
        "GPU_SUBMIT_BUFFER", "GPU_DISPATCH", "GPU_PRESENT", "GPU_SYNC_OBJECT", "GPU_RESET",
    ];
    for c in &counters {
        y = H::line(y, c, 0xFFCCCCCC);
    }
    let _ = y;
    let _ = paint::fill_rect;
}
