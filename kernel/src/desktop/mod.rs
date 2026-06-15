//! Desktop — kernel-side helpers que sirven a los syscalls del compositor
//! Ring 3:
//!   0x60 FbInfo      0x61 FbFill   0x62 FbText   0x63 FbPresent
//!   0x64 FbBlit      0x70 KeyPoll  0x71 MousePoll  0x80 Beep
//!
//! Mantiene cero estado dinámico (lee FB desde `boot_info::FB_*`).
//! Para sonido usa el PC speaker via PIT canal 2 + puerto 0x61.
//! Para ratón hace polling del controlador PS/2.

#![allow(dead_code)]

use crate::boot_info;
use crate::font;

pub mod compositor;
pub mod state;
pub mod render;
pub mod welcome;

// ────────────────────────────────────────────────────────────────────
// Ring 0 desktop loop — supervisor funcional para el compositor Ring 3.
//
// Ring 0 conserva hardware, GOP/framebuffer, input y syscalls. El compositor
// Ring 3 se prepara desde `sched::user_init::spawn_desktop()`, pero todavía no
// toma el control hasta que el scheduler/context switch de user mode pueda
// volver con seguridad. Esto mantiene `Run` como camino estable.
// ────────────────────────────────────────────────────────────────────

const SC_ESC: u8 = 0x01;
const CYCLES_PER_MS: u64 = 3_700_000;

// ── Modifier key tracking for HUD toggle (Alt + Control) ──────────
static mut CTRL_HELD: bool = false;
static mut ALT_HELD: bool = false;
static mut HOTKEY_TOGGLED: bool = false;

/// Loop principal del escritorio en Ring 0. No retorna — termina con
/// `hlt` infinito (idéntico a `welcome::run()`).
pub fn run_ring0() -> ! {
    crate::diag::info("desktop", "entering Ring 0 GOP desktop supervisor");
    crate::drivers::serial::serial_write("[desktop] Entrando en escritorio Ring 0 supervisor.\n");

    // Beep "entré al escritorio".
    beep(880, 60);
    beep(1320, 80);

    loop {
        // 1) Pintar un frame completo.
        render::render_frame();
        crate::diag::paint_overlay();

        // 2) Dormir ~16 ms drenando teclado para no perder ESC.
        let target = (crate::arch::cpu::rdtsc()).wrapping_add(16 * CYCLES_PER_MS);
        loop {
            let sc = poll_key();
            if sc == SC_ESC { return_to_halt(); }
            if crate::arch::cpu::rdtsc() >= target { break; }
            core::hint::spin_loop();
        }
    }
}

/// ESC presionado: detener el escritorio. No queremos volver al welcome
/// (sería confuso), así que apagamos el speaker y hacemos halt.
fn return_to_halt() -> ! {
    beep(0, 0);
    crate::drivers::serial::serial_write("[desktop] ESC — halt.\n");
    loop { unsafe { core::arch::asm!("hlt"); } }
}

// ────────────────────────────────────────────────────────────────────
// Framebuffer primitives
// ────────────────────────────────────────────────────────────────────

#[inline(always)]
fn fb_base() -> Option<(*mut u32, usize, usize, usize)> {
    let (addr, w, h, s) = unsafe {
        (boot_info::FB_ADDR, boot_info::FB_WIDTH as usize,
         boot_info::FB_HEIGHT as usize, boot_info::FB_STRIDE as usize)
    };
    if addr == 0 || w == 0 || h == 0 { return None; }
    Some((addr as *mut u32, s, w, h))
}

pub fn fb_fill(x: u32, y: u32, w: u32, h: u32, color: u32) {
    let Some((buf, stride, fbw, fbh)) = fb_base() else { return; };
    let x0 = (x as usize).min(fbw);
    let y0 = (y as usize).min(fbh);
    let x1 = ((x as usize) + (w as usize)).min(fbw);
    let y1 = ((y as usize) + (h as usize)).min(fbh);
    for row in y0..y1 {
        let line = unsafe { buf.add(row * stride) };
        for col in x0..x1 {
            unsafe { line.add(col).write_volatile(color); }
        }
    }
}

