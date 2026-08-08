//! `bmo_abi::windowing` -- Contrato de ventanas.
//!
//! Define los **datos** que un programa maneja cuando crea, modifica o
//! recibe eventos de una ventana. Los **syscalls** reales (que terminan
//! en `syscall`) estan en `crate::bmo_abi::syscalls`.
//!
//! ## Layout
//!
//! Una `BmoWindowClass` se pasa a `bmo_register_class` por referencia.
//! Una `BmoWindowCreateInfo` se pasa a `bmo_create_window` por referencia.
//! Los eventos llegan via BEFCore (`bmo_recv`).
//!
//! ## Tamanos
//!
//! - `BmoWindowClass`: 64 bytes
//! - `BmoWindowCreateInfo`: 48 bytes
//! - `BmoPaintEvent`: 32 bytes
//! - `BmoKeyEvent`: 32 bytes
//! - `BmoMouseEvent`: 32 bytes

#![allow(dead_code)]

use crate::bmo_abi::fundamentals::handle::BmoHandle;

// --- Window class ----------------------------------------------------

/// Estilo de borde de la ventana.
#[repr(u32)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BmoWindowBorder {
    /// Sin borde (para splash screens).
    None = 0,
    /// Borde fino.
    Thin = 1,
    /// Borde estandar redimensionable.
    Sizable = 2,
    /// Solo titulo, no redimensionable.
    Fixed = 3,
}

/// Comportamiento al cerrarse la ventana (boton X).
#[repr(u32)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BmoCloseAction {
    /// Ocultar la ventana (no destruirla).
    Hide = 0,
    /// Destruir la ventana y liberar recursos.
    Destroy = 1,
    /// Terminar el proceso que la creo.
    Exit = 2,
}

/// Registro de clase de ventana. Pasado a `bmo_register_class`.
///
/// Tamano fijo: 64 bytes. **No agregar campos** sin bump de version ABI.
#[repr(C)]
#[derive(Clone, Copy)]
pub struct BmoWindowClass {
    /// Nombre unico de la clase (ASCII, null-terminated).
    pub name: [u8; 32],
    /// Estilo de borde.
    pub border: BmoWindowBorder,
    /// Accion al cerrar.
    pub close_action: BmoCloseAction,
    /// Color de fondo por defecto (RGBA).
    pub bg_color: u32,
    /// Reservado (padding / futuro flags).
    pub flags: u32,
}
const _: () = assert!(core::mem::size_of::<BmoWindowClass>() == 48);

impl BmoWindowClass {
    pub const SIZE: usize = 48;

    pub const fn new(name: &str, border: BmoWindowBorder, bg: u32) -> Self {
        let mut n = [0u8; 32];
        let bytes = name.as_bytes();
        let mut i = 0;
        while i < bytes.len() && i < 31 {
            n[i] = bytes[i];
            i += 1;
        }
        n[i] = 0;
        Self {
            name: n,
            border,
            close_action: BmoCloseAction::Destroy,
            bg_color: bg,
            flags: 0,
        }
    }
}

// --- Window create info ---------------------------------------------

/// Flags para `bmo_create_window`.
#[repr(u32)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BmoWindowFlag {
    /// La ventana es visible desde el inicio.
    Visible = 1 << 0,
    /// La ventana acepta foco.
    Focusable = 1 << 1,
    /// La ventana siempre encima (topmost).
    Topmost = 1 << 2,
    /// La ventana no aparece en la barra de tareas.
    NoTaskbar = 1 << 3,
    /// La ventana tiene sombra.
    Shadowed = 1 << 4,
}

/// Informacion para crear una ventana.
///
/// Tamano fijo: 48 bytes.
#[repr(C)]
#[derive(Clone, Copy)]
pub struct BmoWindowCreateInfo {
    /// Nombre de la clase registrada (o "" para default).
    pub class_name: [u8; 32],
    /// X (esquina superior izquierda).
    pub x: i32,
    /// Y (esquina superior izquierda).
    pub y: i32,
    /// Ancho.
    pub w: u32,
    /// Alto.
    pub h: u32,
    /// Flags (OR de `BmoWindowFlag`).
    pub flags: u32,
}
const _: () = assert!(core::mem::size_of::<BmoWindowCreateInfo>() == 52);

impl BmoWindowCreateInfo {
    pub const SIZE: usize = 52;
}

// --- Window handle --------------------------------------------------

/// Handle de una ventana. Es un `BmoHandle` con kind forzado a Window.
pub type BmoWindowHandle = BmoHandle;

// --- Eventos de ventana ---------------------------------------------

