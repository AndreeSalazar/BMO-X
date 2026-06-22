//! `bmo_abi::befcore` — Protocolo BEFCore messages.
//!
//! Define cómo las **apps C-compiladas** (que corren en Ring 3) se
//! comunican con **BMO CORE** (que corre en Ring 0/1). Es la "puerta"
//! entre C y FastOS.
//!
//! ## Modelo
//!
//! Cada mensaje es un `BefcoreMessage` de tamaño fijo. La app escribe
//! el mensaje en un **mailbox** (dirección compartida user↔kernel) y
//! luego hace `syscall(BEFCORE_SEND, &msg, 0, 0)`. BMO CORE recibe
//! la syscall, lee el mensaje, y procesa la acción.
//!
//! ## Tipos de mensajes
//!
//! | MsgKind | Payload | Acción |
//! |---------|---------|--------|
//! | `CreateWindow` | title, x, y, w, h | BMO CORE crea ventana y devuelve handle |
//! | `DestroyWindow` | hwnd | Destruye ventana |
//! | `DrawText` | hwnd, x, y, text | Dibuja texto en DC de la ventana |
//! | `FillRect` | hwnd, x, y, w, h, color | Rellena rectángulo |
//! | `ShowWindow` | hwnd, cmd | show/hide/min/max |
//! | `PumpEvents` | timeout_ms | Procesa eventos pendientes |
//! | `Exit` | code | Sale de la app con código |
//!
//! ## Diferencia con syscalls
//!
//! - **Syscalls (0x100..0x1FF):** llamadas bloqueantes o rápidas.
//!   BMO CORE responde inmediatamente.
//! - **BEFCore messages:** tareas asíncronas (dibujar, eventos, etc.)
//!   BMO CORE los procesa cuando el scheduler decida.
//!
//! Ver `Rutas.md` §3 (BMO CORE) y `bmo_core/bef/` (BEF loader).
//!
//! Status: ✅ COMPLETO — módulo nuevo en v1.8.8

#![allow(dead_code)]

use core::mem;
use super::fundamentals::handle::BmoHandle;
use super::fundamentals::status::BmoStatus;

// ── Constantes del protocolo ─────────────────────────────────────────

/// Syscall number: enviar un mensaje a BMO CORE.
/// Definido en BMO ABI syscalls rango 0x190..0x19F (BMO Core mailbox).
pub const NR_BEFCORE_SEND: u32 = 0x190;

/// Syscall number: recibir un mensaje de BMO CORE (evento, response).
pub const NR_BEFCORE_RECV: u32 = 0x191;

/// Syscall number: bloquear hasta que llegue un mensaje de un tipo dado.
pub const NR_BEFCORE_POLL: u32 = 0x192;

/// Versión del protocolo BEFCore.
pub const BEFCORE_VERSION: (u8, u8) = (1, 0);

/// Tamaño máximo del payload de un mensaje (bytes).
pub const BEFCORE_MAX_PAYLOAD: usize = 64;

/// Mailbox por defecto: dirección user donde la app escribe el mensaje.
/// (Las apps pueden crear su propio mailbox con su allocator.)
/// Por ahora, BMO CORE pone el mailbox al final del stack del proceso.
pub const DEFAULT_MAILBOX_OFFSET_FROM_STACK_TOP: usize = 4096;

// ── Tipos de mensajes ───────────────────────────────────────────────

/// Identifica el tipo de mensaje BEFCore.
#[repr(u32)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BefcoreMsgKind {
    /// Crea una ventana. Payload: title, x, y, w, h, style.
    CreateWindow = 0x01,
    /// Destruye una ventana.
    DestroyWindow = 0x02,
    /// Muestra u oculta una ventana.
    ShowWindow = 0x03,
    /// Dibuja texto en una ventana.
    DrawText = 0x04,
    /// Rellena un rectángulo.
    FillRect = 0x05,
    /// Dibuja una línea.
    DrawLine = 0x06,
    /// Procesa eventos pendientes (no-bloqueante).
    PumpEvents = 0x07,
    /// Espera un evento (bloqueante con timeout).
    WaitEvent = 0x08,
    /// Sale de la app.
    Exit = 0x09,
    /// Yield: cede el CPU a otros procesos.
    Yield = 0x0A,
    /// Log: escribe a serial/log BMO CORE.
    Log = 0x0B,
}

