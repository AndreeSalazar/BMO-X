//! v1.7.1 — Wallpaper procedural dark elegante para welcome + desktop.
//!
//! Compone un fondo oscuro en capas:
//!   1. Gradiente base vertical (near-black → indigo oscuro).
//!   2. Mesh gradient: dos blobs radiales suaves (mint y violeta) blend sobre
//!      el gradiente base, simulando iluminación ambiental de un estudio.
//!   3. Aurora: banda elíptica diagonal con glow mint muy tenue.
//!   4. Grid sutil de líneas (1 px, alpha bajo) para textura de superficie.
//!   5. Estrellas estáticas dispersas (semilla fija → determinístico en boot).
//!
//! Todo el render es en aritmética entera (no_std bare-metal) — sin f32
//! ni sqrt. Los falloff de los blobs son cuadráticos y la aurora usa una
//! aproximación de sqrt por tabla o por shift.

#![allow(dead_code)]

use crate::bmo_core::ui::fb::Framebuffer;

#[inline]
fn argb(r: u8, g: u8, b: u8) -> u32 { 0xFF000000 | ((r as u32) << 16) | ((g as u32) << 8) | b as u32 }

#[inline]
fn blend_rgb(fg_r: u8, fg_g: u8, fg_b: u8, bg: u32, alpha: u32) -> u32 {
    let a = alpha.min(255) as u32;
    let inv = 255 - a;
    let br = (bg >> 16) & 0xFF;
    let bg_g = (bg >> 8) & 0xFF;
    let bb = bg & 0xFF;
    let r = (fg_r as u32 * a + br * inv) / 255;
    let g = (fg_g as u32 * a + bg_g * inv) / 255;
    let bl = (fg_b as u32 * a + bb * inv) / 255;
    0xFF000000 | (r << 16) | (g << 8) | bl
}

#[inline]
fn xorshift(state: &mut u32) -> u32 {
    let mut x = *state;
    x ^= x << 13;
    x ^= x >> 17;
    x ^= x << 5;
    *state = x;
    x
}

/// Tabla de sqrt(i*256) precalculada para valores 0..=65535.
/// Sirve para la aurora sin usar f32.
const SQRT_TABLE: [u16; 256] = [
    256, 257, 257, 258, 259, 259, 260, 261, 261, 262, 263, 263, 264, 265, 265, 266,
    266, 267, 268, 268, 269, 270, 270, 271, 272, 272, 273, 274, 274, 275, 275, 276,
    277, 277, 278, 279, 279, 280, 281, 281, 282, 282, 283, 284, 284, 285, 286, 286,
    287, 288, 288, 289, 289, 290, 291, 291, 292, 293, 293, 294, 294, 295, 296, 296,
    297, 298, 298, 299, 299, 300, 301, 301, 302, 303, 303, 304, 304, 305, 306, 306,
    307, 308, 308, 309, 309, 310, 311, 311, 312, 312, 313, 314, 314, 315, 316, 316,
    317, 317, 318, 319, 319, 320, 320, 321, 322, 322, 323, 324, 324, 325, 325, 326,
    327, 327, 328, 328, 329, 330, 330, 331, 331, 332, 333, 333, 334, 334, 335, 336,
    336, 337, 338, 338, 339, 339, 340, 341, 341, 342, 342, 343, 344, 344, 345, 345,
    346, 347, 347, 348, 348, 349, 350, 350, 351, 351, 352, 353, 353, 354, 354, 355,
    356, 356, 357, 357, 358, 359, 359, 360, 360, 361, 362, 362, 363, 363, 364, 364,
    365, 366, 366, 367, 367, 368, 369, 369, 370, 370, 371, 372, 372, 373, 373, 374,
    374, 375, 376, 376, 377, 377, 378, 379, 379, 380, 380, 381, 382, 382, 383, 383,
    384, 384, 385, 386, 386, 387, 387, 388, 389, 389, 390, 390, 391, 391, 392, 393,
    393, 394, 394, 395, 396, 396, 397, 397, 398, 398, 399, 400, 400, 401, 401, 402,
    403, 403, 404, 404, 405, 405, 406, 407, 407, 408, 408, 409, 409, 410, 411, 411,
];