/// Blit XRGB-8888 raster (`w*h*4` bytes a partir de `src_ptr`) en la
/// posición (x,y). Sin escalado, sin alpha. Útil para "rendering bulk"
/// estilo Doom (320×200 → screen).
pub fn fb_blit(x: u32, y: u32, w: u32, h: u32, src_ptr: u64) {
    let Some((buf, stride, fbw, fbh)) = fb_base() else { return; };
    if src_ptr == 0 || w == 0 || h == 0 { return; }
    let x0 = (x as usize).min(fbw);
    let y0 = (y as usize).min(fbh);
    let w = (w as usize).min(fbw.saturating_sub(x0));
    let h = (h as usize).min(fbh.saturating_sub(y0));
    let src = src_ptr as *const u32;
    for row in 0..h {
        let dst_line = unsafe { buf.add((y0 + row) * stride + x0) };
        let src_line = unsafe { src.add(row * w) };
        for col in 0..w {
            unsafe { dst_line.add(col).write_volatile(src_line.add(col).read()); }
        }
    }
}

pub fn fb_text(x: u32, y: u32, text: &[u8], fg: u32) {
    let Some((buf, stride, fbw, fbh)) = fb_base() else { return; };
    let mut cx = x as usize;
    let cy = y as usize;
    for &ch in text {
        if cx + 8 > fbw { break; }
        if cy + 16 > fbh { break; }
        let glyph = font::get_glyph(ch);
        for py in 0..16 {
            let row = glyph[py];
            let line = unsafe { buf.add((cy + py) * stride) };
            for px in 0..8 {
                if (row & (0x80 >> px)) != 0 {
                    unsafe { line.add(cx + px).write_volatile(fg); }
                }
            }
        }
        cx += 8;
    }
}

// ────────────────────────────────────────────────────────────────────
// Input — teclado PS/2 (poll no-bloqueante)
// ────────────────────────────────────────────────────────────────────

pub fn poll_key() -> u8 {
    let status: u8;
    unsafe { core::arch::asm!("in al, dx", out("al") status, in("dx") 0x64u16); }
    if status == 0xFF { return 0; } // Puerto flotante / no hay controlador
    // bit 0 = output buffer full, bit 5 = mouse data
    if (status & 0x01) == 0 { return 0; }
    if (status & 0x20) != 0 {
        let b: u8;
        unsafe {
            core::arch::asm!("in al, dx", out("al") b, in("dx") 0x60u16);
            process_mouse_byte(b);
        }
        return 0;
    }
    let sc: u8;
    unsafe { core::arch::asm!("in al, dx", out("al") sc, in("dx") 0x60u16); }

    // ── Track modifier keys for Alt+Ctrl HUD toggle ──────────────
    unsafe {
        match sc {
            0x1D => { CTRL_HELD = true; }   // Left Ctrl press
            0x9D => { CTRL_HELD = false; HOTKEY_TOGGLED = false; }  // Left Ctrl release
            0x38 => { ALT_HELD = true; }    // Left Alt press
            0xB8 => { ALT_HELD = false; HOTKEY_TOGGLED = false; }   // Left Alt release
            _ => {}
        }

        // Toggle HUD when both Ctrl+Alt are held (once per press combo)
        if CTRL_HELD && ALT_HELD && !HOTKEY_TOGGLED {
            HOTKEY_TOGGLED = true;
            let currently_on = crate::diag::is_overlay_enabled();
            crate::diag::set_overlay_enabled(!currently_on);
            // Beep de confirmación sutil
            beep(660, 30);
            crate::desktop::state::mark_dirty();
        }
    }

    sc
}

// ────────────────────────────────────────────────────────────────────
// Input — ratón PS/2 (poll no-bloqueante y acumulador)
// ────────────────────────────────────────────────────────────────────

static mut MOUSE_X: i32 = 960;     // centro de 1920×1080
static mut MOUSE_Y: i32 = 540;
static mut MOUSE_BUTTONS: u8 = 0;
static mut MOUSE_PKT: [u8; 3] = [0; 3];
static mut MOUSE_PKT_IDX: usize = 0;
static mut MOUSE_INIT_DONE: bool = false;

#[inline(always)]
unsafe fn ps2_wait_input() {
    for _ in 0..1_000 {
        let s: u8;
        core::arch::asm!("in al, dx", out("al") s, in("dx") 0x64u16);
        if s == 0xFF { return; } // Puerto flotante / no hay controlador
        if (s & 0x02) == 0 { return; }
    }
}

#[inline(always)]
unsafe fn ps2_wait_output() {
    for _ in 0..1_000 {
        let s: u8;
        core::arch::asm!("in al, dx", out("al") s, in("dx") 0x64u16);
        if s == 0xFF { return; } // Puerto flotante / no hay controlador
        if (s & 0x01) != 0 { return; }
    }
}

unsafe fn ps2_write_cmd(cmd: u8) {
    ps2_wait_input();
    core::arch::asm!("out dx, al", in("dx") 0x64u16, in("al") cmd);
}

unsafe fn ps2_write_data(data: u8) {
    ps2_wait_input();
    core::arch::asm!("out dx, al", in("dx") 0x60u16, in("al") data);
}

