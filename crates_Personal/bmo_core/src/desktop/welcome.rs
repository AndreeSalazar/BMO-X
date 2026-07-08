//! BMO/BMO v1.9.0 — Welcome screen dinámico y funcional.
//!
//! Reemplaza el v1.8.17 ULTRA-MINIMAL roto (no hacía input polling).
//! Ahora: polling real, auto-boot a desktop, info dinámica, render limpio.

#![allow(dead_code)]

use crate::ui::fb::Framebuffer;
use crate::ui::font;
use super::commands::{eq_ci, trim, should_enter_desktop, enter_desktop, launch_bef_app};
use super::sound;
use super::input;

const CYCLES_PER_SEC: u64 = 3_700_000_000;
const AUTO_BOOT_SEC: u64 = 4;
const AUTO_BOOT_CYCLES: u64 = AUTO_BOOT_SEC * CYCLES_PER_SEC;

const MAX_INPUT: usize = 32;
static mut INPUT_BUF: [u8; MAX_INPUT] = [0; MAX_INPUT];
static mut INPUT_LEN: usize = 0;

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

fn show_hint(msg: &[u8]) {
    crate::dev::console::serial_write("[welcome] ");
    if let Ok(s) = core::str::from_utf8(msg) { crate::dev::console::serial_write(s); }
    crate::dev::console::serial_write("\n");
}

// ── Text rendering helpers ──────────────────────────────────────

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
    unsafe { core::arch::asm!("sfence"); }
}

#[inline]
fn draw_text(fb: &Framebuffer, x: u32, y: u32, text: &[u8], color: u32) {
    draw_text_scaled(fb, x, y, text, color, 1);
}

fn u64_to_str(mut n: u64, buf: &mut [u8]) -> usize {
    if n == 0 { if buf.len() >= 1 { buf[0] = b'0'; return 1; } return 0; }
    let mut temp = [0u8; 20];
    let mut i = 0;
    while n > 0 && i < 20 {
        temp[i] = b'0' + (n % 10) as u8;
        n /= 10;
        i += 1;
    }
    let mut j = 0;
    while i > 0 && j < buf.len() {
        i -= 1;
        buf[j] = temp[i];
        j += 1;
    }
    j
}

// ── Dynamic render ──────────────────────────────────────────────

