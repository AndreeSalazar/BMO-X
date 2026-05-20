//! Estado del escritorio — mantenido en Ring 0 y avanzado en cada
//! `syscall DesktopFrame (0x65)`.
//!
//! Single-instance estático: hoy hay un único compositor.
//! Incluye estado de ventanas dinámicas, drag, focus y edge detection
//! del botón del ratón (para click-vs-hold).

#![allow(dead_code)]

use crate::arch::cpu;
use crate::boot_info;

/// Cycles per second en Ryzen 5 5600X (3.7 GHz boost).
pub const CYCLES_PER_SEC: u64 = 3_700_000_000;

/// Cantidad de iconos del dock.
pub const DOCK_SLOTS: usize = 7;

/// Capacidad de la tabla de ventanas.
pub const MAX_WIN: usize = 8;

/// Cada cuántos frames refrescamos el FPS mostrado. A ~60 fps son ~0.5 s.
/// Evita el parpadeo del número en la status bar.
pub const FPS_DISPLAY_PERIOD: u64 = 30;

/// Una ventana dinámica.
#[derive(Clone, Copy)]
pub struct WinInfo {
    pub open: bool,
    pub x: i32,
    pub y: i32,
    pub w: i32,
    pub h: i32,
    pub title_id: u8, // 0..7 — índice en TITLES de render.rs
}

impl WinInfo {
    pub const fn empty() -> Self {
        Self { open: false, x: 0, y: 0, w: 0, h: 0, title_id: 0 }
    }
}

#[derive(Clone, Copy)]
pub struct DesktopState {
    /// Frame counter.
    pub frame: u64,
    pub clock_start_tsc: u64,
    pub last_tsc: u64,
    /// EMA interno (alto detalle) — se actualiza cada frame.
    pub fps_avg: u32,
    /// Valor estable que muestra la status bar — se "engancha" cada
    /// `FPS_DISPLAY_PERIOD` frames a `fps_avg` para evitar parpadeo.
    pub fps_display: u32,

    pub mouse_x: i32,
    pub mouse_y: i32,
    pub mouse_buttons: u8,
    /// Botones del frame anterior (para edge detection click).
    pub prev_buttons: u8,

    pub dock_hover: i32,
    pub dock_active: i32,

    pub windows: [WinInfo; MAX_WIN],
    /// Índice de la ventana con foco (top), -1 si ninguna.
    pub focus: i32,

    /// Índice de la ventana en arrastre, -1 si ninguno.
    pub drag_idx: i32,
    /// Offset cursor→origen-ventana al iniciar el drag.
    pub drag_dx: i32,
    pub drag_dy: i32,

    /// `true` tras la primera `init()`.
    pub windows_init_done: bool,
}

impl DesktopState {
    pub const fn new() -> Self {
        Self {
            frame: 0,
            clock_start_tsc: 0,
            last_tsc: 0,
            fps_avg: 60,
            fps_display: 60,
            mouse_x: 0,
            mouse_y: 0,
            mouse_buttons: 0,
            prev_buttons: 0,
            dock_hover: -1,
            dock_active: 0,
            windows: [WinInfo::empty(); MAX_WIN],
            focus: -1,
            drag_idx: -1,
            drag_dx: 0,
            drag_dy: 0,
            windows_init_done: false,
        }
    }
}

pub static mut STATE: DesktopState = DesktopState::new();

/// Inicializa el reloj de referencia + ventanas por defecto.
pub fn init() {
    unsafe {
        if STATE.clock_start_tsc == 0 {
            let t = cpu::rdtsc();
            STATE.clock_start_tsc = t;
            STATE.last_tsc = t;
            STATE.mouse_x = (boot_info::FB_WIDTH / 2) as i32;
            STATE.mouse_y = (boot_info::FB_HEIGHT / 2) as i32;
        }
        if !STATE.windows_init_done {
            init_default_windows();
            STATE.windows_init_done = true;
        }
    }
}

