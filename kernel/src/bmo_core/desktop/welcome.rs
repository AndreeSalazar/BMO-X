//! FastOS/BMO v1.8.8
//!
//! Desarrolado por Salazar.
//!
//! v1.7.1 — Welcome screen profesional, dark, elegante.
//!
//! Cambios vs v1.6.x:
//!   * Wallpaper procedural compartido con el desktop (mesh + aurora + grid).
//!   * Card con glass (tinte negro 40% sobre el wallpaper).
//!   * Tipografía jerárquica: title 3× → subtitle 2× → body 1×.
//!   * Acentos mint teal + detalle gold; consistencia total con el desktop.
//!   * Prompt rediseñado: pill con borde neón + caret cuadrado.
//!   * Watermark "Powered by Eddi Andreé Salazar Matos" en una línea sutil.
//!   * Footer con build / ring / arch en un strip inferior.
//!
//! Comandos aceptados: Run, Hello, Ring3, Nexo, Test, Reboot (sin cambios).

use crate::bmo_core::ui::fb::Framebuffer;
use crate::bmo_core::ui::font;
use super::commands::{eq_ci, trim, should_enter_desktop, enter_desktop, nexo_test_compile};
use super::sound;
use super::theme;
use super::wallpaper;

const MAX_INPUT: usize = 32;
static mut INPUT_BUF: [u8; MAX_INPUT] = [0; MAX_INPUT];
static mut INPUT_LEN: usize = 0;
static mut HINT_TIMER: u32 = 0;
static mut HINT_MSG: &[u8] = b"";

static mut KBD_LSHIFT: bool = false;
static mut KBD_RSHIFT: bool = false;
static mut KBD_CAPS:   bool = false;

#[inline]
fn shift_held() -> bool { unsafe { KBD_LSHIFT || KBD_RSHIFT } }

#[inline]
fn caps_on() -> bool { unsafe { KBD_CAPS } }

fn translate_scancode(sc: u8) -> Option<u8> {
    let (base, shifted): (u8, u8) = match sc {
        0x29 => (b'`',  b'~'),
        0x02 => (b'1',  b'!'),
        0x03 => (b'2',  b'@'),
        0x04 => (b'3',  b'#'),
        0x05 => (b'4',  b'$'),
        0x06 => (b'5',  b'%'),
        0x07 => (b'6',  b'^'),
        0x08 => (b'7',  b'&'),
        0x09 => (b'8',  b'*'),
        0x0A => (b'9',  b'('),
        0x0B => (b'0',  b')'),
        0x0C => (b'-',  b'_'),
        0x0D => (b'=',  b'+'),
        0x0F => (b'\t', b'\t'),
        0x10 => (b'q',  b'Q'),
        0x11 => (b'w',  b'W'),
        0x12 => (b'e',  b'E'),
        0x13 => (b'r',  b'R'),
        0x14 => (b't',  b'T'),
        0x15 => (b'y',  b'Y'),
        0x16 => (b'u',  b'U'),
        0x17 => (b'i',  b'I'),
        0x18 => (b'o',  b'O'),
        0x19 => (b'p',  b'P'),
        0x1A => (b'[',  b'{'),
        0x1B => (b']',  b'}'),
        0x1E => (b'a',  b'A'),
        0x1F => (b's',  b'S'),
        0x20 => (b'd',  b'D'),
        0x21 => (b'f',  b'F'),
        0x22 => (b'g',  b'G'),
        0x23 => (b'h',  b'H'),
        0x24 => (b'j',  b'J'),
        0x25 => (b'k',  b'K'),
        0x26 => (b'l',  b'L'),
        0x27 => (164,  165),
        0x28 => (b'\'', b'"'),
        0x2B => (b'\\', b'|'),
        0x2C => (b'z',  b'Z'),
        0x2D => (b'x',  b'X'),
        0x2E => (b'c',  b'C'),
        0x2F => (b'v',  b'V'),
        0x30 => (b'b',  b'B'),
        0x31 => (b'n',  b'N'),
        0x32 => (b'm',  b'M'),
        0x33 => (b',',  b'<'),
        0x34 => (b'.',  b'>'),
        0x35 => (b'/',  b'?'),
        0x39 => (b' ',  b' '),
        0x1C => (b'\n', b'\n'),
        0x0E => (8,     8),
        _ => return None,
    };
    let is_letter = base.is_ascii_lowercase();
    let upper = if is_letter { shift_held() ^ caps_on() } else { shift_held() };
    Some(if upper { shifted } else { base })
}

