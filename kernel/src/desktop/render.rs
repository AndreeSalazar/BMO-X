//! Renderer del escritorio BMO — Ring 0. Una sola llamada
//! `render_frame()` pinta un frame completo estilo Win11/macOS/Linux.
//!
//! Composición:
//!
//! ```text
//! ┌────────────────────────── status bar (macOS top) ─────────────┐
//! │  🍎  BMO  Archivo  Editar  Ver        09:00:00  ⚡85%  📶  📊 │
//! ├───────────────────────────────────────────────────────────────┤
//! │   ╭──── BMO Terminal ────╮      ╭──── Datos.md viewer ────╮   │
//! │   │ ○ ○ ○                │      │ ○ ○ ○                   │   │
//! │   │ $ bmo > _            │      │ FastOS v0.9.0           │   │
//! │   │                      │      │ Ring 0+3 OK             │   │
//! │   ╰──────────────────────╯      ╰─────────────────────────╯   │
//! │           ╭──── Compositor Info ────╮                          │
//! │           │ ○ ○ ○                   │                          │
//! │           │ FPS:60  frame:12345     │                          │
//! │           ╰─────────────────────────╯                          │
//! │                                                                │
//! │   ╭───────── Dock (centrado, semi-translucido) ──────────╮     │
//! │   │ [📁][💬][🎮][🌐][⚙️][🔍][🗑]                        │     │
//! │   ╰──────────────────────────────────────────────────────╯     │
//! └───────────────────────────────────────────────────────────────┘
//! ```

#![allow(dead_code)]

use crate::boot_info;
use crate::fb::Framebuffer;
use crate::font;
use super::state::{self, DOCK_SLOTS};

// ── Paleta del escritorio (look macOS Sequoia + Win11 + Hyprland) ──
mod palette {
    pub const WALL_TOP:     u32 = 0xFF1E2A52;  // azul profundo
    pub const WALL_BOT:     u32 = 0xFF4B1F70;  // púrpura
    pub const STATUS_BG:    u32 = 0xCC1A1B26;  // semi-transparente (no soportamos alpha real, mezclamos)
    pub const STATUS_FG:    u32 = 0xFFE6EDF3;
    pub const STATUS_DIM:   u32 = 0xFFA0A8B8;

    pub const WIN_SHADOW:   u32 = 0xFF050810;
    pub const WIN_BG:       u32 = 0xFF21262D;
    pub const WIN_BORDER:   u32 = 0xFF3A4150;
    pub const WIN_TITLE:    u32 = 0xFF2D333C;
    pub const WIN_TITLE_HL: u32 = 0xFF0078D4;  // ventana activa (Win11 blue)

    pub const TRAFFIC_R:    u32 = 0xFFFF5F56;
    pub const TRAFFIC_Y:    u32 = 0xFFFFBD2E;
    pub const TRAFFIC_G:    u32 = 0xFF27C93F;

    pub const TEXT_PRIMARY: u32 = 0xFFE6EDF3;
    pub const TEXT_SECOND:  u32 = 0xFF8B949E;
    pub const TEXT_OK:      u32 = 0xFF76B900;
    pub const TEXT_INFO:    u32 = 0xFF56D4DD;

    pub const DOCK_BG:      u32 = 0xFF202531;
    pub const DOCK_HOVER:   u32 = 0xFF2F3A55;

    pub const CURSOR_FG:    u32 = 0xFFFFFFFF;
    pub const CURSOR_SHADOW:u32 = 0xFF000000;

    // Iconos del dock (color sólido por slot — placeholder estilo Win11 acrylic)
    pub const DOCK_ICONS: [u32; 7] = [
        0xFFE0A458,  // Files
        0xFF58A6FF,  // Chat
        0xFF76B900,  // Games
        0xFFBC8CFF,  // Web
        0xFF56D4DD,  // Settings
        0xFFFF7B72,  // Search
        0xFF7F848A,  // Trash
    ];
}

// ── Helpers locales ────────────────────────────────────────────────

fn fb() -> Option<Framebuffer> {
    let (addr, w, h, s) = unsafe {
        (boot_info::FB_ADDR, boot_info::FB_WIDTH, boot_info::FB_HEIGHT, boot_info::FB_STRIDE)
    };
    if addr == 0 || w == 0 { return None; }
    // Framebuffer::new toma `pitch` en bytes; stride es pixeles → *4.
    Some(Framebuffer::new(addr, (s as u64) * 4, w, h))
}

