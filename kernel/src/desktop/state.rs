//! Estado del escritorio — mantenido en Ring 0 y avanzado en cada
//! `syscall DesktopFrame (0x65)`.
//!
//! Single-instance estático: hoy hay un único compositor.

#![allow(dead_code)]

use crate::arch::cpu;
use crate::boot_info;

/// Cycles per second en Ryzen 5 5600X (3.7 GHz boost). Una calibración
/// real iría vía PIT en el boot; para escritorio basta esta constante.
pub const CYCLES_PER_SEC: u64 = 3_700_000_000;

/// Cantidad de iconos del dock.
pub const DOCK_SLOTS: usize = 7;

#[derive(Clone, Copy)]
pub struct DesktopState {
    /// Frame counter (avanza +1 por `DesktopFrame`).
    pub frame: u64,
    /// TSC al construir el estado (para reloj real).
    pub clock_start_tsc: u64,
    /// TSC del frame anterior (para FPS instantáneo).
    pub last_tsc: u64,
    /// FPS promedio (suavizado).
    pub fps_avg: u32,
    /// Mouse cacheado (sin tener que repoll).
    pub mouse_x: i32,
    pub mouse_y: i32,
    pub mouse_buttons: u8,
    /// Índice del icono del dock bajo el cursor, -1 si ninguno.
    pub dock_hover: i32,
    /// Índice del icono "activo" (último clickeado).
    pub dock_active: i32,
}

impl DesktopState {
    pub const fn new() -> Self {
        Self {
            frame: 0,
            clock_start_tsc: 0,
            last_tsc: 0,
            fps_avg: 60,
            mouse_x: 0,
            mouse_y: 0,
            mouse_buttons: 0,
            dock_hover: -1,
            dock_active: 0,
        }
    }
}

pub static mut STATE: DesktopState = DesktopState::new();

/// Inicializa el reloj de referencia. Idempotente.
pub fn init() {
    unsafe {
        if STATE.clock_start_tsc == 0 {
            let t = cpu::rdtsc();
            STATE.clock_start_tsc = t;
            STATE.last_tsc = t;
            STATE.mouse_x = (boot_info::FB_WIDTH / 2) as i32;
            STATE.mouse_y = (boot_info::FB_HEIGHT / 2) as i32;
        }
    }
}

/// Avanza el estado un frame: ratón, FPS, frame counter.
pub fn tick() {
    init();
    let packed = super::poll_mouse();
    unsafe {
        STATE.frame = STATE.frame.wrapping_add(1);

        // Desempaquetar mouse: x[15:0] | y[31:16] | buttons[39:32]
        STATE.mouse_x = ((packed & 0xFFFF) as i16) as i32;
        STATE.mouse_y = (((packed >> 16) & 0xFFFF) as i16) as i32;
        STATE.mouse_buttons = ((packed >> 32) & 0xFF) as u8;

        // FPS: 1 / dt
        let now = cpu::rdtsc();
        let dt = now.saturating_sub(STATE.last_tsc).max(1);
        STATE.last_tsc = now;
        let instant_fps = (CYCLES_PER_SEC / dt) as u32;
        // EMA suave: nuevo = 0.9 viejo + 0.1 instante
        STATE.fps_avg = (STATE.fps_avg as u64 * 9 / 10 + instant_fps as u64 / 10) as u32;
    }
}

/// Segundos transcurridos desde init (uptime).
pub fn uptime_sec() -> u64 {
    unsafe {
        let now = cpu::rdtsc();
        let dt = now.saturating_sub(STATE.clock_start_tsc);
        dt / CYCLES_PER_SEC
    }
}

/// (HH, MM, SS) computado desde uptime (sin RTC real todavía).
/// Arranca a `09:00:00` (look-and-feel de "ya hay actividad").
pub fn clock_hms() -> (u8, u8, u8) {
    let t = uptime_sec() + 9 * 3600;
    let h = ((t / 3600) % 24) as u8;
    let m = ((t / 60) % 60) as u8;
    let s = (t % 60) as u8;
    (h, m, s)
}