fn process_scancode(raw: u8) -> Option<u8> {
    let released = (raw & 0x80) != 0;
    let sc = raw & 0x7F;
    match sc {
        0x2A => { unsafe { KBD_LSHIFT = !released; } return None; }
        0x36 => { unsafe { KBD_RSHIFT = !released; } return None; }
        0x3A => {
            if !released { unsafe { KBD_CAPS = !KBD_CAPS; } }
            return None;
        }
        0x1D | 0x38 => return None,
        _ => {}
    }
    if released { return None; }
    translate_scancode(sc)
}

fn fb() -> Option<Framebuffer> {
    let (addr, w, h, s) = unsafe {
        (crate::boot::info::FB_ADDR, crate::boot::info::FB_WIDTH, crate::boot::info::FB_HEIGHT, crate::boot::info::FB_STRIDE)
    };
    if addr == 0 || w == 0 { return None; }
    Some(Framebuffer::new(addr, (s as u64) * 4, w, h))
}

fn draw_text_scaled(fb: &Framebuffer, x: u32, y: u32, text: &[u8], color: u32, scale: u32) {
    let mut cx = x as usize;
    let cy = y as usize;
    let s = scale.max(1) as usize;
    let gw = 8 * s;
    let gh = 16 * s;
    for &ch in text {
        if cx + gw > fb.width || cy + gh > fb.height { break; }
        let glyph = font::get_glyph(ch);
        for py in 0..16 {
            let row = glyph[py];
            for px in 0..8 {
                if (row & (0x80 >> px)) != 0 {
                    for ry in 0..s {
                        for rx in 0..s {
                            fb.put_pixel(cx + px * s + rx, cy + py * s + ry, color);
                        }
                    }
                }
            }
        }
        cx += gw;
    }
    // SFENCE: flush the framebuffer's write-combining buffer so the
    // scattered put_pixel writes aren't reordered relative to subsequent
    // fill_rect calls. v1.8.8: critical for >=1080p GOP displays.
    unsafe { core::arch::asm!("sfence"); }
}

#[inline]
fn draw_text(fb: &Framebuffer, x: u32, y: u32, text: &[u8], color: u32) {
    draw_text_scaled(fb, x, y, text, color, 1);
}

const CARD_W: usize = 1180;
const CARD_H: usize = 580;

fn card_geom(fb: &Framebuffer) -> (usize, usize) {
    let cx = (fb.width - CARD_W) / 2;
    let cy = (fb.height - CARD_H) / 2;
    (cx, cy)
}

fn prompt_rect(fb: &Framebuffer) -> (usize, usize, usize, usize) {
    let (cx, cy) = card_geom(fb);
    let pw = 760usize;
    let ph = 64usize;
    let px = cx + (CARD_W - pw) / 2;
    let py = cy + CARD_H - ph - 110;
    (px, py, pw, ph)
}

fn caret_pos(fb: &Framebuffer) -> (usize, usize) {
    let (px, py, _, _) = prompt_rect(fb);
    let len = unsafe { INPUT_LEN };
    let cx = px + 20 + 8 * 2 * 2 + len * 16;
    let cy = py + 18;
    (cx, cy)
}

fn paint_caret(fb: &Framebuffer, on: bool) {
    let (cx, cy) = caret_pos(fb);
    let color = if on { theme::MINT_SOFT } else { theme::PROMPT_BG };
    fb.fill_rect(cx, cy, 10, 28, color);
}

