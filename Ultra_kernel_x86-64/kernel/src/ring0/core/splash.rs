//! Boot splash screen ??? premium animated boot experience.
//!
//! Ring 0 splash with smooth transitions:
//!   - Animated concentric logo (inside-out expansion)
//!   - Smooth interpolated progress bar
//!   - Phase label fade transitions
//!   - Professional typography with centered layout

// ?????? Font: 8x16 bitmap, chars 32..126 (space through ~) ??????????????????????????????

const FONT_H: usize   = 16;
const FONT_W: usize   = 8;
const CHAR_W: usize   = 10;  // 2px spacing
const CHAR_H: usize   = 20;  // 4px line spacing

static FONT16: [[u8; 16]; 120] = include!("font16_data.rs");
/// Bytes Latin-1 de los glifos extra, en el mismo orden en que aparecen en
/// FONT16 a partir del indice 95. Generado junto al font: si crece la tabla
/// del generador crecen los dos archivos y aqui solo cambia el tamano.
static FONT_EXTRA: [u8; 25] = include!("font16_extra.rs");
/// Cuantos glifos ASCII (32..=126) van primero en FONT16.
const ASCII_GLYPHS: usize = 95;

/// Byte -> indice de glifo. ASCII directo; para el espanol (n~, a-acento, ¿,
/// ...) se busca el byte Latin-1 en la tabla de extras.
///
/// Latin-1 y no UTF-8 a proposito: en Ring 0 un caracter es UN byte, asi el
/// teclado, la linea del shell y el framebuffer hablan el mismo idioma sin
/// decodificador de por medio.
fn glyph_index(c: u8) -> Option<usize> {
    if (32..=126).contains(&c) {
        return Some(c as usize - 32);
    }
    let mut i = 0;
    while i < FONT_EXTRA.len() {
        if FONT_EXTRA[i] == c { return Some(ASCII_GLYPHS + i); }
        i += 1;
    }
    None
}

// ?????? Color palette ???????????????????????????????????????????????????????????????????????????????????????????????????????????????????????????????????????????????????
const BG: u32          = 0xFF0A0F1D; // Deep space slate-blue
const WHITE: u32       = 0xFFF1F5F9; // Soft crisp white
const DIM: u32         = 0xFF64748B; // Slate-500 muted text
const ACCENT: u32      = 0xFF00E5FF; // Neon cyan highlight
const ACCENT2: u32     = 0xFF818CF8; // Indigo-400 accent for loading state
const BAR_BG: u32      = 0xFF1E293B; // Slate-800 progress bar background
const BAR_BORDER: u32  = 0xFF334155; // Slate-700 progress bar border

// Logo layers (inside ??? outside)
const LOGO_CORE: u32   = 0xFF00E5FF; // Cyan core dot
const LOGO_RING1: u32  = 0xFF4F46E5; // Indigo inner ring
const LOGO_RING2: u32  = 0xFF312E81; // Deep indigo mid ring
const LOGO_RING3: u32  = 0xFF1E293B; // Slate outer ring

// ?????? State for smooth progress interpolation ?????????????????????????????????????????????????????????????????????
static mut LAST_PCT: u32 = 0;

// ?????? Primitive drawing ???????????????????????????????????????????????????????????????????????????????????????????????????????????????????????????????????????
//
// The GOP framebuffer is typically mapped as WC (write-combining)
// by UEFI. WC stores are batched into the WC buffer and NOT
// guaranteed to reach VRAM until a full memory barrier flushes
// the buffer. `sfence` only orders `movnti` non-temporal stores;
// for normal WC writes, `mfence` is required. Without `mfence`,
// the display hardware sees the old contents (black) for an
// unpredictable amount of time, and the screen appears blank.

#[inline]
fn wc_flush() {
    // `mfence` is the correct barrier for WC memory:
    // it serializes all load/store instructions AND drains
    // the WC buffer before any subsequent loads or stores.
    unsafe { core::arch::asm!("mfence", options(nostack, preserves_flags)); }
}

fn put_pix(x: u32, y: u32, color: u32) {
    let fb = unsafe { crate::info::FB_ADDR as *mut u32 };
    let st  = unsafe { crate::info::FB_STRIDE as usize };
    let h   = unsafe { crate::info::FB_HEIGHT };
    if y < h && (x as usize) < st {
        unsafe {
            fb.add((y as usize) * st + (x as usize)).write_volatile(color);
        }
    }
}

fn fill_rect(x: u32, y: u32, w: u32, h: u32, color: u32) {
    let fb = unsafe { crate::info::FB_ADDR as *mut u32 };
    let st = unsafe { crate::info::FB_STRIDE as usize };
    let H  = unsafe { crate::info::FB_HEIGHT };
    if fb.is_null() { return; }
    let mut any = false;
    for dy in 0..h {
        let py = y + dy;
        if py >= H { break; }
        for dx in 0..w {
            let px = x + dx;
            if (px as usize) >= st { break; }
            unsafe { fb.add((py as usize) * st + (px as usize)).write_volatile(color); }
            any = true;
        }
    }
    if any { wc_flush(); }
}

fn draw_rect_outline(x: u32, y: u32, w: u32, h: u32, color: u32) {
    if w == 0 || h == 0 { return; }
    for dx in 0..w {
        put_pix(x + dx, y, color);
        put_pix(x + dx, y + h - 1, color);
    }
    for dy in 0..h {
        put_pix(x, y + dy, color);
        put_pix(x + w - 1, y + dy, color);
    }
    wc_flush();
}

