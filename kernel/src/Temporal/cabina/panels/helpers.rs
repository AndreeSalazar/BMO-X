//! Helpers compartidos para paneles de Cabina.

#![allow(dead_code)]

use crate::cabina::paint;

/// Section header con color propio. Retorna el nuevo `y`.
pub fn section(y: u32, title: &str, color: u32) -> u32 {
    paint::fill_rect(0, y, 1920, 20, 0xFF202028);
    paint::draw_text(8, y + 2, title, color);
    y + 24
}

/// Fila de key-value. Retorna el nuevo `y`.
pub fn kv(y: u32, key: &str, val: &str, color: u32) -> u32 {
    paint::draw_text(16, y, key, 0xFFCCCCCC);
    paint::draw_text(280, y, val, color);
    y + 16
}

/// Fila key-value donde el value es `u64`. Retorna el nuevo `y`.
pub fn kv_u64(y: u32, key: &str, val: u64, color: u32) -> u32 {
    let s = alloc::format!("{}", val);
    kv(y, key, &s, color)
}

/// Fila key-value donde el value es un tamaño en bytes. Retorna el nuevo `y`.
pub fn kv_size(y: u32, key: &str, val: u64, color: u32) -> u32 {
    let s = if val < 1024 { alloc::format!("{} B", val) }
            else if val < 1024 * 1024 { alloc::format!("{} KB", val / 1024) }
            else { alloc::format!("{} MB", val / 1024 / 1024) };
    kv(y, key, &s, color)
}

/// Línea simple (sólo texto). Retorna el nuevo `y`.
pub fn line(y: u32, text: &str, color: u32) -> u32 {
    paint::draw_text(16, y, text, color);
    y + 16
}

/// Etiqueta header de panel.
pub fn header(title: &str, color: u32) {
    paint::fill_rect(0, 0, 1920, 32, 0xFF1A1A2E);
    paint::draw_text(8, 8, title, color);
    paint::draw_text(80, 8, "-- Cabina v1.0", 0xFF888888);
    paint::draw_text(1700, 8, "FastOS", 0xFF666666);
}