fn render(fb: &Framebuffer) {
    let time = crate::cpu::rdtsc();
    wallpaper::draw(fb, time);

    let (cx, cy) = card_geom(fb);

    fb.fill_rounded_rect(cx + 16, cy + 22, CARD_W, CARD_H, 28, theme::CARD_SHADOW);
    fb.fill_rounded_rect(cx, cy, CARD_W, CARD_H, 28, theme::SURFACE_0);
    fb.draw_rect(cx + 1, cy + 1, CARD_W - 2, CARD_H - 2, theme::SURFACE_LINE, 1);

    fb.draw_rect(
        cx.saturating_sub(6), cy.saturating_sub(6),
        CARD_W + 12, CARD_H + 12,
        theme::NEON_OUTER, 1,
    );
    fb.draw_rect(
        cx.saturating_sub(3), cy.saturating_sub(3),
        CARD_W + 6, CARD_H + 6,
        theme::NEON_MID, 1,
    );
    fb.draw_rect(cx, cy, CARD_W, CARD_H, theme::SURFACE_BORDER, 2);

    fb.fill_rect(cx + 24, cy + 2, CARD_W - 48, 1, theme::GLASS_HIGHLIGHT);

    fb.fill_rect(cx + 24, cy + 28, CARD_W - 48, 3, theme::MINT);
    fb.fill_rect(cx + 24, cy + 33, CARD_W - 48, 1, theme::MINT_DEEP);
    fb.fill_rect(cx + 24, cy + 36, CARD_W - 48, 1, theme::NEON_INNER);

    let title_left  = b"FastOS";
    let title_dash  = b"-";
    let title_right = b"BMO";
    let scale_t = 3u32;
    let lw_l = title_left.len()  * 8 * scale_t as usize;
    let lw_d = title_dash.len()  * 8 * scale_t as usize;
    let lw_r = title_right.len() * 8 * scale_t as usize;
    let total = lw_l + lw_d + lw_r + 24;
    let title_y = cy + 64;
    let mut tx = cx + (CARD_W - total) / 2;
    draw_text_scaled(fb, (tx + 3) as u32, (title_y + 3) as u32, title_left,  0xFF020610, scale_t);
    draw_text_scaled(fb, (tx + 3) as u32, (title_y + 3) as u32, title_dash,  0xFF020610, scale_t);
    draw_text_scaled(fb, (tx + 3) as u32, (title_y + 3) as u32, title_right, 0xFF020610, scale_t);
    draw_text_scaled(fb, tx as u32, title_y as u32, title_left,  theme::TITLE, scale_t);
    tx += lw_l + 12;
    draw_text_scaled(fb, tx as u32, title_y as u32, title_dash,  theme::MINT, scale_t);
    tx += lw_d + 12;
    draw_text_scaled(fb, tx as u32, title_y as u32, title_right, theme::TITLE, scale_t);

    let sub = b"Bare Metal Orchestrator";
    let sw = sub.len() * 8 * 2;
    let sx = cx + (CARD_W - sw) / 2;
    draw_text_scaled(fb, sx as u32, (cy + 134) as u32, sub, theme::SUBTITLE, 2);

    let ver = b"v1.7.1   ::   Ring 0 + Ring 3   ::   Dark Elegance";
    let vw = ver.len() * 8;
    let vx = cx + (CARD_W - vw) / 2;
    fb.fill_rounded_rect(vx - 16, cy + 174, vw + 32, 24, 8, theme::MINT_PILL_BG);
    draw_text(fb, vx as u32, (cy + 180) as u32, ver, theme::GOLD);

    let pb_x = cx + 90;
    let pb_y = cy + 220;
    let pb_w = CARD_W - 180;
    let pb_h = 12;
    fb.fill_rounded_rect(pb_x - 2, pb_y - 2, pb_w + 4, pb_h + 4, 6, 0xFF0A1018);
    // v1.8.14: progress bar minimal. Solo dibujamos 1 segmento activo
    // (GOLD) sin badges, sin labels, sin shimmer. Costo: ~3 fills.
    let seg_w = pb_w;
    let seg_gap = 6usize;
    let sxi = pb_x + seg_gap / 2;
    let swi = seg_w - seg_gap;
    fb.fill_rounded_rect(sxi, pb_y, swi, pb_h, 5, theme::GOLD);

    // v1.8.14: badges minimal. Solo un status line centrado.
    let by0 = cy + 274;
    let status = b"FastOS / BMO :: Ryzen 5 5600X :: Ring 0 + Ring 3";
    let sw = status.len() * 8;
    let sx = cx + (CARD_W - sw) / 2;
    draw_text(fb, sx as u32, by0 as u32, status, theme::MINT);

    let hint = b">>  Type  Run  and press  Enter  to enter the Ring 0 desktop";
    let hx_pos;
    {
        let (px, py, _, _) = prompt_rect(fb);
        hx_pos = px;
        draw_text(fb, px as u32, (py - 30) as u32, hint, theme::SUBTITLE);
    }

    let (px, py, pw, ph) = prompt_rect(fb);
    fb.draw_rect(
        px.saturating_sub(2), py.saturating_sub(2),
        pw + 4, ph + 4,
        theme::NEON_INNER, 1,
    );
    fb.fill_rounded_rect(px, py, pw, ph, 12, theme::PROMPT_BG);
    fb.draw_rect(px, py, pw, ph, theme::MINT, 2);
    fb.fill_rect(px + 2, py + 2, pw - 4, 1, theme::MINT_SOFT);

    draw_text_scaled(fb, (px + 20) as u32, (py + 20) as u32, b"> ", theme::MINT, 2);
    let len = unsafe { INPUT_LEN };
    if len > 0 {
        let txt = unsafe { &INPUT_BUF[..len] };
        draw_text_scaled(fb, (px + 20 + 32) as u32, (py + 20) as u32, txt, theme::PROMPT_FG, 2);
    } else {
        draw_text_scaled(fb, (px + 20 + 32) as u32, (py + 20) as u32, b"Run", theme::PLACEHOLDER, 2);
    }

    let (timer, msg) = unsafe { (HINT_TIMER, HINT_MSG) };
    if timer > 0 && !msg.is_empty() {
        draw_text(fb, hx_pos as u32, (py + ph + 14) as u32, msg, theme::HINT);
    }

    let btn_w = 130usize;
    let btn_h = 64usize;
    let bx = px + pw - btn_w;
    let by = py;
    let btn_active = unsafe { INPUT_LEN > 0 && {
        let s = &INPUT_BUF[..INPUT_LEN];
        eq_ci(s, b"run")
    } };
    let btn_color = if btn_active { theme::ORANGE_HI } else { theme::ORANGE };
    fb.draw_rect(
        bx.saturating_sub(2), by.saturating_sub(2),
        btn_w + 4, btn_h + 4,
        theme::NEON_INNER, 1,
    );
    fb.fill_rounded_rect(bx, by, btn_w, btn_h, 12, btn_color);
    fb.draw_rect(bx, by, btn_w, btn_h, theme::SURFACE_BORDER, 1);
    fb.fill_rect(bx + 2, by + 2, btn_w - 4, 1, theme::GLASS_HIGHLIGHT);
    draw_text_scaled(fb, (bx + 14) as u32, (by + 20) as u32, b"\x10", theme::TITLE, 2);
    let lbl = b"RUN";
    let lw = lbl.len() * 8 * 2;
    let lx = bx + 40 + (btn_w - 40 - lw) / 2;
    draw_text_scaled(fb, lx as u32, (by + 20) as u32, lbl, theme::TITLE, 2);

    let author = b"\x95  Powered by Eddi Andre\x82 Salazar Matos";
    let aw = author.len() * 8;
    let ax = cx + (CARD_W - aw) / 2;
    fb.fill_rounded_rect(ax - 18, cy + CARD_H - 54, aw + 36, 22, 8, theme::MINT_PILL_BG);
    draw_text(fb, ax as u32, (cy + CARD_H - 50) as u32, author, theme::MINT);

    let build = b"build 1.7.1  ::  AMD64  ::  BMO ABI v0.4.0  ::  Ring 0 + Ring 3";
    let bw2 = build.len() * 8;
    let bx2 = cx + (CARD_W - bw2) / 2;
    draw_text(fb, bx2 as u32, (cy + CARD_H - 22) as u32, build, theme::DIM);
}