/// Draw a filled circle using integer distance squared.
fn fill_circle(cx: u32, cy: u32, r: u32, color: u32) {
    let r_sq = r * r;
    for dy in 0..=r {
        // Horizontal span at this row: solve dx^2 + dy^2 <= r^2
        // dx <= sqrt(r^2 - dy^2)
        let dy_sq = dy * dy;
        if dy_sq > r_sq { break; }
        let dx_max = isqrt(r_sq - dy_sq);
        // Draw 4 quadrants
        let x0 = cx.saturating_sub(dx_max);
        let x1 = cx + dx_max;
        let y_top = cy.saturating_sub(dy);
        let y_bot = cy + dy;
        fill_rect(x0, y_top, x1 - x0 + 1, 1, color);
        if dy > 0 {
            fill_rect(x0, y_bot, x1 - x0 + 1, 1, color);
        }
    }
}

/// Draw a ring (filled circle minus inner filled circle).
fn draw_ring(cx: u32, cy: u32, r_outer: u32, thickness: u32, color: u32) {
    let r_inner = r_outer.saturating_sub(thickness);
    let ro_sq = r_outer * r_outer;
    let ri_sq = r_inner * r_inner;
    for dy in 0..=r_outer {
        let dy_sq = dy * dy;
        if dy_sq > ro_sq { break; }
        let dx_outer = isqrt(ro_sq - dy_sq);
        let dx_inner = if dy_sq <= ri_sq { isqrt(ri_sq - dy_sq) } else { 0 };
        // Right side
        if dx_outer > dx_inner {
            let x0 = cx + dx_inner + 1;
            let w = dx_outer - dx_inner;
            fill_rect(x0, cy.saturating_sub(dy), w, 1, color);
            if dy > 0 { fill_rect(x0, cy + dy, w, 1, color); }
        }
        // Left side
        if dx_outer > dx_inner {
            let x0 = cx.saturating_sub(dx_outer);
            let w = dx_outer - dx_inner;
            fill_rect(x0, cy.saturating_sub(dy), w, 1, color);
            if dy > 0 { fill_rect(x0, cy + dy, w, 1, color); }
        }
    }
}

/// Integer square root (Newton's method).
fn isqrt(n: u32) -> u32 {
    if n == 0 { return 0; }
    let mut x = n;
    let mut y = (x + 1) / 2;
    while y < x {
        x = y;
        y = (x + n / x) / 2;
    }
    x
}

/// TSC-based busy-wait. Reads TSC directly.
#[inline]
fn tsc_read() -> u64 {
    let lo: u32;
    let hi: u32;
    unsafe { core::arch::asm!("rdtsc", out("eax") lo, out("edx") hi); }
    ((hi as u64) << 32) | lo as u64
}

#[inline]
fn tsc_wait(cycles: u64) {
    let start = tsc_read();
    while tsc_read() - start < cycles {
        core::hint::spin_loop();
    }
}

// ?????? Color blending ????????????????????????????????????????????????????????????????????????????????????????????????????????????????????????????????????????????????

/// Blend a foreground color over BG at a given alpha (0..255).
fn blend(fg: u32, alpha: u32) -> u32 {
    let a = alpha.min(255);
    let inv = 255 - a;
    let fg_r = (fg >> 16) & 0xFF;
    let fg_g = (fg >> 8) & 0xFF;
    let fg_b = fg & 0xFF;
    let bg_r = (BG >> 16) & 0xFF;
    let bg_g = (BG >> 8) & 0xFF;
    let bg_b = BG & 0xFF;
    let r = (fg_r * a + bg_r * inv) / 255;
    let g = (fg_g * a + bg_g * inv) / 255;
    let b = (fg_b * a + bg_b * inv) / 255;
    0xFF000000 | (r << 16) | (g << 8) | b
}

/// Create a gradient color along the progress bar (cyan ??? indigo).
fn bar_gradient(x_off: u32, total_w: u32) -> u32 {
    if total_w == 0 { return ACCENT; }
    let t = (x_off * 255 / total_w).min(255);
    let inv = 255 - t;
    // ACCENT=0xFF00E5FF ??? ACCENT2=0xFF818CF8
    let r = (0x00 * inv + 0x81 * t) / 255;
    let g = (0xE5 * inv + 0x8C * t) / 255;
    let b = (0xFF * inv + 0xF8 * t) / 255;
    0xFF000000 | (r << 16) | (g << 8) | b
}

// ?????? Animated Logo (smooth radius sweep) ?????????????????????????????????????????????????????????????????????????????????