/// Tres ventanas iniciales abiertas en cascada.
unsafe fn init_default_windows() {
    STATE.windows[0] = WinInfo { open: true, x: 80,  y: 80,  w: 760, h: 460, title_id: 0 };
    STATE.windows[1] = WinInfo { open: true, x: 920, y: 80,  w: 760, h: 460, title_id: 1 };
    STATE.windows[2] = WinInfo { open: true, x: 500, y: 580, w: 760, h: 380, title_id: 5 };
    STATE.focus = 0;
}

/// Abre la ventana definida por `title_id`. Si ya existe abierta, sólo le da
/// foco y la mueve al frente. Si no, busca un slot libre y la crea
/// cascade-posicionada.
pub fn open_window(title_id: u8) {
    unsafe {
        // ¿existe ya abierta?
        for i in 0..MAX_WIN {
            if STATE.windows[i].open && STATE.windows[i].title_id == title_id {
                STATE.focus = i as i32;
                return;
            }
        }
        // buscar slot libre
        for i in 0..MAX_WIN {
            if !STATE.windows[i].open {
                let cascade = (i as i32) * 40;
                STATE.windows[i] = WinInfo {
                    open: true,
                    x: 200 + cascade,
                    y: 120 + cascade,
                    w: 700,
                    h: 420,
                    title_id,
                };
                STATE.focus = i as i32;
                return;
            }
        }
    }
}

/// Cierra la ventana de índice `idx`.
pub fn close_window(idx: usize) {
    unsafe {
        if idx < MAX_WIN {
            STATE.windows[idx].open = false;
            if STATE.focus == idx as i32 {
                // Buscar nuevo focus
                STATE.focus = -1;
                for j in 0..MAX_WIN {
                    if STATE.windows[j].open { STATE.focus = j as i32; break; }
                }
            }
        }
    }
}

/// Avanza el estado un frame: ratón, FPS, frame counter.
pub fn tick() {
    init();
    let packed = super::poll_mouse();
    unsafe {
        STATE.frame = STATE.frame.wrapping_add(1);

        STATE.prev_buttons = STATE.mouse_buttons;
        STATE.mouse_x = ((packed & 0xFFFF) as i16) as i32;
        STATE.mouse_y = (((packed >> 16) & 0xFFFF) as i16) as i32;
        STATE.mouse_buttons = ((packed >> 32) & 0xFF) as u8;

        let now = cpu::rdtsc();
        let dt = now.saturating_sub(STATE.last_tsc).max(1);
        STATE.last_tsc = now;
        // Clamp del FPS instantáneo a [1, 240] para que un dt anómalo (p.e.
        // primer frame, o jitter del scheduler) no envenene el EMA.
        let instant_fps = ((CYCLES_PER_SEC / dt) as u32).clamp(1, 240);
        // EMA muy suave (32 frames de inercia): nuevo = old*31/32 + inst*1/32.
        STATE.fps_avg =
            ((STATE.fps_avg as u64 * 31 + instant_fps as u64) / 32) as u32;

        // El número mostrado sólo se "engancha" cada FPS_DISPLAY_PERIOD frames,
        // así no parpadea en pantalla.
        if STATE.frame % FPS_DISPLAY_PERIOD == 0 {
            STATE.fps_display = STATE.fps_avg;
        }
    }
}

pub fn uptime_sec() -> u64 {
    unsafe {
        let now = cpu::rdtsc();
        let dt = now.saturating_sub(STATE.clock_start_tsc);
        dt / CYCLES_PER_SEC
    }
}

pub fn clock_hms() -> (u8, u8, u8) {
    let t = uptime_sec() + 9 * 3600;
    let h = ((t / 3600) % 24) as u8;
    let m = ((t / 60) % 60) as u8;
    let s = (t % 60) as u8;
    (h, m, s)
}

// ── Edge detection helpers ─────────────────────────────────────────

pub fn mouse_left_pressed() -> bool {
    unsafe { (STATE.mouse_buttons & 1) != 0 && (STATE.prev_buttons & 1) == 0 }
}

pub fn mouse_left_released() -> bool {
    unsafe { (STATE.mouse_buttons & 1) == 0 && (STATE.prev_buttons & 1) != 0 }
}

pub fn mouse_left_held() -> bool {
    unsafe { (STATE.mouse_buttons & 1) != 0 }
}