fn render_safe(fb: &Framebuffer) {
    fb.clear(0xFF07111F);
    fb.fill_rect(0, 0, fb.width, 42, 0xFF101820);
    draw_text(fb, 14, 13, b"FastOS / BMO  ::  v1.7.1  ::  SAFE WELCOME", 0xFFE6EDF3);
    draw_text(fb, 14, 58, b"GOP framebuffer OK. Storage/NIC deferred for stable boot.", 0xFF76B900);
    draw_text(fb, 14, 82, b"Run + Enter: desktop  ::  F9: diag HUD", 0xFF8B949E);

    let y = (fb.height / 2).saturating_sub(32);
    fb.fill_rect(14, y, fb.width.saturating_sub(28), 72, 0xFF0D1117);
    fb.fill_rect(14, y, fb.width.saturating_sub(28), 2, 0xFF58A6FF);
    draw_text(fb, 30, (y + 16) as u32, b"> ", 0xFF58A6FF);

    let len = unsafe { INPUT_LEN };
    if len > 0 {
        let txt = unsafe { &INPUT_BUF[..len] };
        draw_text(fb, 54, (y + 16) as u32, txt, 0xFFE6EDF3);
    } else {
        draw_text(fb, 54, (y + 16) as u32, b"type Run", 0xFF30363D);
    }

    let (timer, msg) = unsafe { (HINT_TIMER, HINT_MSG) };
    if timer > 0 && !msg.is_empty() {
        draw_text(fb, 30, (y + 50) as u32, msg, 0xFFFFBD2E);
    }
}