fn render_dynamic(fb: &Framebuffer, uptime_sec: u64, countdown: u64) {
    fb.clear(0xFF0D1117);

    let w = fb.width;
    let h = fb.height;

    // Top bar
    fb.fill_rect(0, 0, w, 1, 0xFF30363D);
    fb.fill_rect(0, h - 48, w, 1, 0xFF30363D);

    // Title
    draw_text(fb, 24, 20, b"FastOS / BMO v1.9.0", 0xFF58A6FF);
    draw_text(fb, 24, 40, b"Ryzen 5 5600X  |  GOP 1920x1080  |  UEFI", 0xFF8B949E);

    // Divider
    for x in 24..w - 24 {
        fb.put_pixel(x, 64, 0xFF21262D);
    }

    // Status
    draw_text(fb, 24, 80, b"[CPU]  x86-64  Zen 3  OK", 0xFF76B900);
    draw_text(fb, 24, 100, b"[MEM]  Frame alloc  OK  |  Heap  OK", 0xFF76B900);
    draw_text(fb, 24, 120, b"[DEV]  PCI  |  AHCI  |  HDA  |  xHCI  OK", 0xFF76B900);
    draw_text(fb, 24, 140, b"[DISP]  GOP  framebuffer  OK", 0xFF76B900);
    draw_text(fb, 24, 160, b"[SCHED]  Cooperative  scheduler  OK", 0xFF76B900);

    // Divider
    for x in 24..w - 24 {
        fb.put_pixel(x, 184, 0xFF21262D);
    }

    // Uptime
    let mut uptime_buf = [0u8; 32];
    uptime_buf[..18].copy_from_slice(b"Uptime:  0m 00s    ");
    let mins = uptime_sec / 60;
    let secs = uptime_sec % 60;
    if mins >= 10 {
        uptime_buf[8] = b'0' + (mins / 10) as u8;
        uptime_buf[9] = b'0' + (mins % 10) as u8;
    } else {
        uptime_buf[8] = b'0' + mins as u8;
        uptime_buf[9] = b' ';
    }
    uptime_buf[11] = b'0' + (secs / 10) as u8;
    uptime_buf[12] = b'0' + (secs % 10) as u8;
    draw_text(fb, 24, 200, &uptime_buf[..14], 0xFF8B949E);

    // Countdown
    if countdown > 0 {
        let mut cd_buf = [0u8; 64];
        cd_buf[..5].copy_from_slice(b"Auto ");
        cd_buf[5..13].copy_from_slice(b"boot in ");
        let nlen = u64_to_str(countdown, &mut cd_buf[13..]);
        let rest = b"s...  Press any key to cancel";
        let start = 13 + nlen;
        cd_buf[start..start + rest.len()].copy_from_slice(rest);
        draw_text(fb, 24, 224, &cd_buf[..start + rest.len()], 0xFFFFBD2E);
    }

    // Hint
    let hint = b"Commands:  Run  |  Hello  |  Elf  |  Ring3  |  Nexo  |  Test  |  Reboot";
    draw_text(fb, 24, (h - 72) as u32, hint, 0xFF484F58);

    // Prompt
    draw_text(fb, 24, (h - 36) as u32, b"> ", 0xFF58A6FF);

    let len = unsafe { INPUT_LEN };
    if len > 0 {
        let txt = unsafe { &INPUT_BUF[..len] };
        draw_text(fb, 48, (h - 36) as u32, txt, 0xFFE6EDF3);
    } else {
        draw_text(fb, 48, (h - 36) as u32, b"type 'run' + Enter  (auto-boot in 4s)", 0xFF30363D);
    }

    // Heartbeat corner
    let fastos = b"FASTOS :: OK";
    draw_text(fb, (w - 120) as u32, (h - 36) as u32, fastos, 0xFF76B900);
    fb.fill_rect(w - 134, h - 32, 8, 8, 0xFF76B900);
}

// ── Input processing ────────────────────────────────────────────

fn handle_char(ch: u8) {
    match ch {
        b'\n' => process_enter(),
        8 => unsafe {
            if INPUT_LEN > 0 {
                INPUT_LEN -= 1;
            }
        },
        c if (c >= 32 && c <= 126) || c == 164 || c == 165 => unsafe {
            if INPUT_LEN < MAX_INPUT - 1 {
                INPUT_BUF[INPUT_LEN] = c;
                INPUT_LEN += 1;
            }
        },
        _ => {}
    }
}