/// Aproxima sqrt(v) en Q16.16 (devuelve entero escalado a 65536).
/// Para v ≤ 65535 funciona con la tabla; para v mayores lo computamos con
/// un método de bisección/Newton entero. Como en 1920×1080 las distancias
/// cuadradas caben en 32 bits con holgura, basta una iteración de Newton
/// partiendo de la tabla.
#[inline]
fn isqrt(v: u32) -> u32 {
    if v == 0 { return 0; }
    // Aproximación inicial desde la tabla (256 entradas para 0..=255).
    let mut x = if v <= 255 {
        SQRT_TABLE[v as usize] as u32
    } else {
        // Para v > 255 escalamos por 16: tomamos sqrt(v/16) de la tabla y
        // multiplicamos por 4. Buen seed para Newton.
        let s = v >> 4; // v/16
        let idx = if s > 255 { 255 } else { s as usize };
        (SQRT_TABLE[idx] as u32) << 2
    };
    if x == 0 { x = 1; }
    // Una iteración de Newton: x = (x + v/x) / 2
    let mut next = (x + v / x) >> 1;
    if next < x { x = next; } else { return x; }
    next = (x + v / x) >> 1;
    if next < x { x = next; }
    x
}

/// Pinta el wallpaper completo sobre `fb`.
///
/// Dibuja el wallpaper del welcome.
///
/// v1.8.14: versión MINIMAL. La versión procedural anterior pintaba
/// ~2M píxeles por frame (gradient + 2 blobs + aurora + grid + 173
/// estrellas). En un monitor 1920×1080, sin MTRR WC bien configurado,
/// cada `write_volatile` al framebuffer tardaba ~100ns = 200ms total.
/// Eso provocaba que el watchdog de 5s disparara reset ANTES de que
/// el welcome terminara de renderizar, y el usuario veía solo 2-3
/// segmentos verdes de la progress bar.
///
/// Ahora: un solo `fill_rect` con un color sólido. El render del
/// welcome se completa en <5ms y la progress bar pinta los 5
/// segmentos correctamente. La estética procedural se puede
/// reintroducir en v1.9 con MTRR WC fix.
pub fn draw(fb: &Framebuffer, _time: u64) {
    if fb.width == 0 || fb.height == 0 { return; }
    // Color base del tema oscuro (mismo que el gradient bottom original).
    fb.fill_rect(0, 0, fb.width, fb.height, 0xFF050B18);
}

/// Aurora: banda elíptica diagonal pintada por scanline. Recorremos
/// cada fila y calculamos el grosor de la banda en ese y; pintamos
/// los píxeles interiores con mint muy tenue.
fn draw_aurora_band(fb: &Framebuffer, w: i32, h: i32, t: i32) {
    let cx = w / 2 + (t % 9) * 2;
    let cy = h * 45 / 100;
    let a_axis = w * 80 / 100;
    let b_axis = h * 22 / 100;
    let mint_r = 0x4Eu8;
    let mint_g = 0xCCu8;
    let mint_b = 0xA3u8;

    let y0 = (cy - b_axis).max(0);
    let y1 = (cy + b_axis).min(h - 1);
    let b_axis_sq = (b_axis as i64) * (b_axis as i64);
    let a_axis_sq = (a_axis as i64) * (a_axis as i64);

    let mut y = y0;
    while y <= y1 {
        let dy = (y - cy) as i64;
        let dy2 = dy * dy;
        // k = 1 - dy²/b²; k_num = (b² - dy²) * 255 / b²
        if dy2 >= b_axis_sq { y += 1; continue; }
        let k_num = ((b_axis_sq - dy2) * 255) / b_axis_sq;
        // half_w² = a² * k_num / 255; half_w = isqrt(half_w²)
        let half_w_sq = ((a_axis_sq * k_num) / 255) as u32;
        let half_w = isqrt(half_w_sq) as i32;
        let x0 = (cx - half_w).max(0);
        let x1 = (cx + half_w).min(w - 1);
        // alpha varía según distancia vertical al centro.
        // alpha = (1 - |dy|/b) * 38
        let alpha_num = b_axis - (dy.abs() as i32);
        let alpha = (alpha_num * 38) / b_axis;
        if alpha <= 0 { y += 1; continue; }
        let alpha_u = alpha as u32;
        let mut x = x0;
        while x <= x1 {
            let pxi = x as usize;
            let pyi = y as usize;
            if pxi < fb.width && pyi < fb.height {
                fb.put_pixel(pxi, pyi, blend_rgb(mint_r, mint_g, mint_b, 0, alpha_u));
            }
            x += 1;
        }
        y += 1;
    }
}