fn draw_text(fb: &Framebuffer, x: u32, y: u32, text: &[u8], color: u32) {
    let mut cx = x as usize;
    let cy = y as usize;
    for &ch in text {
        if cx + 8 > fb.width || cy + 16 > fb.height { break; }
        let glyph = font::get_glyph(ch);
        for py in 0..16 {
            let row = glyph[py];
            for px in 0..8 {
                if (row & (0x80 >> px)) != 0 {
                    fb.put_pixel(cx + px, cy + py, color);
                }
            }
        }
        cx += 8;
    }
}

fn fmt_u64(buf: &mut [u8], mut v: u64) -> &str {
    if v == 0 { buf[0] = b'0'; return core::str::from_utf8(&buf[..1]).unwrap(); }
    let mut tmp = [0u8; 20]; let mut i = 0;
    while v > 0 { tmp[i] = b'0' + (v % 10) as u8; v /= 10; i += 1; }
    for k in 0..i { buf[k] = tmp[i - 1 - k]; }
    core::str::from_utf8(&buf[..i]).unwrap()
}

fn fmt_hms(buf: &mut [u8; 8], h: u8, m: u8, s: u8) -> &str {
    buf[0] = b'0' + h / 10; buf[1] = b'0' + h % 10;
    buf[2] = b':';
    buf[3] = b'0' + m / 10; buf[4] = b'0' + m % 10;
    buf[5] = b':';
    buf[6] = b'0' + s / 10; buf[7] = b'0' + s % 10;
    core::str::from_utf8(buf).unwrap()
}

// ── Wallpaper ──────────────────────────────────────────────────────

fn draw_wallpaper(fb: &Framebuffer) {
    fb.gradient_v(0, 0, fb.width, fb.height, palette::WALL_TOP, palette::WALL_BOT);

    // "estrellas" simples (puntos brillantes deterministas a partir del frame)
    let frame = unsafe { state::STATE.frame };
    for i in 0..60usize {
        let pseudo = (i.wrapping_mul(73) ^ (frame as usize / 60).wrapping_mul(17)) as u32;
        let x = (pseudo as usize) % fb.width;
        let y = ((pseudo >> 8) as usize) % (fb.height / 2);
        fb.put_pixel(x, y, 0xFFCCDDFF);
    }
}

// ── Status bar (top, macOS-like) ───────────────────────────────────

fn draw_status_bar(fb: &Framebuffer) {
    fb.fill_rect(0, 0, fb.width, 28, palette::STATUS_BG);
    fb.fill_rect(0, 28, fb.width, 1, palette::WIN_BORDER);

    draw_text(fb, 14, 6, b"BMO", palette::TEXT_OK);
    draw_text(fb, 56, 6, b"Archivo  Editar  Ver  Ventana  Ayuda", palette::STATUS_FG);

    // Lado derecho: fps, frame, reloj
    let (h, m, s) = state::clock_hms();
    let mut buf = [0u8; 8];
    let clock_s = fmt_hms(&mut buf, h, m, s);
    let st = unsafe { &state::STATE };

    let mut fbuf = [0u8; 32];
    fbuf[0..4].copy_from_slice(b"fps ");
    let mut tmp = [0u8; 20];
    let fps_s = fmt_u64(&mut tmp, st.fps_avg as u64);
    let mut p = 4;
    for &b in fps_s.as_bytes() { fbuf[p] = b; p += 1; }
    fbuf[p] = b' '; p += 1;
    fbuf[p] = b'|'; p += 1;
    fbuf[p] = b' '; p += 1;
    let mut tmp2 = [0u8; 20];
    let frame_s = fmt_u64(&mut tmp2, st.frame);
    for &b in frame_s.as_bytes() { fbuf[p] = b; p += 1; }

    let fps_str = core::str::from_utf8(&fbuf[..p]).unwrap();
    let fps_x = fb.width - (p * 8) - (8 * 12) - 16;
    draw_text(fb, fps_x as u32, 6, fps_str.as_bytes(), palette::STATUS_DIM);

    let clk_x = fb.width - 8 * 8 - 16;
    draw_text(fb, clk_x as u32, 6, clock_s.as_bytes(), palette::STATUS_FG);
}

// ── Ventana con esquinas redondeadas + sombra + traffic-lights ─────