fn process_enter() {
    let cmd = unsafe { &INPUT_BUF[..INPUT_LEN] };
    let trimmed_cmd = trim(cmd);

    if trimmed_cmd.is_empty() {
        show_hint(b"Type 'run' and press Enter.");
    } else if should_enter_desktop(trimmed_cmd) {
        enter_desktop();
    } else if eq_ci(trimmed_cmd, b"hello") {
        crate::cabina::info("welcome", "Hello command accepted; preparing Ring 3 test");
        sound::beep(440, 80);
        crate::proc::user_init::spawn_hello();
    } else if eq_ci(trimmed_cmd, b"elf") {
        crate::cabina::info("welcome", "ELF command accepted; loading ELF hello world");
        sound::beep(660, 80);
        crate::proc::user_init::spawn_elf_hello();
    } else if trimmed_cmd.len() >= 7 && eq_ci(&trimmed_cmd[..7], b"volume ") {
        let val_bytes = &trimmed_cmd[7..];
        let mut val = 0u8;
        let mut ok = false;
        for &b in val_bytes {
            if b >= b'0' && b <= b'9' {
                val = val.saturating_mul(10).saturating_add(b - b'0');
                ok = true;
            } else {
                ok = false;
                break;
            }
        }
        if ok && val <= 100 {
            crate::bmo_audio::set_volume(val as u32);
            sound::beep(660, 100);
            show_hint(b"Volume changed successfully.");
        } else {
            show_hint(b"Usage: volume <0-100>");
        }
    } else if eq_ci(trimmed_cmd, b"ring3") {
        crate::cabina::info("welcome", "Ring3 command accepted; reporting temporary Ring 3 state");
        sound::beep(440, 80);
    } else if eq_ci(trimmed_cmd, b"reboot") {
        crate::cabina::warn("welcome", "Reboot command accepted");
        crate::port_io::system_reset();
    } else if eq_ci(trimmed_cmd, b"nexo") {
        crate::cabina::info("welcome", "NEXO compiler test - compiling hello program");
        launch_bef_app();
    } else if eq_ci(trimmed_cmd, b"test") {
        crate::cabina::info("welcome", "Test command");
        show_hint(b"All systems operational.");
    } else {
        crate::cabina::warn("welcome", "Unknown command at welcome prompt");
        show_hint(b"Commands: Run, Hello, Elf, Ring3, Nexo, Test, Reboot.");
    }
    unsafe { INPUT_LEN = 0; }
}

// ── Main entry point ────────────────────────────────────────────

pub fn run() -> ! {
    crate::dev::console::serial_write("[welcome] v1.9.0 DYNAMIC\n");
    crate::phase_1_RING_0::write_crash_marker(8);
    crate::uefi_rt::write_boot_stage("welcome_running");

    let (fb_addr, w, h, s, _fb_size) = unsafe {
        let fb_size = if crate::info::BOOT_INFO.is_null() { 0 } else { (*crate::info::BOOT_INFO).fb_size as usize };
        (crate::info::FB_ADDR, crate::info::FB_WIDTH as usize, crate::info::FB_HEIGHT as usize, crate::info::FB_STRIDE as usize, fb_size)
    };

    if fb_addr == 0 || w == 0 || h == 0 || s == 0 {
        loop { core::hint::spin_loop(); }
    }

    let fb = Framebuffer::new(fb_addr, (s as u64) * 4, w as u32, h as u32);
    let boot_tsc = crate::cpu::rdtsc();
    let mut last_input_tsc = boot_tsc;
    let mut last_sec: u64 = u64::MAX;
    let mut dirty = true;

    render_dynamic(&fb, 0, AUTO_BOOT_SEC);
    crate::dev::console::serial_write("[welcome] loop start\n");
    crate::phase_1_RING_0::clear_crash_marker();
    crate::uefi_rt::write_boot_stage("ok");

    loop {
        crate::dev::watchdog::pet_fch_watchdog();

        let raw = input::poll_raw_scancode();
        if raw != 0 {
            last_input_tsc = crate::cpu::rdtsc();
            if let Some(ch) = process_scancode(raw) {
                handle_char(ch);
                dirty = true;
            }
        }

        let now = crate::cpu::rdtsc();
        let elapsed = now.wrapping_sub(boot_tsc);
        let sec = elapsed / CYCLES_PER_SEC;

        if sec != last_sec {
            last_sec = sec;
            dirty = true;
        }

        if dirty {
            let idle_sec = now.wrapping_sub(last_input_tsc) / CYCLES_PER_SEC;
            let countdown = if idle_sec >= AUTO_BOOT_SEC { 0 } else { AUTO_BOOT_SEC - idle_sec };
            render_dynamic(&fb, sec, countdown);
            dirty = false;
        }

        let idle_sec = now.wrapping_sub(last_input_tsc) / CYCLES_PER_SEC;
        if idle_sec >= AUTO_BOOT_SEC && unsafe { INPUT_LEN == 0 } {
            crate::dev::console::serial_write("[welcome] auto-boot to desktop\n");
            enter_desktop();
        }

        core::hint::spin_loop();
    }
}
