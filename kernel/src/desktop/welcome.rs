//! Welcome screen — pantalla profesional de bienvenida en framebuffer.
//!
//! Layout:
//!
//! ```text
//!   ╔══════════════════════════════════════════════════════════╗
//!   ║                                                          ║
//!   ║                     FastOS / BMO                         ║
//!   ║                  Bare Metal Orchestrator                 ║
//!   ║                       v0.9.0                             ║
//!   ║                                                          ║
//!   ║   ✓  Ring 0 + Ring 3 listos                              ║
//!   ║   ✓  13 syscalls BMO activos                             ║
//!   ║   ✓  Compositor Ring 0 cargado                           ║
//!   ║   ✓  Mouse PS/2 + Beep PC speaker                        ║
//!   ║   ✓  RAMdisk con file I/O                                ║
//!   ║                                                          ║
//!   ║   Escribe (Run) y pulsa Enter para entrar al escritorio: ║
//!   ║                                                          ║
//!   ║       > _                                                ║
//!   ║                                                          ║
//!   ╚══════════════════════════════════════════════════════════╝
//! ```
//!
//! Comandos aceptados (case-insensitive):
//!   - `Run`   → lanza el escritorio estable: Ring 0 supervisor + Ring 3 preparado
//!   - `Hello` → lanza el payload mínimo (`spawn_hello`)
//!   - `Reboot` → reinicia
//!   - cualquier otra cosa → muestra hint

use crate::boot_info;
use crate::ui::fb::Framebuffer;
use crate::ui::font;
use super::commands::{eq_ci, trim, should_enter_desktop, enter_desktop, nexo_test_compile};
use super::sound;

// ── Paleta del welcome ─────────────────────────────────────────────
mod pal {
    pub const BG_TOP:    u32 = 0xFF0E1729;
    pub const BG_BOT:    u32 = 0xFF1F1145;

    pub const CARD_BG:   u32 = 0xFF1A2238;
    pub const CARD_BD:   u32 = 0xFF3A4878;
    pub const CARD_SHADOW: u32 = 0xFF040611;

    pub const TITLE:     u32 = 0xFFE6EDF3;
    pub const ACCENT:    u32 = 0xFF76B900;     // BMO green
    pub const SUBTITLE:  u32 = 0xFF8B949E;
    pub const VERSION:   u32 = 0xFF56D4DD;

    pub const OK_FG:     u32 = 0xFF27C93F;
    pub const ITEM:      u32 = 0xFFCBD2DB;
    pub const PROMPT_BG: u32 = 0xFF0B1224;
    pub const PROMPT_BD: u32 = 0xFF56D4DD;
    pub const PROMPT_FG: u32 = 0xFFE6EDF3;
    pub const HINT:      u32 = 0xFFFFBD2E;

    pub const RUN_BTN:   u32 = 0xFF0078D4;
    pub const RUN_BTN_HI:u32 = 0xFF1A8BE0;
}

// ── State del welcome ──────────────────────────────────────────────

const MAX_INPUT: usize = 32;
static mut INPUT_BUF: [u8; MAX_INPUT] = [0; MAX_INPUT];
static mut INPUT_LEN: usize = 0;
static mut HINT_TIMER: u32 = 0;        // frames mostrando hint
static mut HINT_MSG: &[u8] = b"";

// ── Modificadores de teclado ───────────────────────────────────────
static mut KBD_LSHIFT: bool = false;
static mut KBD_RSHIFT: bool = false;
static mut KBD_CAPS:   bool = false;

#[inline]
fn shift_held() -> bool { unsafe { KBD_LSHIFT || KBD_RSHIFT } }

#[inline]
fn caps_on() -> bool { unsafe { KBD_CAPS } }

/// Letras: si `shift XOR caps`, mayúscula. Otros símbolos: shift selecciona el
/// glifo superior. Devuelve `None` para teclas sin texto.
fn translate_scancode(sc: u8) -> Option<u8> {
    // Base (sin shift) — PS/2 Set 1 US-ASCII estándar.
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
        0x27 => (164,  165), // ñ y Ñ (distribución española)
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
    let upper = if is_letter {
        // Letras: shift XOR caps → mayúscula.
        shift_held() ^ caps_on()
    } else {
        shift_held()
    };
    Some(if upper { shifted } else { base })
}