/// Comandos para `ShowWindow`.
#[repr(u32)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WindowShowCmd {
    Hide = 0,
    Show = 1,
    Minimize = 2,
    Maximize = 3,
    Restore = 4,
}

/// Estilos de ventana.
pub mod window_style {
    pub const TITLE_BAR: u32 = 1 << 0;
    pub const RESIZABLE: u32 = 1 << 1;
    pub const CLOSE_BOX: u32 = 1 << 2;
    pub const MINIMIZE_BOX: u32 = 1 << 3;
    pub const MAXIMIZE_BOX: u32 = 1 << 4;
    pub const VISIBLE: u32 = 1 << 5;
    pub const ON_TOP: u32 = 1 << 6;
}

/// Colores predefinidos (X11/RGB-style, XRGB32 en memoria).
pub mod color {
    pub const BLACK:        u32 = 0x000000;
    pub const WHITE:        u32 = 0xFFFFFF;
    pub const RED:          u32 = 0xFF0000;
    pub const GREEN:        u32 = 0x00FF00;
    pub const BLUE:         u32 = 0x0000FF;
    pub const CYAN:         u32 = 0x00FFFF;
    pub const MAGENTA:      u32 = 0xFF00FF;
    pub const YELLOW:       u32 = 0xFFFF00;
    pub const GRAY:         u32 = 0x808080;
    pub const DARK_GRAY:    u32 = 0x404040;
    pub const LIGHT_GRAY:   u32 = 0xC0C0C0;
}

// ── Mensaje ────────────────────────────────────────────────────────────

/// Mensaje BEFCore de tamaño fijo (64 bytes payload + 32 bytes header).
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct BefcoreMessage {
    /// Tipo de mensaje.
    pub kind: u32,
    /// Identificador de la ventana (donde aplique) o 0.
    pub hwnd: u32,
    /// Argumento 1 (significado depende de `kind`).
    pub arg1: u32,
    /// Argumento 2.
    pub arg2: u32,
    /// Argumento 3.
    pub arg3: u32,
    /// Argumento 4.
    pub arg4: u32,
    /// Argumento 5.
    pub arg5: u32,
    /// Bytes del payload (texto, datos raw).
    pub payload: [u8; 64],
    /// Tamaño real del payload.
    pub payload_len: u32,
    /// Código de respuesta (rellenado por BMO CORE).
    pub response: BmoStatus,
    /// Handle de respuesta (cuando aplique).
    pub response_handle: BmoHandle,
    /// Padding/reservado para alineación.
    pub _pad: [u8; 8],
}

impl BefcoreMessage {
    /// Tamaño total del mensaje en bytes.
    pub const SIZE: usize = mem::size_of::<Self>();
}

// ── Constructores de conveniencia ─────────────────────────────────────

impl BefcoreMessage {
    /// Crea un mensaje `CreateWindow` con título + geometría + estilo.
    pub fn create_window(
        title: &str,
        x: i32, y: i32, w: u32, h: u32,
        style: u32,
    ) -> Self {
        let mut m = Self::empty();
        m.kind = BefcoreMsgKind::CreateWindow as u32;
        m.arg1 = x as u32;
        m.arg2 = y as u32;
        m.arg3 = w;
        m.arg4 = h;
        m.arg5 = style;
        let bytes = title.as_bytes();
        let len = bytes.len().min(BEFCORE_MAX_PAYLOAD);
        m.payload[..len].copy_from_slice(&bytes[..len]);
        m.payload_len = len as u32;
        m
    }

    /// Crea un mensaje `DestroyWindow`.
    pub fn destroy_window(hwnd: u32) -> Self {
        let mut m = Self::empty();
        m.kind = BefcoreMsgKind::DestroyWindow as u32;
        m.hwnd = hwnd;
        m
    }

    /// Crea un mensaje `ShowWindow`.
    pub fn show_window(hwnd: u32, cmd: WindowShowCmd) -> Self {
        let mut m = Self::empty();
        m.kind = BefcoreMsgKind::ShowWindow as u32;
        m.hwnd = hwnd;
        m.arg1 = cmd as u32;
        m
    }