/// Tipos de eventos que puede recibir una ventana.
#[repr(u32)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BmoWindowEventKind {
    /// La ventana necesita repintarse. Datos: `BmoPaintEvent`.
    Paint = 1,
    /// Tecla presionada. Datos: `BmoKeyEvent`.
    KeyDown = 2,
    /// Tecla liberada. Datos: `BmoKeyEvent`.
    KeyUp = 3,
    /// Boton del raton presionado. Datos: `BmoMouseEvent`.
    MouseDown = 4,
    /// Boton del raton liberado. Datos: `BmoMouseEvent`.
    MouseUp = 5,
    /// Movimiento del raton. Datos: `BmoMouseEvent`.
    MouseMove = 6,
    /// La ventana cambio de tamano. Datos: `BmoResizeEvent`.
    Resize = 7,
    /// La ventana gano foco. Datos: 0 bytes.
    FocusGained = 8,
    /// La ventana perdio foco. Datos: 0 bytes.
    FocusLost = 9,
    /// La ventana se cerro. Datos: 0 bytes.
    Close = 10,
}

/// Evento de repintado.
///
/// Tamano: 32 bytes.
#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct BmoPaintEvent {
    pub x: i32,
    pub y: i32,
    pub w: u32,
    pub h: u32,
    /// Puntero a un buffer RGBA8 del cliente. Puede ser `0` si el sistema
    /// debe asignarlo.
    pub surface_ptr: u64,
    pub surface_len: u64,
}

/// Tecla (virtual key code, layout-independent).
///
/// Usamos el estandar de Windows VK_* (0x01..0xFF). Solo las mas comunes
/// se enumeran; ver `BmoKey::from_vk` para la lista completa.
#[repr(u32)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BmoKey {
    None = 0,
    A = 0x41,
    B,
    C,
    D,
    E,
    F,
    G,
    H,
    I,
    J,
    K,
    L,
    M,
    N,
    O,
    P,
    Q,
    R,
    S,
    T,
    U,
    V,
    W,
    X,
    Y,
    Z,
    D0 = 0x30,
    D1,
    D2,
    D3,
    D4,
    D5,
    D6,
    D7,
    D8,
    D9,
    F1 = 0x70,
    F2,
    F3,
    F4,
    F5,
    F6,
    F7,
    F8,
    F9,
    F10,
    F11,
    F12,
    Esc = 0x1B,
    Tab = 0x09,
    Space = 0x20,
    Enter = 0x0D,
    Back = 0x08,
    Shift = 0x10,
    Ctrl = 0x11,
    Alt = 0x12,
    Left = 0x25,
    Up = 0x26,
    Right = 0x27,
    Down = 0x28,
}

/// Modificadores activos.
#[derive(Clone, Copy, Debug, Default)]
pub struct BmoModifiers(pub u32);

impl BmoModifiers {
    pub const NONE: Self = Self(0);
    pub const SHIFT: Self = Self(1 << 0);
    pub const CTRL: Self = Self(1 << 1);
    pub const ALT: Self = Self(1 << 2);
    pub const SUPER: Self = Self(1 << 3);
    pub const CAPS: Self = Self(1 << 4);
    pub const NUM: Self = Self(1 << 5);

    pub fn contains(&self, other: Self) -> bool {
        (self.0 & other.0) == other.0
    }
}

/// Evento de teclado.
///
/// Tamano: 32 bytes.
#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct BmoKeyEvent {
    pub key: BmoKey,
    pub scancode: u32,
    pub modifiers: BmoModifiers,
    pub repeat: u32,
    pub _pad: u32,
}

/// Boton del raton.
#[repr(u32)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BmoMouseButton {
    None = 0,
    Left = 1,
    Right = 2,
    Middle = 3,
    X1 = 4,
    X2 = 5,
}

/// Evento de raton.
///
/// Tamano: 32 bytes.
#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct BmoMouseEvent {
    pub x: i32,
    pub y: i32,
    pub dx: i32,
    pub dy: i32,
    pub button: BmoMouseButton,
    pub pressed: u32,
    pub modifiers: BmoModifiers,
    pub _pad: u32,
}

/// Evento de resize.
#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct BmoResizeEvent {
    pub new_w: u32,
    pub new_h: u32,
    pub _pad: u64,
}

/// Payload union para los datos de un evento.
#[repr(C)]
pub union BmoWindowEventData {
    pub paint: BmoPaintEvent,
    pub key: BmoKeyEvent,
    pub mouse: BmoMouseEvent,
    pub resize: BmoResizeEvent,
    pub raw: [u8; 32],
}

impl Copy for BmoWindowEventData {}
impl Clone for BmoWindowEventData {
    fn clone(&self) -> Self {
        *self
    }
}

/// Evento completo recibido por `bmo_recv` (via BEFCore).
///
/// Tamano total: 48 bytes.
#[repr(C)]
#[derive(Clone, Copy)]
pub struct BmoWindowEvent {
    pub kind: BmoWindowEventKind,
    pub window: BmoWindowHandle,
    pub timestamp_ns: u64,
    pub data: BmoWindowEventData,
}

impl BmoWindowEvent {
    pub const SIZE: usize = 48;
}