/// Procesa un scancode crudo y, si es modifier, actualiza el estado;
/// si es tecla normal pulsada, devuelve el carácter a insertar.
fn process_scancode(raw: u8) -> Option<u8> {
    let released = (raw & 0x80) != 0;
    let sc = raw & 0x7F;

    // Modificadores
    match sc {
        0x2A => { unsafe { KBD_LSHIFT = !released; } return None; }
        0x36 => { unsafe { KBD_RSHIFT = !released; } return None; }
        0x3A => {
            if !released { unsafe { KBD_CAPS = !KBD_CAPS; } }
            return None;
        }
        0x1D | 0x38 => return None, // Ctrl / Alt — ignorados aquí
        _ => {}
    }

    if released { return None; }
    translate_scancode(sc)
}

// ── Drawing helpers ────────────────────────────────────────────────

fn fb() -> Option<Framebuffer> {
    let (addr, w, h, s) = unsafe {
        (boot_info::FB_ADDR, boot_info::FB_WIDTH, boot_info::FB_HEIGHT, boot_info::FB_STRIDE)
    };
    if addr == 0 || w == 0 { return None; }
    // FB_STRIDE llega desde GOP en píxeles por línea. `Framebuffer::new`
    // espera pitch en bytes, igual que el escritorio Ring 0. Si pasamos el
    // stride crudo, el welcome sólo pinta bien el fondo y el texto/tarjetas
    // quedan corruptos o fuera de sitio en hardware real.
    Some(Framebuffer::new(addr, (s as u64) * 4, w, h))
}

/// Dibuja texto. Soporta escala 1×, 2× y 3× (replicando píxeles).
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
}

#[inline]
fn draw_text(fb: &Framebuffer, x: u32, y: u32, text: &[u8], color: u32) {
    draw_text_scaled(fb, x, y, text, color, 1);
}

// ── Render del welcome ─────────────────────────────────────────────

/// Geometría fija del prompt — usada tanto por el render principal como
/// por el repaint local del caret. Devuelve `(prompt_x, prompt_y, w, h)`.
fn prompt_rect(fb: &Framebuffer) -> (usize, usize, usize, usize) {
    let cw = 980usize;
    let ch = 620usize;
    let cx = (fb.width - cw) / 2;
    let cy = (fb.height - ch) / 2;
    let pw = 720usize;
    let ph = 60usize;
    let px = cx + (cw - pw) / 2;
    let py = cy + ch - ph - 110;
    (px, py, pw, ph)
}

/// Posición del caret en píxeles, dado `len` (caracteres ya escritos).
fn caret_pos(fb: &Framebuffer) -> (usize, usize) {
    let (px, py, _, _) = prompt_rect(fb);
    let len = unsafe { INPUT_LEN };
    let cx = px + 16 + 8 * 2 * 2 + len * 16;
    let cy = py + 20;
    (cx, cy)
}

/// Pinta o borra el caret en su posición actual. No toca el resto del
/// frame: usado tanto por el render full como por el blink local.
fn paint_caret(fb: &Framebuffer, on: bool) {
    let (cx, cy) = caret_pos(fb);
    let color = if on { pal::PROMPT_FG } else { pal::PROMPT_BG };
    fb.fill_rect(cx, cy, 12, 24, color);
}

