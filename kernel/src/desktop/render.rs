//! Renderer del escritorio BMO — Ring 0. `render_frame()` pinta
//! un frame completo (wallpaper + status bar + ventanas dinámicas
//! + dock + cursor) y `handle_input()` procesa el ratón (drag,
//! close-button, dock launcher).

#![allow(dead_code)]

use crate::boot_info;
use crate::fb::Framebuffer;
use crate::font;
use super::state::{self, DesktopState, DOCK_SLOTS, MAX_WIN, WinInfo};

// ── Paleta ─────────────────────────────────────────────────────────
mod palette {
    pub const WALL_TOP:     u32 = 0xFF1E2A52;
    pub const WALL_BOT:     u32 = 0xFF4B1F70;
    pub const STATUS_BG:    u32 = 0xCC1A1B26;
    pub const STATUS_FG:    u32 = 0xFFE6EDF3;
    pub const STATUS_DIM:   u32 = 0xFFA0A8B8;

    pub const WIN_SHADOW:   u32 = 0xFF050810;
    pub const WIN_BG:       u32 = 0xFF21262D;
    pub const WIN_BORDER:   u32 = 0xFF3A4150;
    pub const WIN_TITLE:    u32 = 0xFF2D333C;
    pub const WIN_TITLE_HL: u32 = 0xFF0078D4;

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

    pub const DOCK_ICONS: [u32; 7] = [
        0xFFE0A458, 0xFF58A6FF, 0xFF76B900, 0xFFBC8CFF,
        0xFF56D4DD, 0xFFFF7B72, 0xFF7F848A,
    ];
}

// ── Catálogo de ventanas (título + contenido por title_id) ─────────

const TITLES: [&[u8]; 7] = [
    b"BMO Terminal",
    b"Datos.md viewer",
    b"Juegos",
    b"Web",
    b"Ajustes",
    b"Compositor Info",
    b"Papelera",
];

const DOCK_LABELS: [&[u8]; 7] = [
    b"BMO Terminal", b"Datos.md", b"Juegos", b"Web", b"Ajustes", b"Buscar", b"Papelera",
];

// Mapeo dock-slot → title_id
const DOCK_TO_TITLE: [u8; 7] = [0, 1, 2, 3, 4, 5, 6];