static mut DIRTY: bool = true;
static mut LAST_BLINK_ON: bool = false;
static mut LAST_HINT_TIMER: u32 = 0;

#[inline]
fn mark_dirty() { unsafe { DIRTY = true; } }

fn show_hint(msg: &'static [u8]) {
    unsafe {
        HINT_MSG = msg;
        HINT_TIMER = 120;
    }
    mark_dirty();
}

fn run_phase_self_test(n: u8) {
    use crate::boot::phases::report_self_test;
    let report = match n {
        0 => crate::boot::phases::p0_arch::self_test(),
        1 => crate::boot::phases::p1_mem::self_test(),
        2 => crate::boot::phases::p2_dev::self_test(),
        3 => crate::boot::phases::p3_proc::self_test(),
        4 => crate::boot::phases::p4_bmo::self_test(),
        5 => crate::boot::phases::p5_user::self_test(),
        _ => {
            crate::cabina::warn("welcome", "Unknown phase index");
            return;
        }
    };
    crate::cabina::info("welcome", "Phase self-test");
    report_self_test(&report);
}

fn run_phase_self_test_ring3() {
    use crate::boot::phases::report_self_test;
    let report = crate::boot::phases::p0_arch::self_test();
    crate::cabina::info("welcome", "Ring 3 self-test (using p0_arch)");
    report_self_test(&report);
}

fn run_test_all_phases() {
    use crate::boot::phases::report_self_test;
    let reports = [
        crate::boot::phases::p0_arch::self_test(),
        crate::boot::phases::p1_mem::self_test(),
        crate::boot::phases::p2_dev::self_test(),
        crate::boot::phases::p3_proc::self_test(),
        crate::boot::phases::p4_bmo::self_test(),
        crate::boot::phases::p5_user::self_test(),
    ];
    crate::cabina::info("welcome", "All-phase self-test");
    for r in &reports {
        report_self_test(r);
    }
    let total_failed: usize = reports.iter().map(|r| r.failed_count()).sum();
    if total_failed == 0 {
        sound::beep(880, 60);
        sound::beep(1175, 60);
        show_hint(b"All phase self-tests PASSED.");
    } else {
        show_hint(b"Self-test failures - see serial log.");
    }
}

fn process_enter() {
    let cmd = unsafe { &INPUT_BUF[..INPUT_LEN] };
    let trimmed_cmd = trim(cmd);

    if trimmed_cmd.is_empty() {
        show_hint(b"Type Run and press Enter.");
    } else if should_enter_desktop(trimmed_cmd) {
        enter_desktop();
        crate::cabina::info("welcome", "Desktop returned; re-entering welcome");
        show_hint(b"Desktop returned unexpectedly. Type test for diagnostics.");
    } else if eq_ci(trimmed_cmd, b"hello") {
        crate::cabina::info("welcome", "Hello command accepted; preparing Ring 3 test");
        sound::beep(440, 80);
        crate::proc::user_init::spawn_hello();
    } else if eq_ci(trimmed_cmd, b"ring3") {
        crate::cabina::info("welcome", "Ring3 command accepted; testing Ring 0 -> Ring 3");
        sound::beep(440, 80);
        crate::proc::user_init::spawn_hello();
    } else if eq_ci(trimmed_cmd, b"reboot") {
        crate::cabina::warn("welcome", "Reboot command accepted");
        unsafe { core::arch::asm!("out dx, al", in("dx") 0x64u16, in("al") 0xFEu8); }
    } else if eq_ci(trimmed_cmd, b"nexo") {
        crate::cabina::info("welcome", "NEXO compiler test - compiling hello program");
        nexo_test_compile();
    } else if eq_ci(trimmed_cmd, b"test desktop") {
        crate::cabina::info("welcome", "test desktop: rendering single frame");
        crate::dev::console::serial_write("[welcome] test desktop: calling render_frame()\n");
        crate::bmo_core::desktop::render::render_frame();
        crate::dev::console::serial_write("[welcome] test desktop: render_frame() returned OK\n");
        crate::cabina::info("welcome", "test desktop: render_frame OK");
    } else if eq_ci(trimmed_cmd, b"test") {
        run_test_all_phases();
    } else if eq_ci(trimmed_cmd, b"test phase 0") {
        run_phase_self_test(0);
    } else if eq_ci(trimmed_cmd, b"test phase 1") {
        run_phase_self_test(1);
    } else if eq_ci(trimmed_cmd, b"test phase 2") {
        run_phase_self_test(2);
    } else if eq_ci(trimmed_cmd, b"test phase 3") {
        run_phase_self_test(3);
    } else if eq_ci(trimmed_cmd, b"test phase 4") {
        run_phase_self_test(4);
    } else if eq_ci(trimmed_cmd, b"test phase 5") {
        run_phase_self_test(5);
    } else if eq_ci(trimmed_cmd, b"test ring3") {
        run_phase_self_test_ring3();
    } else {
        crate::cabina::warn("welcome", "Unknown command at welcome prompt");
        show_hint(b"Commands: Run, Hello, Ring3, Nexo, Test, Reboot.");
    }
    unsafe { INPUT_LEN = 0; }
    mark_dirty();
}