unsafe fn ps2_write_mouse(data: u8) {
    ps2_write_cmd(0xD4);             // próximo byte va al ratón
    ps2_write_data(data);
}

unsafe fn ps2_read_data() -> u8 {
    ps2_wait_output();
    let v: u8;
    core::arch::asm!("in al, dx", out("al") v, in("dx") 0x60u16);
    v
}

fn mouse_init() {
    unsafe {
        if MOUSE_INIT_DONE { return; }
        MOUSE_INIT_DONE = true;
        crate::drivers::serial::serial_write("[desktop] Bypassing legacy PS/2 mouse setup for pure UEFI.\n");
    }
}

unsafe fn process_mouse_byte(b: u8) {
    MOUSE_PKT[MOUSE_PKT_IDX] = b;
    MOUSE_PKT_IDX += 1;
    if MOUSE_PKT_IDX < 3 { return; }
    MOUSE_PKT_IDX = 0;

    let b0 = MOUSE_PKT[0];
    // bit 3 del primer byte debe ser 1 (sync). Si no, descartar.
    if (b0 & 0x08) == 0 { return; }
    // descartar overflow
    if (b0 & 0xC0) != 0 { return; }

    let dx_raw = MOUSE_PKT[1] as i32;
    let dy_raw = MOUSE_PKT[2] as i32;
    let dx = if (b0 & 0x10) != 0 { dx_raw - 0x100 } else { dx_raw };
    let dy = if (b0 & 0x20) != 0 { dy_raw - 0x100 } else { dy_raw };

    MOUSE_X = (MOUSE_X + dx).clamp(0, boot_info::FB_WIDTH as i32 - 1);
    MOUSE_Y = (MOUSE_Y - dy).clamp(0, boot_info::FB_HEIGHT as i32 - 1);
    MOUSE_BUTTONS = b0 & 0x07;
}

/// Devuelve `(x:i16) | (y:i16 << 16) | (buttons:u8 << 32)`.
pub fn poll_mouse() -> u64 {
    mouse_init();

    unsafe {
        // Drenar todos los paquetes disponibles en una sola llamada.
        let mut limit = 0;
        loop {
            let status: u8;
            core::arch::asm!("in al, dx", out("al") status, in("dx") 0x64u16);
            if status == 0xFF { break; } // Puerto flotante / no hay controlador
            if (status & 0x21) != 0x21 { break; }  // necesita bit 5 (mouse data) Y bit 0 (output full)
            let b: u8;
            core::arch::asm!("in al, dx", out("al") b, in("dx") 0x60u16);
            process_mouse_byte(b);
            limit += 1;
            if limit > 64 { break; } // Evitar bucles infinitos
        }

        let x = (MOUSE_X as i16) as u16 as u64;
        let y = (MOUSE_Y as i16) as u16 as u64;
        let bt = MOUSE_BUTTONS as u64;
        x | (y << 16) | (bt << 32)
    }
}

// ────────────────────────────────────────────────────────────────────
// Sonido — PC speaker via PIT canal 2 + puerto 0x61
// ────────────────────────────────────────────────────────────────────

#[inline]
unsafe fn outb(port: u16, val: u8) {
    core::arch::asm!("out dx, al", in("dx") port, in("al") val);
}

#[inline]
unsafe fn inb(port: u16) -> u8 {
    let v: u8;
    core::arch::asm!("in al, dx", out("al") v, in("dx") port);
    v
}

/// Suena el PC speaker a `freq_hz` Hz durante `duration_ms` ms.
/// Si `freq_hz == 0`, simplemente silencia.
pub fn beep(freq_hz: u32, duration_ms: u32) {
    unsafe {
        if freq_hz == 0 {
            // silenciar
            let p = inb(0x61);
            outb(0x61, p & 0xFC);
            return;
        }
        let div = (1_193_180u32 / freq_hz) as u16;
        outb(0x43, 0xB6); // PIT cmd: ch2, lobyte/hibyte, mode 3 (square wave)
        outb(0x42, (div & 0xFF) as u8);
        outb(0x42, ((div >> 8) & 0xFF) as u8);
        let p = inb(0x61);
        outb(0x61, p | 0x03);  // habilitar speaker + gate

        // espera busy ~ duration_ms (3.7 GHz Ryzen)
        let cycles = (duration_ms as u64) * 3_700_000;
        let start = crate::arch::cpu::rdtsc();
        while (crate::arch::cpu::rdtsc() - start) < cycles {
            core::hint::spin_loop();
        }

        let p2 = inb(0x61);
        outb(0x61, p2 & 0xFC); // apagar
    }
}