fn render(fb: &Framebuffer) {
    // 1) Wallpaper gradient
    fb.gradient_v(0, 0, fb.width, fb.height, pal::BG_TOP, pal::BG_BOT);

    // 2) Card centrado
    let cw = 980usize;
    let ch = 620usize;
    let cx = (fb.width - cw) / 2;
    let cy = (fb.height - ch) / 2;

    // sombra
    fb.fill_rounded_rect(cx + 10, cy + 14, cw, ch, 24, pal::CARD_SHADOW);
    // cuerpo
    fb.fill_rounded_rect(cx, cy, cw, ch, 24, pal::CARD_BG);
    fb.draw_rect(cx, cy, cw, ch, pal::CARD_BD, 2);

    // 3) Header — "FastOS / BMO" en grande
    let title = b"FastOS / BMO";
    let tw = title.len() * 8 * 3;
    let tx = cx + (cw - tw) / 2;
    draw_text_scaled(fb, tx as u32, (cy + 60) as u32, title, pal::TITLE, 3);

    // 4) Subtítulo
    let sub = b"Bare Metal Orchestrator";
    let sw = sub.len() * 8 * 2;
    let sx = cx + (cw - sw) / 2;
    draw_text_scaled(fb, sx as u32, (cy + 120) as u32, sub, pal::SUBTITLE, 2);

    // 5) Versión
    let ver = b"v0.9.1  ::  Ring 0 + Ring 3  [WD:5s|Heap:16MB]";
    let vw = ver.len() * 8;
    let vx = cx + (cw - vw) / 2;
    draw_text(fb, vx as u32, (cy + 170) as u32, ver, pal::VERSION);

    // 6) Linea divisoria
    fb.fill_rect(cx + 60, cy + 200, cw - 120, 1, pal::CARD_BD);

    // 7) Lista de status
    let items: [&[u8]; 5] = [
        b"  [OK]  Ring 0 + Ring 3 activos",
        b"  [OK]  13 syscalls BMO operativos",
        b"  [OK]  Compositor Ring 0 cargado",
        b"  [OK]  Mouse PS/2 + Beep PC speaker",
        b"  [OK]  RAMdisk + FileOpen/Read/Close",
    ];
    let mut iy = cy + 220;
    for it in items {
        draw_text(fb, (cx + 80) as u32, iy as u32, it, pal::ITEM);
        // marker verde en la palabra [OK]
        draw_text(fb, (cx + 80) as u32, iy as u32, b"  [OK]", pal::OK_FG);
        iy += 24;
    }

    // 8) Prompt box
    let (px, py, pw, ph) = prompt_rect(fb);

    // hint sobre el prompt
    let hint = b"Escribe (Run) y pulsa Enter -> Ring 0 desktop (stub):";
    let hx = px;
    draw_text(fb, hx as u32, (py - 28) as u32, hint, pal::TITLE);

    // caja
    fb.fill_rounded_rect(px, py, pw, ph, 10, pal::PROMPT_BG);
    fb.draw_rect(px, py, pw, ph, pal::PROMPT_BD, 2);

    // prompt "> " + input (sin caret: el caret se pinta aparte)
    draw_text_scaled(fb, (px + 16) as u32, (py + 18) as u32, b"> ", pal::ACCENT, 2);
    let len = unsafe { INPUT_LEN };
    if len > 0 {
        let txt = unsafe { &INPUT_BUF[..len] };
        draw_text_scaled(fb, (px + 16 + 8 * 2 * 2) as u32, (py + 18) as u32, txt, pal::PROMPT_FG, 2);
    }

    // 9) Hint (si hay)
    let (timer, msg) = unsafe { (HINT_TIMER, HINT_MSG) };
    if timer > 0 && !msg.is_empty() {
        draw_text(fb, hx as u32, (py + ph + 16) as u32, msg, pal::HINT);
    }

    // 10) Botón "RUN" visual a la derecha del prompt
    let btn_w = 120usize;
    let btn_h = 60usize;
    let bx = px + pw - btn_w;
    let by = py;
    let btn_color = if unsafe { INPUT_LEN > 0 && {
        let s = &INPUT_BUF[..INPUT_LEN];
        eq_ci(s, b"run")
    } } { pal::RUN_BTN_HI } else { pal::RUN_BTN };
    fb.fill_rounded_rect(bx, by, btn_w, btn_h, 10, btn_color);
    fb.draw_rect(bx, by, btn_w, btn_h, pal::CARD_BD, 2);
    let lbl = b"RUN";
    let lw = lbl.len() * 8 * 3;
    let lx = bx + (btn_w - lw) / 2;
    draw_text_scaled(fb, lx as u32, (by + 12) as u32, lbl, pal::TITLE, 3);

    // 11) Pie
    let foot = b"FastOS / BMO  ::  Ryzen 5 5600X  ::  GOP framebuffer  ::  UEFI";
    let fw = foot.len() * 8;
    let fx = cx + (cw - fw) / 2;
    draw_text(fb, fx as u32, (cy + ch - 36) as u32, foot, pal::SUBTITLE);
}