/// Draw the logo with smooth inside-out animation.
/// Each ring expands 1px of radius per frame tick.
fn draw_logo_animated(cx: u32, cy: u32) {
    // Phase 1: Core dot expands from r=0 to r=4
    let mut r: u32 = 0;
    while r <= 4 {
        fill_circle(cx, cy, r, LOGO_CORE);
        tsc_wait(8_000_000);
        r += 1;
    }
    tsc_wait(15_000_000);

    // Phase 2: Inner ring sweeps from r=8 to r=14
    r = 8;
    while r <= 14 {
        draw_ring(cx, cy, r, 2, LOGO_RING1);
        tsc_wait(5_000_000);
        r += 1;
    }
    tsc_wait(10_000_000);

    // Phase 3: Mid ring sweeps from r=16 to r=22
    r = 16;
    while r <= 22 {
        draw_ring(cx, cy, r, 3, LOGO_RING2);
        tsc_wait(4_000_000);
        r += 1;
    }
    tsc_wait(8_000_000);

    // Phase 4: Outer ring sweeps from r=24 to r=30
    r = 24;
    while r <= 30 {
        draw_ring(cx, cy, r, 2, LOGO_RING3);
        tsc_wait(3_000_000);
        r += 1;
    }

    // Phase 5: Accent glow ring (instant thin highlight)
    tsc_wait(6_000_000);
    draw_ring(cx, cy, 34, 1, LOGO_RING1);
}

// ?????? Text drawing ??????????????????????????????????????????????????????????????????????????????????????????????????????????????????????????????????????????????????????

fn draw_char(x: u32, y: u32, c: u8, color: u32) {
    let idx = match glyph_index(c) { Some(i) => i, None => return };
    let glyph = &FONT16[idx];

    // Is the glyph pixel at (col,row) set? Out-of-bounds counts as empty.
    let lit = |col: i32, row: i32| -> bool {
        if col < 0 || col >= FONT_W as i32 || row < 0 || row >= FONT_H as i32 {
            return false;
        }
        glyph[row as usize] & (0x80u8 >> col) != 0
    };

    // NÍTIDO Y SIMPLE (pedido del usuario, monitor 74 Hz): solo el glifo
    // exacto, a color pleno. Antes había un pase de "anti-alias" que rellenaba
    // las esquinas cóncavas con un tono tenue (blend 110) — eso REDONDEA pero
    // DIFUMINA el texto. Sin ese pase, cada píxel es limpio: letras crujientes.
    for row in 0..FONT_H as i32 {
        for col in 0..FONT_W as i32 {
            if lit(col, row) {
                put_pix(x + col as u32, y + row as u32, color);
            }
        }
    }
}

fn draw_str(x: u32, y: u32, s: &str, color: u32) {
    let mut cx = x;
    for b in s.bytes() {
        draw_char(cx, y, b, color);
        cx += CHAR_W as u32;
    }
    // `draw_char` pinta con `put_pix`, que NO drena el buffer WC. Sin este
    // flush, las letras llegan a VRAM tarde/parciales y dejan estela
    // fantasma (el "ghosting" del log rodante y del prompt). Un solo flush
    // por línea — barato — mata el efecto en todos los que dibujan texto.
    wc_flush();
}

fn text_width(s: &str) -> u32 {
    s.len() as u32 * CHAR_W as u32
}

/// Animate text with a 4-step alpha fade-in.
fn draw_str_fadein(x: u32, y: u32, s: &str, color: u32) {
    // 4 alpha steps: 64, 128, 192, 255
    let steps: [u32; 4] = [64, 128, 192, 255];
    let tw = text_width(s);
    for &alpha in steps.iter() {
        let c = blend(color, alpha);
        // Clear previous draw
        fill_rect(x, y, tw, FONT_H as u32, BG);
        draw_str(x, y, s, c);
        tsc_wait(8_000_000);
    }
}

// ══ Boot cinematic: escenas escaladas con transiciones ═══════════════════
//
// La entrada de BMO-X deja de ser un volcado de texto: una secuencia de
// escenas centradas (logo → preparando → RING 0 → RING 3) con fundido de
// entrada y una línea de acento que barre, al estilo de un arranque de SO
// moderno. Luego aterriza en el dashboard donde el trabajo real fluye.

/// Espera de `ms` milisegundos reales (usa la frecuencia TSC ya calibrada;
/// si aún no existe, aproxima a ~3 GHz).
fn hold_ms(ms: u64) {
    let f = crate::ring0::scheduler::tsc_freq();
    let cycles = if f == 0 { ms * 3_000_000 } else { ms * (f / 1000) };
    let start = tsc_read();
    while tsc_read().wrapping_sub(start) < cycles {
        core::hint::spin_loop();
    }
}

fn text_width_scaled(s: &str, scale: u32) -> u32 {
    s.len() as u32 * CHAR_W as u32 * scale
}

/// Un glifo dibujado a `scale`× (cada píxel = un bloque scale×scale). Sin AA:
/// a escala ≥3 los bloques ya leen limpios y con peso.
fn draw_char_scaled(x: u32, y: u32, c: u8, color: u32, scale: u32) {
    let idx = match glyph_index(c) { Some(i) => i, None => return };
    let glyph = &FONT16[idx];
    for row in 0..FONT_H {
        let bits = glyph[row];
        for col in 0..FONT_W {
            if bits & (0x80 >> col) != 0 {
                fill_rect(x + col as u32 * scale, y + row as u32 * scale, scale, scale, color);
            }
        }
    }
}

fn draw_str_scaled(x: u32, y: u32, s: &str, color: u32, scale: u32) {
    let mut cx = x;
    for b in s.bytes() {
        draw_char_scaled(cx, y, b, color, scale);
        cx += CHAR_W as u32 * scale;
    }
}

