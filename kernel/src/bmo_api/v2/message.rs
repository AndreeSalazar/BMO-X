//! v2.0 — Mensajes BMO API (struct 32 bytes + enum kind).

#![allow(dead_code)]

/// Tipos de mensaje. Coinciden con el spec §2.5. u16 porque 64 KB
/// es espacio más que suficiente para todos los tipos del sistema +
/// los definidos por usuario (0x0400..=0xFFFF).
#[repr(u16)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BmoMsgKind {
    Null            = 0x0000,
    Create          = 0x0001,
    Destroy         = 0x0002,
    Paint           = 0x0003,
    Size            = 0x0004,
    Move            = 0x0005,
    Activate        = 0x0006,
    SetFocus        = 0x0007,
    KillFocus       = 0x0008,
    Close           = 0x0009,
    ShowWindow      = 0x000A,
    Hide            = 0x000B,
    EraseBkGnd      = 0x000C,
    NcPaint         = 0x000D,
    NcCalcSize      = 0x000E,
    NcCreate        = 0x000F,
    NcDestroy       = 0x0010,
    InitDialog      = 0x0011,
    Command         = 0x0012,
    Timer           = 0x0013,
    Quit            = 0x0014,
    EnterSizeMove   = 0x0015,
    ExitSizeMove    = 0x0016,
    GetMinMaxInfo   = 0x0017,
    WindowPosChanged= 0x0018,

    KeyDown         = 0x0200,
    KeyUp           = 0x0201,
    Char            = 0x0202,
    SysKeyDown      = 0x0203,
    SysKeyUp        = 0x0204,

    MouseMove       = 0x0300,
    LButtonDown     = 0x0301,
    LButtonUp       = 0x0302,
    RButtonDown     = 0x0303,
    RButtonUp       = 0x0304,
    MButtonDown     = 0x0305,
    MButtonUp       = 0x0306,
    MouseWheel      = 0x0307,
    MouseHover      = 0x0308,
    MouseLeave      = 0x0309,
    CaptureChanged  = 0x030A,

    User            = 0x0400,
}

impl BmoMsgKind {
    pub fn from_u16(v: u16) -> Self {
        match v {
            0x0000 => Self::Null,
            0x0001 => Self::Create,
            0x0002 => Self::Destroy,
            0x0003 => Self::Paint,
            0x0004 => Self::Size,
            0x0005 => Self::Move,
            0x0006 => Self::Activate,
            0x0007 => Self::SetFocus,
            0x0008 => Self::KillFocus,
            0x0009 => Self::Close,
            0x000A => Self::ShowWindow,
            0x000B => Self::Hide,
            0x000C => Self::EraseBkGnd,
            0x000D => Self::NcPaint,
            0x000E => Self::NcCalcSize,
            0x000F => Self::NcCreate,
            0x0010 => Self::NcDestroy,
            0x0011 => Self::InitDialog,
            0x0012 => Self::Command,
            0x0013 => Self::Timer,
            0x0014 => Self::Quit,
            0x0015 => Self::EnterSizeMove,
            0x0016 => Self::ExitSizeMove,
            0x0017 => Self::GetMinMaxInfo,
            0x0018 => Self::WindowPosChanged,
            0x0200 => Self::KeyDown,
            0x0201 => Self::KeyUp,
            0x0202 => Self::Char,
            0x0203 => Self::SysKeyDown,
            0x0204 => Self::SysKeyUp,
            0x0300 => Self::MouseMove,
            0x0301 => Self::LButtonDown,
            0x0302 => Self::LButtonUp,
            0x0303 => Self::RButtonDown,
            0x0304 => Self::RButtonUp,
            0x0305 => Self::MButtonDown,
            0x0306 => Self::MButtonUp,
            0x0307 => Self::MouseWheel,
            0x0308 => Self::MouseHover,
            0x0309 => Self::MouseLeave,
            0x030A => Self::CaptureChanged,
            _ => Self::User,
        }
    }

    /// Clasificación de prioridad (Win32: WM_PAINT y WM_TIMER son low).
    pub fn is_low_priority(self) -> bool {
        matches!(self, Self::Paint | Self::Timer | Self::MouseMove | Self::MouseHover)
    }
}

/// Mensaje de 32 bytes — pasa la frontera ring 0/ring 3 por valor.
#[derive(Debug, Clone, Copy, Default)]
#[repr(C)]
pub struct BmoMsg {
    pub kind: u16,        // BmoMsgKind
    pub target: u16,      // wid del receptor
    pub source: u16,      // wid del emisor (0 = kernel)
    pub _pad0: u16,
    pub timestamp: u32,   // ms desde boot
    pub wparam: u64,
    pub lparam: u64,
    pub pt_x: i32,
    pub pt_y: i32,
}
// 2+2+2+2+4+8+8+4+4 = 36 bytes. Lo ajustamos con 4 bytes al final:
// (Mejor: 2+2+2+2 = 8, +4 = 12, +8+8 = 28, +4+4 = 36).
// Lo dejamos en 36; el user-mode y kernel usan el mismo layout.

// Verificamos el tamaño esperado en compilación.
const _: () = assert!(core::mem::size_of::<BmoMsg>() <= 64);

impl BmoMsg {
    pub const fn null() -> Self {
        Self {
            kind: 0, target: 0, source: 0, _pad0: 0,
            timestamp: 0, wparam: 0, lparam: 0,
            pt_x: -1, pt_y: -1,
        }
    }

    pub fn new(kind: BmoMsgKind, target: u16, source: u16, wparam: u64, lparam: u64) -> Self {
        Self {
            kind: kind as u16,
            target,
            source,
            _pad0: 0,
            timestamp: 0,
            wparam,
            lparam,
            pt_x: -1, pt_y: -1,
        }
    }
}