/// Render mínimo y robusto para hardware real.
///
/// Usa sólo `clear/fill_rect/text` y evita gradientes/rounded-corners en el
/// boot path. Así el prompt y el hotkey diag funcionan aunque el renderer
/// avanzado tenga un problema de GOP/pitch específico de la máquina.
fn render_safe(fb: &Framebuffer) {
    fb.clear(0xFF07111F);
    fb.fill_rect(0, 0, fb.width, 42, 0xFF101820);
    draw_text(fb, 14, 13, b"FastOS / BMO - SAFE WELCOME", 0xFFE6EDF3);
    draw_text(fb, 14, 58, b"GOP framebuffer OK. Storage/NIC deferred for stable boot.", 0xFF76B900);
    draw_text(fb, 14, 82, b"F9: diag HUD   Ctrl+Alt: diag HUD   Run + Enter: desktop Ring 0", 0xFF8B949E);

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

// ── State global del welcome ───────────────────────────────────────
//
// `DIRTY` se pone a true cada vez que algo del frame cambia (input,
// hint, etc). El loop principal sólo re-renderiza el frame entero
// cuando `DIRTY = true`; el blink del caret se gestiona aparte
// repintando sólo la zona del caret. Esto elimina el ghost flicker
// que se veía cuando hacíamos full repaint cada 32 ms.

static mut DIRTY: bool = true;
static mut LAST_BLINK_ON: bool = false;
static mut LAST_HINT_TIMER: u32 = 0;

#[inline]
fn mark_dirty() { unsafe { DIRTY = true; } }

fn show_hint(msg: &'static [u8]) {
    unsafe {
        HINT_MSG = msg;
        HINT_TIMER = 120; // ~2 seg a 60 FPS
    }
    mark_dirty();
}

// ── Self-test commands ─────────────────────────────────────────────

fn run_phase_self_test(n: u8) {
    use crate::boot::phases::report_self_test;
    let report = match n {
        0 => crate::boot::phases::phase0_cpu::self_test(),
        1 => crate::boot::phases::phase1_memory::self_test(),
        2 => crate::boot::phases::phase2_devices::self_test(),
        3 => crate::boot::phases::phase3_display::self_test(),
        4 => crate::boot::phases::phase4_scheduler::self_test(),
        5 => crate::boot::phases::phase5_desktop::self_test(),
        _ => {
            crate::diag::warn("welcome", "Unknown phase index");
            return;
        }
    };
    crate::diag::info("welcome", "Phase self-test");
    report_self_test(&report);
}

fn run_phase_self_test_ring3() {
    use crate::boot::phases::report_self_test;
    let report = crate::boot::phases::ring3_tests::self_test();
    crate::diag::info("welcome", "Ring 3 self-test");
    report_self_test(&report);
}

fn run_test_all_phases() {
    use crate::boot::phases::report_self_test;
    let reports = [
        crate::boot::phases::phase0_cpu::self_test(),
        crate::boot::phases::ring3_tests::self_test(),
        crate::boot::phases::phase1_memory::self_test(),
        crate::boot::phases::phase2_devices::self_test(),
        crate::boot::phases::phase3_display::self_test(),
        crate::boot::phases::phase4_scheduler::self_test(),
        crate::boot::phases::phase5_desktop::self_test(),
    ];
    crate::diag::info("welcome", "All-phase self-test");
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
        show_hint(b"Escribe (Run) y Enter.");
    } else if should_enter_desktop(trimmed_cmd) {
        enter_desktop();
        // If we get here, the desktop returned (shouldn't happen but
        // the watchdog can cause this). Just redraw the welcome.
        crate::diag::info("welcome", "Desktop returned; re-entering welcome");
        show_hint(b"Desktop returned unexpectedly. Type test for diagnostics.");
    } else if eq_ci(trimmed_cmd, b"hello") {
        crate::diag::info("welcome", "Hello command accepted; preparing Ring 3 test");
        sound::beep(440, 80);
        crate::sched::user_init::spawn_hello();
    } else if eq_ci(trimmed_cmd, b"ring3") {
        // Alias for "hello" — explicit Ring 3 transition test.
        crate::diag::info("welcome", "Ring3 command accepted; testing Ring 0 -> Ring 3");
        sound::beep(440, 80);
        crate::sched::user_init::spawn_hello();
    } else if eq_ci(trimmed_cmd, b"reboot") {
        crate::diag::warn("welcome", "Reboot command accepted");
        unsafe { core::arch::asm!("out dx, al", in("dx") 0x64u16, in("al") 0xFEu8); }
    } else if eq_ci(trimmed_cmd, b"nexo") {
        crate::diag::info("welcome", "NEXO compiler test — compiling hello program");
        nexo_test_compile();
    } else if eq_ci(trimmed_cmd, b"test desktop") {
        // Isolated desktop test: render one frame and report.
        crate::diag::info("welcome", "test desktop: rendering single frame");
        crate::drivers::serial::serial_write("[welcome] test desktop: calling render_frame()\n");
        crate::desktop::render::render_frame();
        crate::drivers::serial::serial_write("[welcome] test desktop: render_frame() returned OK\n");
        crate::diag::info("welcome", "test desktop: render_frame OK");
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
        crate::diag::warn("welcome", "Unknown command at welcome prompt");
        show_hint(b"Comandos: Run, Hello, Ring3, Nexo, Test, Reboot.");
    }
    unsafe { INPUT_LEN = 0; }
    mark_dirty();
}