pub fn run() -> ! {
    crate::dev::console::serial_write("[welcome] v1.7.1 Pantalla de bienvenida activa.\n");

    if let Some(fb) = fb() {
        crate::dev::console::serial_write("[welcome] fb: clearing to black\n");
        fb.fill_rect(0, 0, fb.width, fb.height, 0xFF000000);
        crate::dev::console::serial_write("[welcome] fb: cleared\n");
    } else {
        crate::dev::console::serial_write("[welcome] fb() returned None!\n");
    }
    crate::boot::visual::clear();

    if let Some(fb) = fb() {
        crate::dev::console::serial_write("[welcome] starting first render\n");
        render(&fb);
        crate::dev::console::serial_write("[welcome] first render done\n");
        let on = blink_on();
        paint_caret(&fb, on);
        unsafe { LAST_BLINK_ON = on; }
    }
    unsafe { DIRTY = true; }

    crate::dev::console::serial_write("[welcome] playing logon sound\n");
    crate::bmo_core::gustos::tracks::windows::logon();
    crate::dev::console::serial_write("[welcome] logon done\n");

    loop {
        if unsafe { DIRTY } {
            if let Some(fb) = fb() {
                render(&fb);
                let on = blink_on();
                paint_caret(&fb, on);
                unsafe { LAST_BLINK_ON = on; }
            }
            unsafe { DIRTY = false; }
        }

        if crate::cabina::is_overlay_enabled() {
            crate::cabina::paint_overlay();
        }

        let cycles = 16u64 * 3_700_000;
        let start = crate::cpu::rdtsc();
        while (crate::cpu::rdtsc() - start) < cycles {
            let overlay_was_enabled = crate::cabina::is_overlay_enabled();
            let sc = super::input::poll_key();
            if crate::cabina::is_overlay_enabled() != overlay_was_enabled {
                mark_dirty();
            }
            if sc != 0 {
                if sc == 0x1C {
                    process_enter();
                } else if let Some(ch) = process_scancode(sc) {
                    handle_char(ch);
                }
            }

            if let Some(mut ch) = crate::dev::console::serial_read_byte() {
                if ch == b'\r' { ch = b'\n'; }
                handle_char(ch);
            }

            let cur = blink_on();
            if cur != unsafe { LAST_BLINK_ON } {
                if let Some(fb) = fb() {
                    paint_caret(&fb, cur);
                }
                unsafe { LAST_BLINK_ON = cur; }
            }

            core::hint::spin_loop();
        }

        unsafe {
            let prev = HINT_TIMER;
            if HINT_TIMER > 0 { HINT_TIMER -= 1; }
            if (prev > 0 && HINT_TIMER == 0) || (prev != LAST_HINT_TIMER && HINT_TIMER == 0) {
                DIRTY = true;
            }
            LAST_HINT_TIMER = HINT_TIMER;
        }
    }
}

fn blink_on() -> bool {
    (crate::cpu::rdtsc() / 1_250_000_000) & 1 != 0
}

fn handle_char(ch: u8) {
    match ch {
        b'\n' => process_enter(),
        8 => unsafe {
            if INPUT_LEN > 0 {
                if let Some(fb) = fb() { paint_caret(&fb, false); }
                INPUT_LEN -= 1;
                mark_dirty();
            }
        },
        c if (c >= 32 && c <= 126) || c == 164 || c == 165 => unsafe {
            if INPUT_LEN < MAX_INPUT - 1 {
                if let Some(fb) = fb() { paint_caret(&fb, false); }
                INPUT_BUF[INPUT_LEN] = c;
                INPUT_LEN += 1;
                mark_dirty();
            }
        },
        _ => {}
    }
}