/// Una escena centrada: título grande con fundido de entrada, subtítulo dim,
/// y una línea de acento que barre bajo el título. Deja la pantalla en BG.
fn scene(title: &str, sub: &str, accent: u32, scale: u32) {
    let w = unsafe { crate::info::FB_WIDTH };
    let h = unsafe { crate::info::FB_HEIGHT };
    if w == 0 || h == 0 { return; }
    fill_rect(0, 0, w, h, BG);

    let tw = text_width_scaled(title, scale);
    let th = FONT_H as u32 * scale;
    let tx = w.saturating_sub(tw) / 2;
    let ty = h / 2 - th / 2 - 8;

    // Fundido de entrada del título (4 pasos de alpha sobre BG).
    for &a in &[70u32, 140, 210, 255] {
        draw_str_scaled(tx, ty, title, blend(accent, a), scale);
        wc_flush();
        hold_ms(45);
    }

    // Línea de acento que barre bajo el título.
    let uy = ty + th + 8;
    for step in 0..=24u32 {
        fill_rect(tx, uy, tw * step / 24, 3, accent);
        wc_flush();
        hold_ms(9);
    }

    // Subtítulo dim, centrado bajo la línea.
    if !sub.is_empty() {
        let sw = text_width(sub);
        draw_str(w.saturating_sub(sw) / 2, uy + 16, sub, DIM);
        wc_flush();
    }
}

/// Reproduce la secuencia de arranque completa (4 escenas). Llamar una vez,
/// con framebuffer disponible, antes de montar el dashboard.
pub fn boot_intro() {
    scene("BMO-X", "Bare Metal Orchestrator", ACCENT, 5);
    hold_ms(700);
    scene("Preparando", "iniciando subsistemas", ACCENT2, 3);
    hold_ms(350);
    scene("RING 0", "kernel + hardware al mando", ACCENT, 4);
    hold_ms(550);
    scene("RING 3", "userspace listo", DASH_RING3, 4);
    hold_ms(550);
}

// ?????? Smooth progress bar ?????????????????????????????????????????????????????????????????????????????????????????????????????????????????????????????????

/// Animate the progress bar smoothly from `LAST_PCT` to `target_pct`.
/// Uses sub-percentage pixel-level interpolation for ultra-smooth fill.
fn smooth_progress(bx: u32, by: u32, bar_w: u32, bar_h: u32, target_pct: u32) {
    let start_pix = unsafe { (bar_w as u64 * LAST_PCT as u64 / 100) as u32 };
    let end_pix = (bar_w as u64 * target_pct.min(100) as u64 / 100) as u32;

    if start_pix >= end_pix {
        unsafe { LAST_PCT = target_pct.min(100); }
        return;
    }

    // Animate pixel-by-pixel for maximum smoothness
    let mut px = start_pix;
    while px < end_pix {
        // Draw the new column with gradient color
        let col_color = bar_gradient(px, bar_w);
        fill_rect(bx + px, by, 1, bar_h, col_color);
        px += 1;

        // Adaptive speed: fast start, smooth middle, slow finish
        let progress_ratio = px * 100 / bar_w;
        let delay = if progress_ratio < 30 {
            800_000u64
        } else if progress_ratio < 70 {
            1_200_000u64
        } else {
            1_800_000u64
        };
        tsc_wait(delay);
    }

    unsafe { LAST_PCT = target_pct.min(100); }
}

// ?????? Public API ????????????????????????????????????????????????????????????????????????????????????????????????????????????????????????????????????????????????????????????

pub fn splash_init() {
    let w = unsafe { crate::info::FB_WIDTH };
    let h = unsafe { crate::info::FB_HEIGHT };
    let fb_addr = unsafe { crate::info::FB_ADDR };
    let fb_stride = unsafe { crate::info::FB_STRIDE };
    let fb_fmt = unsafe { crate::info::FB_PIXEL_FORMAT };

    // Log to serial (even if the user can't see it, it's useful later)
    crate::ring0::dev::console::serial_write("[splash] fb=0x");
    crate::ring0::dev::console::serial_write_u64(fb_addr, 16);
    crate::ring0::dev::console::serial_write(" ");
    crate::ring0::dev::console::serial_write_u64_dec(w as u64);
    crate::ring0::dev::console::serial_write("x");
    crate::ring0::dev::console::serial_write_u64_dec(h as u64);
    crate::ring0::dev::console::serial_write(" stride=");
    crate::ring0::dev::console::serial_write_u64_dec(fb_stride as u64);
    crate::ring0::dev::console::serial_write(" fmt=");
    crate::ring0::dev::console::serial_write_u64_dec(fb_fmt as u64);
    crate::ring0::dev::console::serial_write("\n");

    if w == 0 || h == 0 || fb_addr == 0 {
        crate::ring0::dev::console::serial_write("[splash] FB not available\n");
        return;
    }

    // ── Try filling the whole screen using rep stosd ───────────────
    //    This is the fastest, most reliable way to write a GPU
    //    framebuffer: the CPU's string-store engine does 64-byte
    //    bursts internally and handles WC buffering correctly.
    //    After the fill, we use mfence to flush the WC buffer.
    let total = (fb_stride as usize) * (h as usize);
    crate::ring0::dev::console::serial_write("[splash] filling ");
    crate::ring0::dev::console::serial_write_u64_dec(total as u64);
    crate::ring0::dev::console::serial_write(" px\n");

    unsafe {
        let di = fb_addr;
        let color: u32 = 0xFFFFFF00u32;
        core::arch::asm!(
            "cld",
            "mov rdi, {di}",
            "mov eax, {color:e}",
            "mov ecx, {count:e}",
            "rep stosd",
            "mfence",
            di = in(reg) di,
            color = in(reg) color,
            count = in(reg) total,
            options(nostack, preserves_flags),
        );
    }

    crate::ring0::dev::console::serial_write("[splash] fill done — screen should be yellow\n");

    // Wait a moment so the user can see the fill
    tsc_wait(300_000_000); // ~100 ms @ 3.7 GHz

    // Draw centered text over the fill
    let txt = "BMO-X";
    let tx = (w as u32).saturating_sub(text_width(txt)) / 2;
    let cy = h / 2;
    draw_str(tx, cy - 10, txt, 0xFF000000u32);
    wc_flush();
    crate::ring0::dev::console::serial_write("[splash] text drawn\n");

    // Skip the animated splash for now — the fill test is priority
}