    /// Crea un mensaje `DrawText`.
    pub fn draw_text(hwnd: u32, x: i32, y: i32, text: &str) -> Self {
        let mut m = Self::empty();
        m.kind = BefcoreMsgKind::DrawText as u32;
        m.hwnd = hwnd;
        m.arg1 = x as u32;
        m.arg2 = y as u32;
        let bytes = text.as_bytes();
        let len = bytes.len().min(BEFCORE_MAX_PAYLOAD);
        m.payload[..len].copy_from_slice(&bytes[..len]);
        m.payload_len = len as u32;
        m
    }

    /// Crea un mensaje `FillRect`.
    pub fn fill_rect(hwnd: u32, x: i32, y: i32, w: u32, h: u32, color: u32) -> Self {
        let mut m = Self::empty();
        m.kind = BefcoreMsgKind::FillRect as u32;
        m.hwnd = hwnd;
        m.arg1 = x as u32;
        m.arg2 = y as u32;
        m.arg3 = w;
        m.arg4 = h;
        m.arg5 = color;
        m
    }

    /// Crea un mensaje `PumpEvents`.
    pub fn pump_events() -> Self {
        let mut m = Self::empty();
        m.kind = BefcoreMsgKind::PumpEvents as u32;
        m
    }

    /// Crea un mensaje `WaitEvent` con timeout en ms.
    pub fn wait_event(timeout_ms: u32) -> Self {
        let mut m = Self::empty();
        m.kind = BefcoreMsgKind::WaitEvent as u32;
        m.arg1 = timeout_ms;
        m
    }

    /// Crea un mensaje `Exit`.
    pub fn exit(code: i32) -> Self {
        let mut m = Self::empty();
        m.kind = BefcoreMsgKind::Exit as u32;
        m.arg1 = code as u32;
        m
    }

    /// Crea un mensaje `Yield`.
    pub fn yield_now() -> Self {
        let mut m = Self::empty();
        m.kind = BefcoreMsgKind::Yield as u32;
        m
    }

    /// Crea un mensaje `Log`.
    pub fn log(text: &str) -> Self {
        let mut m = Self::empty();
        m.kind = BefcoreMsgKind::Log as u32;
        let bytes = text.as_bytes();
        let len = bytes.len().min(BEFCORE_MAX_PAYLOAD);
        m.payload[..len].copy_from_slice(&bytes[..len]);
        m.payload_len = len as u32;
        m
    }

    /// Crea un mensaje vacío (todos los campos a cero).
    pub const fn empty() -> Self {
        Self {
            kind: 0,
            hwnd: 0,
            arg1: 0,
            arg2: 0,
            arg3: 0,
            arg4: 0,
            arg5: 0,
            payload: [0u8; BEFCORE_MAX_PAYLOAD],
            payload_len: 0,
            response: super::fundamentals::status::code::BmoStatus::OK,
            response_handle: BmoHandle::NULL,
            _pad: [0u8; 8],
        }
    }
}

// ── Eventos (BMO CORE → app) ──────────────────────────────────────────

/// Eventos que BMO CORE envía a la app.
#[repr(u32)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BefcoreEventKind {
    /// Ventana fue creada.
    WindowCreated = 0x100,
    /// Ventana fue destruida.
    WindowDestroyed = 0x101,
    /// Ventana necesita repintarse.
    Paint = 0x102,
    /// Tecla presionada.
    KeyDown = 0x110,
    /// Tecla soltada.
    KeyUp = 0x111,
    /// Carácter de texto.
    Char = 0x112,
    /// Mouse movido.
    MouseMove = 0x120,
    /// Botón del mouse presionado.
    MouseDown = 0x121,
    /// Botón del mouse soltado.
    MouseUp = 0x122,
    /// Timer expiró.
    Timer = 0x130,
    /// La app debe cerrarse.
    Quit = 0x1FF,
}

/// Evento de BMO CORE → app.
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct BefcoreEvent {
    /// Tipo de evento.
    pub kind: u32,
    /// Handle de la ventana relacionada.
    pub hwnd: u32,
    /// Timestamp TSC.
    pub timestamp: u64,
    /// Datos del evento (significado depende de `kind`).
    pub data: [u64; 4],
}

impl BefcoreEvent {
    /// Tamaño en bytes.
    pub const SIZE: usize = mem::size_of::<Self>();
}

// ── Re-exports ────────────────────────────────────────────────────────

pub use BefcoreMsgKind as MsgKind;
pub use BefcoreEventKind as EventKind;