// ── Main loop ──────────────────────────────────────────────────────

pub fn run() -> ! {
    crate::drivers::serial::serial_write("[welcome] Pantalla de bienvenida activa.\n");

    // Beep suave de arranque
    sound::beep(523, 40);
    sound::beep(659, 40);
    sound::beep(784, 80);

    loop {
        // 1) Full repaint solo si algo cambió.
        if unsafe { DIRTY } {
            if let Some(fb) = fb() {
                render(&fb);
                // Pintar el caret en el estado actual del blink para que
                // no aparezca/desaparezca en el siguiente sub-loop.
                let on = blink_on();
                paint_caret(&fb, on);
                unsafe { LAST_BLINK_ON = on; }
            }
            unsafe { DIRTY = false; }
        }

            if crate::diag::is_overlay_enabled() {
                crate::diag::paint_overlay();
            }

        // 3) Sub-loop de ~16 ms: drena input y gestiona blink local.
        let cycles = 16u64 * 3_700_000;
        let start = crate::arch::cpu::rdtsc();
        while (crate::arch::cpu::rdtsc() - start) < cycles {
            let overlay_was_enabled = crate::diag::is_overlay_enabled();
            let sc = super::input::poll_key();
            if crate::diag::is_overlay_enabled() != overlay_was_enabled {
                mark_dirty();
            }
            if sc != 0 {
                if sc == 0x1C {
                    process_enter();
                } else if let Some(ch) = process_scancode(sc) {
                    handle_char(ch);
                }
            }

            if let Some(mut ch) = crate::drivers::serial::serial_read_byte() {
                if ch == b'\r' { ch = b'\n'; }
                handle_char(ch);
            }

            // Blink local: si el estado cambia, repintar SOLO el caret.
            let cur = blink_on();
            if cur != unsafe { LAST_BLINK_ON } {
                if let Some(fb) = fb() {
                    paint_caret(&fb, cur);
                }
                unsafe { LAST_BLINK_ON = cur; }
            }

            core::hint::spin_loop();
        }

        // 3) Decrementar hint timer — si llega a 0, marcar dirty para
        //    borrarlo del frame.
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

/// Estado del caret blink basado en TSC (~1.5 Hz). Centralizado para
/// que el render full y el blink local lean exactamente el mismo bit.
fn blink_on() -> bool {
    (crate::arch::cpu::rdtsc() / 1_250_000_000) & 1 != 0
}

fn handle_char(ch: u8) {
    match ch {
        b'\n' => process_enter(),
        8 => unsafe {
            if INPUT_LEN > 0 {
                // Antes de mover el caret, borrarlo de la posición vieja
                // para evitar ghost de la barra del caret previo.
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
                // Ya NO auto-disparamos al escribir "run": el usuario
                // debe pulsar Enter para evitar congelamientos.
                mark_dirty();
            }
        },
        _ => {}
    }
}