pub fn splash_progress(pct: u32, label: &str) {
    let w = unsafe { crate::info::FB_WIDTH };
    let h = unsafe { crate::info::FB_HEIGHT };
    if w == 0 || h == 0 { return; }

    let cy = h / 2;
    let bar_w = 320u32;
    let bar_h = 6u32;
    let bx = (w as u32).saturating_sub(bar_w) / 2;
    let bar_y = cy + 50;

    // Smooth pixel-level interpolated progress bar
    smooth_progress(bx, bar_y, bar_w, bar_h, pct);

    // Update label (clear old, draw new centered)
    let label_y = bar_y + bar_h + 12;
    fill_rect(0, label_y, w, CHAR_H as u32, BG);
    let lx = (w as u32).saturating_sub(text_width(label)) / 2;
    draw_str(lx, label_y, label, ACCENT2);
}

pub fn splash_clear() {
    let w = unsafe { crate::info::FB_WIDTH };
    let h = unsafe { crate::info::FB_HEIGHT };
    if w == 0 { return; }
    fill_rect(0, 0, w, h, BG);
}

// ═══════════════════════════════════════════════════════════════════
//  Persistent Dashboard
// ═══════════════════════════════════════════════════════════════════
//
// Once the boot splash finishes, the kernel switches to a
// persistent dashboard on the framebuffer. This is the visual
// equivalent of the serial shell: it shows the system status,
// the latest kernel log lines, and a prompt. Anything typed on
// the serial (COM1) is echoed on the screen so the user can
// interact even without a serial terminal attached.

const DASH_HEADER_H:  u32 = 44;  // top bar height
const DASH_FOOTER_H:  u32 = 36;  // bottom prompt bar height
const DASH_LOG_TOP:   u32 = 72;  // y of first log line
const DASH_LOG_W:     u32 = 80;  // max chars per line
const DASH_ROWS_MAX:  usize = 64; // tope duro (protege los buffers de filas)

/// Filas de log que CABEN de verdad en el panel, según el alto REAL del
/// framebuffer.
///
/// Antes esto era una constante de 14. En 1080p (CHAR_H=20) caben ~49: se
/// desperdiciaban dos tercios del panel y, peor, obligaba al log rodante y a
/// CABINA a pelearse las mismas filas 2-13 borrándose mutuamente. El reparto
/// ahora lo decide el hardware, no un número mágico: pregúntale al hardware
/// los HECHOS, hardcodea solo los CONTRATOS.
pub fn dash_rows() -> usize {
    let h = unsafe { crate::info::FB_HEIGHT };
    if h == 0 { return 0; }
    let avail = h.saturating_sub(DASH_FOOTER_H + DASH_LOG_TOP + 4);
    ((avail as usize) / CHAR_H).min(DASH_ROWS_MAX)
}

// ── PALETA: neón sobre negro ────────────────────────────────────────────────
//
// El fondo baja casi a negro puro a propósito: un neón solo brilla si lo que
// tiene alrededor está apagado. El slate azulado anterior le robaba fuerza a
// todos los acentos porque ya era luminoso de por sí.
//
// La familia son tres luces frías (cian, jade, violeta) contra tres cálidas
// (ámbar, oro, magenta), con el rojo lacado reservado EXCLUSIVAMENTE para lo
// que va mal. Que el rojo no se use de adorno es lo que hace que, cuando
// aparece, la vista vaya sola.

const VOID:           u32 = 0xFF04060C; // fuera del panel — negro con tinte
const PANEL:          u32 = 0xFF080B14; // fondo del área de log
const CHROME:         u32 = 0xFF10151F; // barras superior e inferior
const EDGE:           u32 = 0xFF1E2738; // bordes apagados

const NEON_CYAN:      u32 = 0xFF00F0FF;
const NEON_MAGENTA:   u32 = 0xFFFF2D9B;
const NEON_AMBER:     u32 = 0xFFF6C445; // el amarillo de firma
const NEON_GOLD:      u32 = 0xFFFFB300;
const NEON_RED:       u32 = 0xFFFF3355; // solo para faults
const NEON_GREEN:     u32 = 0xFF39FF88;
const NEON_VIOLET:    u32 = 0xFFA78BFA;
const NEON_JADE:      u32 = 0xFF2DE2C5;

const DASH_BG:        u32 = PANEL;
const DASH_BAR:       u32 = CHROME;
const DASH_ACCENT:    u32 = NEON_CYAN;
const DASH_TEXT:      u32 = 0xFFE6EDF7;
const DASH_DIM:       u32 = 0xFF55647E;