fn content_for(title_id: u8, fps: u32, frame: u64, buf1: &mut [u8; 48], buf2: &mut [u8; 48]) -> [(&'static [u8], u32); 8] {
    let pal = &palette::TEXT_PRIMARY;
    let _ = pal;
    match title_id {
        0 => [
            (b"$ bmo > help", palette::TEXT_OK),
            (b"  desktop   -- compositor Win/Mac/Linux", palette::TEXT_PRIMARY),
            (b"  ring0     -- estado GDT/IDT/MSR", palette::TEXT_PRIMARY),
            (b"  user      -- spawn 'hello' Ring 3", palette::TEXT_PRIMARY),
            (b"$ bmo > _", palette::TEXT_OK),
            (b"", palette::TEXT_PRIMARY),
            (b"Arrastra la barra para mover.", palette::TEXT_SECOND),
            (b"Click rojo para cerrar.", palette::TEXT_SECOND),
        ],
        1 => [
            (b"FastOS / BMO  v0.9.0", palette::TEXT_PRIMARY),
            (b"Ring 0 + Ring 3 OK", palette::TEXT_INFO),
            (b"13 syscalls activos", palette::TEXT_INFO),
            (b"Compositor Ring 0 (slim)", palette::TEXT_PRIMARY),
            (b"RAMdisk + FileOpen/Read/Close", palette::TEXT_PRIMARY),
            (b"Mouse PS/2 + Beep PIT", palette::TEXT_PRIMARY),
            (b"Drag-and-drop activo", palette::TEXT_OK),
            (b"Dock launcher activo", palette::TEXT_OK),
        ],
        2 => [
            (b"== Juegos ==", palette::TEXT_INFO),
            (b"Snake     (pendiente)", palette::TEXT_SECOND),
            (b"Tetris    (pendiente)", palette::TEXT_SECOND),
            (b"Pong      (pendiente)", palette::TEXT_SECOND),
            (b"DOOM      (4-6 sesiones)", palette::TEXT_SECOND),
            (b"", palette::TEXT_PRIMARY),
            (b"Ver ROADMAP_GAMES.md", palette::TEXT_OK),
            (b"", palette::TEXT_PRIMARY),
        ],
        3 => [
            (b"== Web ==", palette::TEXT_INFO),
            (b"barex::net listo:", palette::TEXT_PRIMARY),
            (b"  TCP/UDP/QUIC/TLS13", palette::TEXT_SECOND),
            (b"  HTTP3 + DNS", palette::TEXT_SECOND),
            (b"  ring buffers io_uring-style", palette::TEXT_SECOND),
            (b"", palette::TEXT_PRIMARY),
            (b"Falta: driver NIC real.", palette::TEXT_SECOND),
            (b"", palette::TEXT_PRIMARY),
        ],
        4 => [
            (b"== Ajustes ==", palette::TEXT_INFO),
            (b"CPU: AMD Ryzen 5 5600X", palette::TEXT_PRIMARY),
            (b"GPU: UEFI GOP framebuffer", palette::TEXT_PRIMARY),
            (b"RAM: BootInfo memory map", palette::TEXT_PRIMARY),
            (b"USB: keyboard + mouse + Redragon", palette::TEXT_PRIMARY),
            (b"Boot: UEFI puro (sin legacy)", palette::TEXT_PRIMARY),
            (b"BMO ABI: 7-GPR, 64B align", palette::TEXT_OK),
            (b"", palette::TEXT_PRIMARY),
        ],
        5 => {
            let mut p = 0;
            buf1[p..p+5].copy_from_slice(b"FPS: "); p += 5;
            let s = fmt_u64_into(&mut buf1[p..], fps as u64); p += s;
            let p1 = p;
            let mut q = 0;
            buf2[q..q+8].copy_from_slice(b"Frame:  "); q += 8;
            let s2 = fmt_u64_into(&mut buf2[q..], frame); q += s2;
            let p2 = q;
            // SAFETY: we only return the slices that we filled in this turn.
            // The lifetimes are tied to the buffers (caller controls them).
            let l1: &'static [u8] = unsafe { core::mem::transmute(&buf1[..p1]) };
            let l2: &'static [u8] = unsafe { core::mem::transmute(&buf2[..p2]) };
            [
                (l1, palette::TEXT_INFO),
                (l2, palette::TEXT_INFO),
                (b"Renderer: Ring 0 / Rust",      palette::TEXT_PRIMARY),
                (b"Wallpaper: gradiente azul -> purpura", palette::TEXT_SECOND),
                (b"Ventanas: rounded + shadow + traffic", palette::TEXT_SECOND),
                (b"Dock: macOS-style + click launch", palette::TEXT_SECOND),
                (b"Drag-and-drop sobre titlebar", palette::TEXT_OK),
                (b"ESC para salir.", palette::TEXT_OK),
            ]
        }
        6 => [
            (b"Papelera vacia.", palette::TEXT_SECOND),
            (b"", palette::TEXT_PRIMARY), (b"", palette::TEXT_PRIMARY),
            (b"", palette::TEXT_PRIMARY), (b"", palette::TEXT_PRIMARY),
            (b"", palette::TEXT_PRIMARY), (b"", palette::TEXT_PRIMARY),
            (b"", palette::TEXT_PRIMARY),
        ],
        _ => [
            (b"(ventana sin contenido)", palette::TEXT_SECOND),
            (b"", palette::TEXT_PRIMARY), (b"", palette::TEXT_PRIMARY),
            (b"", palette::TEXT_PRIMARY), (b"", palette::TEXT_PRIMARY),
            (b"", palette::TEXT_PRIMARY), (b"", palette::TEXT_PRIMARY),
            (b"", palette::TEXT_PRIMARY),
        ],
    }
}

// ── Framebuffer helpers ────────────────────────────────────────────

fn fb() -> Option<Framebuffer> {
    let (addr, w, h, s) = unsafe {
        (boot_info::FB_ADDR, boot_info::FB_WIDTH, boot_info::FB_HEIGHT, boot_info::FB_STRIDE)
    };
    if addr == 0 || w == 0 { return None; }
    Some(Framebuffer::new(addr, (s as u64) * 4, w, h))
}

pub(super) fn draw_text(fb: &Framebuffer, x: u32, y: u32, text: &[u8], color: u32) {
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

fn fmt_u64_into(buf: &mut [u8], mut v: u64) -> usize {
    if v == 0 { buf[0] = b'0'; return 1; }
    let mut tmp = [0u8; 20]; let mut i = 0;
    while v > 0 { tmp[i] = b'0' + (v % 10) as u8; v /= 10; i += 1; }
    for k in 0..i { buf[k] = tmp[i - 1 - k]; }
    i
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
    // Wallpaper liso: gradiente vertical sólido. Antes había un loop "ghost"
    // que pintaba 60 pseudo-estrellas redibujadas en posiciones distintas
    // cada 60 frames — provocaba parpadeo aleatorio sin sentido.
    fb.gradient_v(0, 0, fb.width, fb.height, palette::WALL_TOP, palette::WALL_BOT);
}

// ── Status bar ─────────────────────────────────────────────────────

fn draw_status_bar(fb: &Framebuffer) {
    fb.fill_rect(0, 0, fb.width, 28, palette::STATUS_BG);
    fb.fill_rect(0, 28, fb.width, 1, palette::WIN_BORDER);

    draw_text(fb, 14, 6, b"BMO", palette::TEXT_OK);
    draw_text(fb, 56, 6, b"Archivo  Editar  Ver  Ventana  Ayuda", palette::STATUS_FG);

    let (h, m, sec) = state::clock_hms();
    let mut buf = [0u8; 8];
    let clock_s = fmt_hms(&mut buf, h, m, sec);
    let st = unsafe { &state::STATE };

    let mut fbuf = [0u8; 48];
    fbuf[0..4].copy_from_slice(b"fps ");
    let mut p = 4;
    // Usamos `fps_display` (snapshotted cada 30 frames) en vez de `fps_avg`
    // para que el número no parpadee letra a letra cada frame.
    p += fmt_u64_into(&mut fbuf[p..], st.fps_display as u64);
    fbuf[p] = b' '; p += 1; fbuf[p] = b'|'; p += 1; fbuf[p] = b' '; p += 1;
    p += fmt_u64_into(&mut fbuf[p..], st.frame);
    let fps_str = &fbuf[..p];

    let fps_x = fb.width - (p * 8) - (8 * 12) - 16;
    draw_text(fb, fps_x as u32, 6, fps_str, palette::STATUS_DIM);

    let clk_x = fb.width - 8 * 8 - 16;
    draw_text(fb, clk_x as u32, 6, clock_s.as_bytes(), palette::STATUS_FG);
}

// ── Ventana ────────────────────────────────────────────────────────

fn draw_window(fb: &Framebuffer, w: &WinInfo, active: bool) {
    let (x, y, ww, wh) = (w.x.max(0) as usize, w.y.max(0) as usize, w.w.max(0) as usize, w.h.max(0) as usize);
    if ww == 0 || wh == 0 { return; }

    // Sombra
    fb.fill_rounded_rect(x + 6, y + 8, ww, wh, 14, palette::WIN_SHADOW);
    fb.fill_rounded_rect(x, y, ww, wh, 14, palette::WIN_BG);
    fb.draw_rect(x, y, ww, wh, palette::WIN_BORDER, 1);

    let tb_color = if active { palette::WIN_TITLE_HL } else { palette::WIN_TITLE };
    fb.fill_rect(x + 1, y + 1, ww - 2, 32, tb_color);

    // Traffic lights
    fb.fill_circle(x + 18, y + 16, 7, palette::TRAFFIC_R);
    fb.fill_circle(x + 38, y + 16, 7, palette::TRAFFIC_Y);
    fb.fill_circle(x + 58, y + 16, 7, palette::TRAFFIC_G);

    // Título centrado
    let title = TITLES[w.title_id as usize];
    let title_x = x + (ww.saturating_sub(title.len() * 8)) / 2;
    draw_text(fb, title_x as u32, (y + 8) as u32, title, palette::STATUS_FG);

    // Contenido
    let st = unsafe { &state::STATE };
    let mut buf1 = [0u8; 48];
    let mut buf2 = [0u8; 48];
    let lines = content_for(w.title_id, st.fps_display, st.frame, &mut buf1, &mut buf2);
    let mut cy = y + 48;
    for (line, color) in lines.iter() {
        if cy + 16 > y + wh { break; }
        draw_text(fb, (x + 18) as u32, cy as u32, line, *color);
        cy += 20;
    }
}

// ── Dock ───────────────────────────────────────────────────────────

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
    fb.fill_rounded_rect(dx + 4, dy + 6, dw, dh, 22, palette::WIN_SHADOW);
    fb.fill_rounded_rect(dx, dy, dw, dh, 22, palette::DOCK_BG);
    fb.draw_rect(dx, dy, dw, dh, palette::WIN_BORDER, 1);

    let st = unsafe { &state::STATE };
    let hover = st.dock_hover;

    for i in 0..DOCK_SLOTS {
        let (ix, iy) = icon_rect(fb, i);
        if hover == i as i32 {
            fb.fill_rounded_rect(ix - 6, iy - 6, DOCK_ICON + 12, DOCK_ICON + 12, 12, palette::DOCK_HOVER);
        }
        fb.fill_rounded_rect(ix, iy, DOCK_ICON, DOCK_ICON, 12, palette::DOCK_ICONS[i]);
        fb.draw_rect(ix, iy, DOCK_ICON, DOCK_ICON, palette::WIN_BORDER, 1);

        // Indicador "abierta": dot blanco si la ventana de ese title_id está open
        let mut is_open = false;
        for j in 0..MAX_WIN {
            if st.windows[j].open && st.windows[j].title_id == DOCK_TO_TITLE[i] {
                is_open = true; break;
            }
        }
        if is_open {
            let cx = ix + DOCK_ICON / 2;
            let cy = iy + DOCK_ICON + 6;
            fb.fill_circle(cx, cy, 3, palette::STATUS_FG);
        }
    }

    if hover >= 0 {
        let label = DOCK_LABELS[hover as usize];
        let (ix, iy) = icon_rect(fb, hover as usize);
        let lx = ix + DOCK_ICON / 2 - (label.len() * 8) / 2;
        let ly = iy.saturating_sub(28);
        fb.fill_rounded_rect(lx.saturating_sub(8), ly.saturating_sub(4),
                             label.len() * 8 + 16, 22, 6, palette::WIN_BG);
        draw_text(fb, lx as u32, ly as u32, label, palette::STATUS_FG);
    }
}

// ── Cursor ─────────────────────────────────────────────────────────

const CURSOR: [&[u8]; 17] = [
    b"X           ", b"XX          ", b"XOX         ", b"XOOX        ",
    b"XOOOX       ", b"XOOOOX      ", b"XOOOOOX     ", b"XOOOOOOX    ",
    b"XOOOOOOOX   ", b"XOOOOOOOOX  ", b"XOOOOOXXXXX ", b"XOOXOOX     ",
    b"XOX XOOX    ", b"XX  XOOX    ", b"     XOOX   ", b"      XOOX  ",
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

// ── Input handling — drag, close, dock launcher ────────────────────

fn point_in_rect(px: i32, py: i32, x: i32, y: i32, w: i32, h: i32) -> bool {
    px >= x && px < x + w && py >= y && py < y + h
}

fn point_in_circle(px: i32, py: i32, cx: i32, cy: i32, r: i32) -> bool {
    let dx = px - cx; let dy = py - cy;
    dx * dx + dy * dy <= r * r
}

fn handle_input(fb: &Framebuffer) {
    let st: &mut DesktopState = unsafe { &mut state::STATE };
    let mx = st.mouse_x; let my = st.mouse_y;

    // 1) Si estamos en drag → mover ventana, terminar drag al soltar
    if st.drag_idx >= 0 {
        if state::mouse_left_held() {
            let idx = st.drag_idx as usize;
            if idx < MAX_WIN && st.windows[idx].open {
                st.windows[idx].x = mx - st.drag_dx;
                st.windows[idx].y = my - st.drag_dy;
                // Clamp dentro de la pantalla
                let maxx = (fb.width as i32) - st.windows[idx].w;
                let maxy = (fb.height as i32) - st.windows[idx].h;
                st.windows[idx].x = st.windows[idx].x.clamp(0, maxx.max(0));
                st.windows[idx].y = st.windows[idx].y.clamp(28, maxy.max(28));
            }
        } else {
            st.drag_idx = -1;
        }
        return;
    }

    // Calcular dock hover SIEMPRE (no sólo en click)
    let mut hover: i32 = -1;
    for i in 0..DOCK_SLOTS {
        let (ix, iy) = icon_rect(fb, i);
        if mx as usize >= ix && (mx as usize) < ix + DOCK_ICON &&
           my as usize >= iy && (my as usize) < iy + DOCK_ICON {
            hover = i as i32;
        }
    }
    st.dock_hover = hover;

    // 2) ¿click left edge? buscar target
    if !state::mouse_left_pressed() { return; }

    // 2a) Dock icon → abrir ventana
    if hover >= 0 {
        state::open_window(DOCK_TO_TITLE[hover as usize]);
        st.dock_active = hover;
        return;
    }

    // 2b) Iterar ventanas en orden de focus (focus primero) — el orden de
    // dibujo es focus al final (sobre todo), pero el click va al de arriba.
    let order = z_order_top_first();
    for &idx in order.iter() {
        let w = st.windows[idx];
        if !w.open { continue; }

        // Close button (traffic light rojo)
        if point_in_circle(mx, my, w.x + 18, w.y + 16, 9) {
            state::close_window(idx);
            return;
        }

        // Titlebar → drag
        if point_in_rect(mx, my, w.x + 80, w.y, w.w - 80, 32) ||
           point_in_rect(mx, my, w.x, w.y, w.w, 32) && !point_in_circle(mx, my, w.x + 18, w.y + 16, 9)
                                                    && !point_in_circle(mx, my, w.x + 38, w.y + 16, 9)
                                                    && !point_in_circle(mx, my, w.x + 58, w.y + 16, 9) {
            st.focus = idx as i32;
            st.drag_idx = idx as i32;
            st.drag_dx = mx - w.x;
            st.drag_dy = my - w.y;
            return;
        }

        // Body → sólo focus
        if point_in_rect(mx, my, w.x, w.y, w.w, w.h) {
            st.focus = idx as i32;
            return;
        }
    }
}

/// Orden de top→bottom para hit-testing. Focus arriba, luego el resto en
/// orden de índice ascendente. Como `MAX_WIN = 8` cabe en stack.
fn z_order_top_first() -> [usize; MAX_WIN] {
    let st = unsafe { &state::STATE };
    let mut out = [0usize; MAX_WIN];
    let mut p = 0;
    if st.focus >= 0 && (st.focus as usize) < MAX_WIN {
        out[p] = st.focus as usize; p += 1;
    }
    for i in 0..MAX_WIN {
        if i as i32 == st.focus { continue; }
        if p < MAX_WIN { out[p] = i; p += 1; }
    }
    out
}

// ── Frame ──────────────────────────────────────────────────────────

pub fn render_frame() {
    state::tick();
    let Some(fb) = fb() else { return; };

    handle_input(&fb);

    draw_wallpaper(&fb);
    draw_status_bar(&fb);

    // Ventanas: focus al final (encima)
    let st = unsafe { &state::STATE };
    for i in 0..MAX_WIN {
        if i as i32 == st.focus { continue; }
        if st.windows[i].open {
            draw_window(&fb, &st.windows[i], false);
        }
    }
    if st.focus >= 0 && (st.focus as usize) < MAX_WIN {
        let w = st.windows[st.focus as usize];
        if w.open { draw_window(&fb, &w, true); }
    }

    draw_dock(&fb);
    draw_cursor(&fb, st.mouse_x, st.mouse_y);
}