fn draw_window(
    fb: &Framebuffer,
    x: usize, y: usize, w: usize, h: usize,
    title: &[u8], lines: &[(&[u8], u32)],
    active: bool,
) {
    // Sombra (offset 6, 8)
    fb.fill_rounded_rect(x + 6, y + 8, w, h, 14, palette::WIN_SHADOW);

    // Cuerpo
    fb.fill_rounded_rect(x, y, w, h, 14, palette::WIN_BG);

    // Borde
    fb.draw_rect(x, y, w, h, palette::WIN_BORDER, 1);

    // Titlebar (32 px)
    let tb_color = if active { palette::WIN_TITLE_HL } else { palette::WIN_TITLE };
    // Rect interior simulando esquinas redondeadas arriba
    fb.fill_rect(x + 1, y + 1, w - 2, 32, tb_color);
    // Traffic lights (macOS)
    fb.fill_circle(x + 18, y + 16, 7, palette::TRAFFIC_R);
    fb.fill_circle(x + 38, y + 16, 7, palette::TRAFFIC_Y);
    fb.fill_circle(x + 58, y + 16, 7, palette::TRAFFIC_G);

    // Título centrado
    let title_x = x + (w - title.len() * 8) / 2;
    draw_text(fb, title_x as u32, (y + 8) as u32, title, palette::STATUS_FG);

    // Contenido
    let mut cy = y + 48;
    for (line, color) in lines {
        draw_text(fb, (x + 18) as u32, cy as u32, line, *color);
        cy += 20;
    }
}

// ── Dock (macOS bottom, semi-translucido visual) ───────────────────

const DOCK_ICON: usize = 56;
const DOCK_GAP: usize = 16;
const DOCK_PAD: usize = 12;

fn dock_geometry(fb: &Framebuffer) -> (usize, usize, usize, usize) {
    let inner_w = DOCK_SLOTS * DOCK_ICON + (DOCK_SLOTS - 1) * DOCK_GAP;
    let w = inner_w + 2 * DOCK_PAD;
    let h = DOCK_ICON + 2 * DOCK_PAD;
    let x = (fb.width - w) / 2;
    let y = fb.height - h - 16;
    (x, y, w, h)
}

fn icon_rect(fb: &Framebuffer, idx: usize) -> (usize, usize) {
    let (x, y, _, _) = dock_geometry(fb);
    let ix = x + DOCK_PAD + idx * (DOCK_ICON + DOCK_GAP);
    let iy = y + DOCK_PAD;
    (ix, iy)
}

fn draw_dock(fb: &Framebuffer) {
    let (dx, dy, dw, dh) = dock_geometry(fb);
    // Sombra dock
    fb.fill_rounded_rect(dx + 4, dy + 6, dw, dh, 22, palette::WIN_SHADOW);
    // Cuerpo dock
    fb.fill_rounded_rect(dx, dy, dw, dh, 22, palette::DOCK_BG);
    fb.draw_rect(dx, dy, dw, dh, palette::WIN_BORDER, 1);

    let st = unsafe { &state::STATE };

    // Detectar hover
    let mut hover: i32 = -1;
    for i in 0..DOCK_SLOTS {
        let (ix, iy) = icon_rect(fb, i);
        if (st.mouse_x as usize) >= ix && (st.mouse_x as usize) < ix + DOCK_ICON &&
           (st.mouse_y as usize) >= iy && (st.mouse_y as usize) < iy + DOCK_ICON {
            hover = i as i32;
        }
    }
    unsafe { state::STATE.dock_hover = hover; }

    // Detectar click → set active
    if (st.mouse_buttons & 1) != 0 && hover >= 0 {
        unsafe { state::STATE.dock_active = hover; }
    }

    for i in 0..DOCK_SLOTS {
        let (ix, iy) = icon_rect(fb, i);

        // Fondo hover (highlight detrás)
        if hover == i as i32 {
            fb.fill_rounded_rect(ix - 6, iy - 6, DOCK_ICON + 12, DOCK_ICON + 12, 12, palette::DOCK_HOVER);
        }

        // Icono (rect redondeado coloreado)
        fb.fill_rounded_rect(ix, iy, DOCK_ICON, DOCK_ICON, 12, palette::DOCK_ICONS[i]);
        fb.draw_rect(ix, iy, DOCK_ICON, DOCK_ICON, palette::WIN_BORDER, 1);

        // Indicador "activo": punto debajo
        if st.dock_active == i as i32 {
            let cx = ix + DOCK_ICON / 2;
            let cy = iy + DOCK_ICON + 6;
            fb.fill_circle(cx, cy, 3, palette::STATUS_FG);
        }
    }

    // Etiqueta hover (tooltip)
    if hover >= 0 {
        let labels: [&[u8]; 7] = [
            b"Archivos", b"Mensajes", b"Juegos", b"Web", b"Ajustes", b"Buscar", b"Papelera",
        ];
        let label = labels[hover as usize];
        let (ix, iy) = icon_rect(fb, hover as usize);
        let lx = ix + DOCK_ICON / 2 - (label.len() * 8) / 2;
        let ly = iy - 28;
        // fondo tooltip
        fb.fill_rounded_rect(lx.saturating_sub(8), ly.saturating_sub(4),
                             label.len() * 8 + 16, 22, 6, palette::WIN_BG);
        draw_text(fb, lx as u32, ly as u32, label, palette::STATUS_FG);
    }
}