// Colores-filtro por origen de línea (pedido del usuario): quien emite se
// reconoce por color sin leer el prefijo.
const DASH_RING3:     u32 = NEON_GREEN;   // salida de Ring 3
const DASH_TELEMETRY: u32 = NEON_AMBER;   // heartbeat r3hb (tablero)
const DASH_KBD:       u32 = NEON_VIOLET;  // entrada — teclado y ratón
const DASH_FAULT:     u32 = NEON_RED;     // reporter de CPU faults
const DASH_STORAGE:   u32 = NEON_JADE;    // disco y sistema de ficheros
const DASH_LANG_C:    u32 = NEON_CYAN;    // programas C
const DASH_LANG_COB:  u32 = NEON_GOLD;    // programas COBOL
const DASH_LANG_ASM:  u32 = NEON_MAGENTA; // programas en ensamblador
const DASH_STAGE:     u32 = NEON_AMBER;   // encabezados de acto

/// Color de una línea del log según su prefijo. Un solo punto de decisión:
/// TODOS los caminos que pintan al panel (rolling log, CABINA, faults) pasan
/// por aquí.
///
/// La tabla creció con los emisores que ya existían y salían todos en blanco:
/// los tres lenguajes tenían el mismo color que un mensaje del kernel, así que
/// la pantalla más impresionante del proyecto —tres programas propios
/// entrelazándose— se leía como un párrafo plano. Ahora cada voz tiene la suya.
fn dash_line_color(msg: &str) -> u32 {
    let b = msg.as_bytes();
    // Programas de Ring 3, por lenguaje: cada uno con su luz.
    if b.starts_with(b"C> ") {
        DASH_LANG_C
    } else if b.starts_with(b"COBOL>") {
        DASH_LANG_COB
    } else if b.starts_with(b"asm>") {
        DASH_LANG_ASM
    } else if b.starts_with(b"ring3>") || b.starts_with(b"[ring3]") {
        DASH_RING3
    } else if b.starts_with(b"==") {
        // Encabezados de etapa del boot ("== RING 0 ... ==") y del shell.
        DASH_STAGE
    } else if b.starts_with(b"r3hb") {
        DASH_TELEMETRY
    } else if b.starts_with(b"kbd ") || b.starts_with(b"[usb]") || b.starts_with(b"[xhci]")
        || b.starts_with(b"[uhid]") {
        DASH_KBD
    } else if b.starts_with(b"[disk]") || b.starts_with(b"[ahci]") || b.starts_with(b"[fs]")
        || b.starts_with(b"[cabina]") {
        DASH_STORAGE
    } else if b.starts_with(b"[ring0]") || b.starts_with(b"[bex]") {
        DASH_ACCENT
    } else if b.starts_with(b"***") || b.starts_with(b"vec ") || b.starts_with(b"flt") {
        DASH_FAULT
    } else {
        DASH_TEXT
    }
}

// ── Cromo: las piezas que dan el look ───────────────────────────────────────

/// Línea horizontal de 1 px con degradado entre dos colores.
///
/// Es el truco más barato que existe para que una interfaz deje de parecer un
/// terminal: una sola fila de píxeles interpolada cuesta un bucle y cambia por
/// completo la sensación de la barra que subraya.
fn hline_gradient(x: u32, y: u32, w: u32, c1: u32, c2: u32) {
    if w == 0 { return; }
    let (r1, g1, b1) = ((c1 >> 16) & 0xFF, (c1 >> 8) & 0xFF, c1 & 0xFF);
    let (r2, g2, b2) = ((c2 >> 16) & 0xFF, (c2 >> 8) & 0xFF, c2 & 0xFF);
    for i in 0..w {
        // Media ponderada: multiplicar ANTES de dividir. Interpolar por canal
        // con una resta encadenada se rompe en cuanto el color destino es más
        // oscuro que el de origen, y el degradado se queda plano sin avisar.
        let r = (r1 * (w - i) + r2 * i) / w;
        let g = (g1 * (w - i) + g2 * i) / w;
        let b = (b1 * (w - i) + b2 * i) / w;
        put_pix(x + i, y, 0xFF00_0000 | (r << 16) | (g << 8) | b);
    }
}

/// Esquinas en L en vez de un marco cerrado.
///
/// Es la firma visual del género: el ojo cierra el rectángulo solo y el panel
/// respira. Un borde continuo encajona; cuatro corchetes sugieren.
fn corner_brackets(x: u32, y: u32, w: u32, h: u32, len: u32, thick: u32, color: u32) {
    if w < len * 2 || h < len * 2 { return; }
    // Superior izquierda
    fill_rect(x, y, len, thick, color);
    fill_rect(x, y, thick, len, color);
    // Superior derecha
    fill_rect(x + w - len, y, len, thick, color);
    fill_rect(x + w - thick, y, thick, len, color);
    // Inferior izquierda
    fill_rect(x, y + h - thick, len, thick, color);
    fill_rect(x, y + h - len, thick, len, color);
    // Inferior derecha
    fill_rect(x + w - len, y + h - thick, len, thick, color);
    fill_rect(x + w - thick, y + h - len, thick, len, color);
}

