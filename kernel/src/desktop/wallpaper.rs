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

use crate::ui::fb::Framebuffer;
use crate::arch::cpu::rdtsc;

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
/// `time` (TSC ticks) se usa para una shimmer de aurora casi imperceptible.
/// Si no quieres animación, pásale 0.
pub fn draw(fb: &Framebuffer, time: u64) {
    let w = fb.width as i32;
    let h = fb.height as i32;
    if w <= 0 || h <= 0 { return; }

    // ── 1) Gradiente base por scanline ─────────────────────────────
    fb.gradient_v(0, 0, fb.width, fb.height, 0xFF050B18, 0xFF0A101F);

    // ── 2) Mesh gradient: dos blobs radiales ───────────────────────
    // t controla shimmer lento.
    let t = (time / 60_000_000) as i32;
    let blob_a_cx = w * 22 / 100 + ((t % 7) * 3);
    let blob_a_cy = h * 18 / 100;
    let blob_a_r = (w * 55 / 100).max(400);

    let blob_b_cx = w * 78 / 100 - ((t % 5) * 2);
    let blob_b_cy = h * 82 / 100;
    let blob_b_r = (w * 50 / 100).max(380);

    // Recorremos por bloques de 4 px en y para mantener coste bajo.
    let step = 4i32;
    let aa = (blob_a_r as i64) * (blob_a_r as i64);
    let bb = (blob_b_r as i64) * (blob_b_r as i64);

    let mut y = 0i32;
    while y < h {
        let block_h = step.min(h - y);
        let sample_y = y + block_h / 2;
        // Componente vertical del gradiente base en enteros 0..=255.
        // 0x05..=0x0A en R, 0x0B..=0x10 en G, 0x18..=0x1F en B.
        let t_num = sample_y.max(0) as u32;
        let t_den = h as u32;
        let t_y = if t_den == 0 { 0 } else { (t_num * 255) / t_den };
        let base_r = (0x05 + ((0x0A - 0x05) * t_y) / 255) as u8;
        let base_g = (0x0B + ((0x10 - 0x0B) * t_y) / 255) as u8;
        let base_b = (0x18 + ((0x1F - 0x18) * t_y) / 255) as u8;
        let base = argb(base_r, base_g, base_b);

        let mut x = 0i32;
        while x < w {
            let block_w = step.min(w - x);

            // Distancias al centro de cada blob.
            let dxa = (x - blob_a_cx) as i64;
            let dya = (y - blob_a_cy) as i64;
            let da2 = dxa * dxa + dya * dya;
            let a_int: u32 = if da2 < aa {
                // Falloff cuadrático: t = 1 - d²/r²; alpha = t² * 255
                let t_a_num = (aa - da2) as u32;
                let t_a = (t_a_num * 255) / (aa as u32);
                (t_a * t_a) / 255
            } else { 0 };

            let dxb = (x - blob_b_cx) as i64;
            let dyb = (y - blob_b_cy) as i64;
            let db2 = dxb * dxb + dyb * dyb;
            let b_int: u32 = if db2 < bb {
                let t_b_num = (bb - db2) as u32;
                let t_b = (t_b_num * 200) / (bb as u32);
                (t_b * t_b) / 255
            } else { 0 };

            if a_int == 0 && b_int == 0 {
                x += step;
                continue;
            }

            let mut px = base;
            if a_int > 0 {
                px = blend_rgb(0x4E, 0xCC, 0xA3, px, a_int);
            }
            if b_int > 0 {
                px = blend_rgb(0x6C, 0x5C, 0xE7, px, b_int);
            }

            for yy in 0..block_h {
                for xx in 0..block_w {
                    let pxi = (x + xx) as usize;
                    let pyi = (y + yy) as usize;
                    if pxi < fb.width && pyi < fb.height {
                        fb.put_pixel(pxi, pyi, px);
                    }
                }
            }
            x += step;
        }
        y += step;
    }

    // ── 3) Aurora: banda elíptica diagonal con glow mint tenue ─────
    if w >= 800 {
        draw_aurora_band(fb, w, h, t);
    }

    // ── 4) Grid sutil (1 px cada 64 px) ─────────────────────────────
    let grid_color = argb(0x1A, 0x25, 0x38);
    let step_g = 64i32;
    let mut gx = step_g;
    while gx < w {
        let mut gy = 0i32;
        while gy < h {
            let pxi = gx as usize;
            let pyi = gy as usize;
            if pxi < fb.width && pyi < fb.height {
                fb.put_pixel(pxi, pyi, grid_color);
            }
            gy += 1;
        }
        gx += step_g;
    }
    let mut gy = step_g;
    while gy < h {
        let mut gx2 = 0i32;
        while gx2 < w {
            let pxi = gx2 as usize;
            let pyi = gy as usize;
            if pxi < fb.width && pyi < fb.height {
                fb.put_pixel(pxi, pyi, grid_color);
            }
            gx2 += 1;
        }
        gy += step_g;
    }

    // ── 5) Estrellas dispersas ─────────────────────────────────────
    let mut rng = (rdtsc() as u32) | 1;
    let count = ((w as i32) * (h as i32) / 12000) as u32;
    for _ in 0..count {
        let r1 = xorshift(&mut rng);
        let r2 = xorshift(&mut rng);
        let r3 = xorshift(&mut rng);
        let sx = (r1 % (w as u32)) as usize;
        let sy = (r2 % (h as u32)) as usize;
        let alpha = 80 + (r3 % 70);
        let star = blend_rgb(0xE0, 0xEE, 0xFF, 0xFF0A101F, alpha);
        fb.put_pixel(sx, sy, star);
    }
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