// ── Cursor del ratón (flecha simple, 12×17) ────────────────────────

const CURSOR: [&[u8]; 17] = [
    b"X           ",
    b"XX          ",
    b"XOX         ",
    b"XOOX        ",
    b"XOOOX       ",
    b"XOOOOX      ",
    b"XOOOOOX     ",
    b"XOOOOOOX    ",
    b"XOOOOOOOX   ",
    b"XOOOOOOOOX  ",
    b"XOOOOOXXXXX ",
    b"XOOXOOX     ",
    b"XOX XOOX    ",
    b"XX  XOOX    ",
    b"     XOOX   ",
    b"      XOOX  ",
    b"       XXX  ",
];

fn draw_cursor(fb: &Framebuffer, x: i32, y: i32) {
    if x < 0 || y < 0 { return; }
    for (row, line) in CURSOR.iter().enumerate() {
        for (col, ch) in line.iter().enumerate() {
            let px = (x as usize) + col;
            let py = (y as usize) + row;
            match *ch {
                b'X' => fb.put_pixel(px, py, palette::CURSOR_SHADOW),
                b'O' => fb.put_pixel(px, py, palette::CURSOR_FG),
                _ => {}
            }
        }
    }
}

// ── Frame completo ─────────────────────────────────────────────────

pub fn render_frame() {
    state::tick();
    let Some(fb) = fb() else { return; };

    draw_wallpaper(&fb);
    draw_status_bar(&fb);

    let st = unsafe { &state::STATE };

    // Ventana 1 — terminal (activa)
    draw_window(&fb, 80, 80, 760, 460, b"BMO Terminal", &[
        (b"$ bmo > help", palette::TEXT_OK),
        (b"  desktop   -- compositor Win/Mac/Linux", palette::TEXT_PRIMARY),
        (b"  ring0     -- estado GDT/IDT/MSR", palette::TEXT_PRIMARY),
        (b"  user      -- spawn 'hello' Ring 3", palette::TEXT_PRIMARY),
        (b"$ bmo > _", palette::TEXT_OK),
    ], true);

    // Ventana 2 — datos.md viewer
    draw_window(&fb, 920, 80, 760, 460, b"Datos.md viewer", &[
        (b"FastOS / BMO  v0.9.0", palette::TEXT_PRIMARY),
        (b"Ring 0 + Ring 3 OK", palette::TEXT_INFO),
        (b"12 syscalls activos", palette::TEXT_INFO),
        (b"Compositor Ring 0 (slim)", palette::TEXT_PRIMARY),
        (b"RAMdisk + FileOpen/Read/Close", palette::TEXT_PRIMARY),
        (b"Mouse PS/2 + Beep PIT canal 2", palette::TEXT_PRIMARY),
    ], false);

    // Ventana 3 — info compositor
    let mut info_buf1 = [0u8; 32];
    let mut info_buf2 = [0u8; 32];
    info_buf1[..5].copy_from_slice(b"FPS: ");
    let mut tmp1 = [0u8; 20];
    let fps_s = fmt_u64(&mut tmp1, st.fps_avg as u64);
    let mut p = 5; for &b in fps_s.as_bytes() { info_buf1[p] = b; p += 1; }

    info_buf2[..8].copy_from_slice(b"Frame:  ");
    let mut tmp2 = [0u8; 20];
    let fr_s = fmt_u64(&mut tmp2, st.frame);
    let mut q = 8; for &b in fr_s.as_bytes() { info_buf2[q] = b; q += 1; }

    draw_window(&fb, 500, 580, 760, 380, b"Compositor Info", &[
        (&info_buf1[..p], palette::TEXT_INFO),
        (&info_buf2[..q], palette::TEXT_INFO),
        (b"Renderer: Ring 0 / Rust", palette::TEXT_PRIMARY),
        (b"Wallpaper: gradiente azul -> purpura", palette::TEXT_SECOND),
        (b"Ventanas: esquinas redondeadas + sombra", palette::TEXT_SECOND),
        (b"Dock: macOS-style + hover + click", palette::TEXT_SECOND),
        (b"Tooltips activos al pasar el cursor", palette::TEXT_SECOND),
        (b"ESC para salir.", palette::TEXT_OK),
    ], false);

    draw_dock(&fb);
    draw_cursor(&fb, st.mouse_x, st.mouse_y);
}