/// Etiqueta de sección con su bloque de acento delante: `▌ TEXTO`.
///
/// El bloque es un rectángulo, no un glifo: la fuente es de 95 caracteres
/// ASCII más 25 de Latin-1 y no tiene caracteres de dibujo. Pintar el adorno
/// en vez de escribirlo evita inventar glifos que no existen.
fn section_label(x: u32, y: u32, text: &str, accent: u32) {
    fill_rect(x, y + 2, 4, FONT_H as u32 - 4, accent);
    draw_str(x + 12, y, text, DASH_DIM);
}

/// Draw the persistent dashboard frame. Called once after the
/// splash finishes — replaces the cleared screen with a UI that
/// stays visible for the rest of the kernel's lifetime.
pub fn splash_dashboard_init() {
    let w = unsafe { crate::info::FB_WIDTH };
    let h = unsafe { crate::info::FB_HEIGHT };
    if w == 0 || h == 0 { return; }

    // 1. El vacío. Todo lo que no es panel ni barra queda casi negro para que
    //    el neón tenga contra qué brillar.
    fill_rect(0, 0, w, h, VOID);

    // 2. Barra superior: identidad del sistema.
    fill_rect(0, 0, w, DASH_HEADER_H, CHROME);
    // Marca de acento a la izquierda — el bloque vertical que ancla el título.
    fill_rect(0, 0, 5, DASH_HEADER_H, NEON_MAGENTA);
    // El nombre en dos pesos: la marca en ámbar, el subsistema en magenta.
    // Separarlos dice de un vistazo QUÉ es y DÓNDE está corriendo.
    draw_str(22, 14, "BMO-X", NEON_AMBER);
    let x_after = 22 + text_width("BMO-X") + 12;
    draw_str(x_after, 14, "// RING 0", NEON_MAGENTA);
    let x_sub = x_after + text_width("// RING 0") + 16;
    draw_str(x_sub, 14, "bare metal orchestrator", DASH_DIM);
    // Subrayado de neón que recorre la barra: cian a la izquierda, magenta a
    // la derecha. Es la pieza que más cambia la sensación por menos píxeles.
    hline_gradient(0, DASH_HEADER_H - 2, w, NEON_CYAN, NEON_MAGENTA);
    hline_gradient(0, DASH_HEADER_H - 1, w, NEON_CYAN, NEON_MAGENTA);

    // 3. Barra inferior: el prompt.
    let fy = h - DASH_FOOTER_H;
    fill_rect(0, fy, w, DASH_FOOTER_H, CHROME);
    fill_rect(0, fy, 5, DASH_FOOTER_H, NEON_CYAN);
    hline_gradient(0, fy, w, NEON_MAGENTA, NEON_CYAN);

    // 4. El panel del log: fondo propio, un punto más claro que el vacío, para
    //    que se lea como una superficie y no como un agujero.
    let log_y = DASH_LOG_TOP;
    let log_h = h - DASH_FOOTER_H - log_y - 4;
    fill_rect(8, log_y - 6, w - 16, log_h, PANEL);
    // Bordes tenues + esquinas en L encendidas.
    draw_rect_outline(8, log_y - 6, w - 16, log_h, EDGE);
    corner_brackets(8, log_y - 6, w - 16, log_h, 22, 2, NEON_CYAN);

    // 5. Etiqueta de sección, ya fuera de la barra superior. Antes se dibujaba
    //    a 22 px del borde del log, o sea DENTRO de la cabecera: los dos
    //    textos se rozaban.
    section_label(20, DASH_LOG_TOP - 30, "KERNEL LOG", NEON_CYAN);
}

/// Write a single log line into the dashboard's log area at
/// line `row` (0 = top, growing downward). Newer lines overwrite
/// older ones on the same row, so callers can manage a ring of
/// `dash_rows()` rows.
pub fn splash_dashboard_log(row: usize, msg: &str) {
    let c = dash_line_color(msg);
    splash_dashboard_log_color(row, msg, c);
}

/// Regla de separación con etiqueta, a la altura de una fila del panel.
///
/// Es lo que separa el log rodante del cockpit de CABINA. Antes las dos zonas
/// se tocaban y la única pista de dónde acababa una era leer el contenido;
/// ahora hay una frontera que se ve sin leer. La línea se apaga hacia la
/// derecha para no competir con el texto que viene debajo.
///
/// El texto tiene que ser ASCII: la consola es Latin-1 de un byte por carácter
/// y un literal Rust con acentos viajaría en UTF-8, o sea dos glifos raros
/// donde debería haber uno.
pub fn splash_dash_rule(row: usize, label: &str, accent: u32) {
    let w = unsafe { crate::info::FB_WIDTH };
    if w == 0 || row >= dash_rows() { return; }
    let y = DASH_LOG_TOP + (row as u32) * CHAR_H as u32;
    fill_rect(20, y, w - 40, CHAR_H as u32, PANEL);
    fill_rect(20, y + 3, 4, CHAR_H as u32 - 8, accent);
    draw_str(32, y + 1, label, accent);
    let lx = 32 + text_width(label) + 14;
    let right = w.saturating_sub(20);
    if right > lx {
        hline_gradient(lx, y + (CHAR_H as u32) / 2, right - lx, accent, PANEL);
    }
}

/// Igual que `splash_dashboard_log` pero con COLOR EXPLÍCITO — para que CABINA
/// pinte cada fila según su estado (verde=bien, ámbar=atención, rojo=problema)
/// en vez de un solo color plano.
pub fn splash_dashboard_log_color(row: usize, msg: &str, color: u32) {
    let w = unsafe { crate::info::FB_WIDTH };
    let h = unsafe { crate::info::FB_HEIGHT };
    if w == 0 || h == 0 { return; }
    if row >= dash_rows() { return; }
    let y = DASH_LOG_TOP + (row as u32) * CHAR_H as u32;
    // Clear the row (background)
    fill_rect(20, y, w - 40, CHAR_H as u32, DASH_BG);
    // Draw up to DASH_LOG_W characters
    let mut buf = [0u8; DASH_LOG_W as usize];
    let bytes = msg.as_bytes();
    let n = bytes.len().min(buf.len());
    buf[..n].copy_from_slice(&bytes[..n]);
    if let Ok(s) = core::str::from_utf8(&buf[..n]) {
        draw_str(20, y, s, color);
    }
}

/// Update the bottom prompt area with the current command being
/// typed. The caller passes the in-progress line (up to a
/// reasonable limit). The prompt always starts with "serial > ".
pub fn splash_dashboard_prompt(line: &str, cursor: usize, blink: bool) {
    let w = unsafe { crate::info::FB_WIDTH };
    let h = unsafe { crate::info::FB_HEIGHT };
    if w == 0 || h == 0 { return; }
    let y = h - DASH_FOOTER_H + 10;
    fill_rect(20, y, w - 40, CHAR_H as u32, CHROME);
    // El prompt ya no dice "serial": el teclado USB escribe desde hace tiempo
    // y la etiqueta se había quedado contando una etapa anterior del proyecto.
    // La marca en ámbar, el signo en magenta — los mismos dos colores del
    // título, para que cabecera y pie se lean como el mismo sistema.
    const PROMPT: &str = "bmo-x";
    draw_str(20, y, PROMPT, NEON_AMBER);
    let sign_x = 20 + text_width(PROMPT) + 8;
    draw_str(sign_x, y, ">", NEON_MAGENTA);
    let prefix_w = text_width(PROMPT) + 8 + text_width("> ") + 4;
    let max_chars = ((w - 40 - prefix_w) / CHAR_W as u32) as usize;
    let n = line.len().min(max_chars);
    let s = &line[..n];
    draw_str(20 + prefix_w, y, s, DASH_TEXT);
    // Cursor de bloque parpadeante EN SU POSICION dentro de la linea, no
    // siempre al final: con las flechas se edita en medio, y el cursor tiene
    // que estar donde va a caer la siguiente letra. Si tapa un caracter, se
    // redibuja encima en el color del fondo — video inverso, como una terminal
    // de verdad.
    if blink {
        let cx = 20 + prefix_w + (cursor.min(n) as u32) * CHAR_W as u32;
        fill_rect(cx, y, (CHAR_W as u32) - 2, FONT_H as u32, NEON_MAGENTA);
        if cursor < n {
            let one = [line.as_bytes()[cursor]];
            if let Ok(ch) = core::str::from_utf8(&one) {
                draw_str(cx, y, ch, CHROME);
            }
        }
    }
    wc_flush();
}


/// Indicadores de la barra superior: distribucion de teclado activa y estado
/// de los bloqueos. Las lucecitas fisicas de un teclado pueden no responder
/// (firmware, emulacion); la pantalla no depende de eso.
pub fn splash_status_right(layout: &str, caps: bool, num: bool) {
    let w = unsafe { crate::info::FB_WIDTH };
    if w == 0 { return; }

    // La franja se limpia entera antes de escribir: al apagarse un indicador su
    // texto tiene que desaparecer, no quedarse pegado.
    let bar_x = w.saturating_sub(460);
    fill_rect(bar_x, 8, w.saturating_sub(bar_x + 16), DASH_HEADER_H - 12, CHROME);

    // Los bloqueos dejan de ser texto suelto y pasan a ser PASTILLAS: fondo
    // encendido y letra oscura. Un estado activo se ve encendido, no escrito —
    // que es justo lo que un teclado cuyas lucecitas no responden necesita.
    let caps_w = text_width("MAYUS") + 14;
    let num_w  = text_width("NUM") + 14;
    let mut kbd = [0u8; 32];
    let mut ko = 0usize;
    for &c in b"kbd ".iter() { if ko < kbd.len() { kbd[ko] = c; ko += 1; } }
    for &c in layout.as_bytes() { if ko < kbd.len() { kbd[ko] = c; ko += 1; } }
    let kbd_s = core::str::from_utf8(&kbd[..ko]).unwrap_or("");
    let kbd_w = text_width(kbd_s);

    let mut total = kbd_w;
    if caps { total += caps_w + 10; }
    if num  { total += num_w + 10; }
    let mut x = w.saturating_sub(total + 20);

    draw_str(x, 14, kbd_s, DASH_DIM);
    x += kbd_w + 10;
    if caps {
        fill_rect(x, 10, caps_w, FONT_H as u32 + 8, NEON_AMBER);
        draw_str(x + 7, 14, "MAYUS", CHROME);
        x += caps_w + 10;
    }
    if num {
        fill_rect(x, 10, num_w, FONT_H as u32 + 8, NEON_JADE);
        draw_str(x + 7, 14, "NUM", CHROME);
    }
}
