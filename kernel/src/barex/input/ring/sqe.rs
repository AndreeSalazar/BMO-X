use crate::bmo_abi::primitives::{bx_u8, bx_u32, bx_u64};

#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InputSqeOp {
    /// Subscribirse a un dispositivo específico (device-filter).
    Subscribe       = 1,
    /// Cancelar suscripción.
    Unsubscribe     = 2,
    /// Inyectar evento sintético (requiere `EVENT_INJECT` cap).
    Inject          = 3,
    /// Activar rumble en gamepad (requiere `RUMBLE` cap).
    Rumble          = 4,
    /// Configurar cursor mode (visible/captured/etc).
    SetCursorMode   = 5,
}

#[repr(C, align(64))]
#[derive(Debug, Clone, Copy)]
pub struct InputSqe {
    pub op: bx_u8,
    pub _pad: [bx_u8; 7],
    /// Device handle target.
    pub device: bx_u64,
    pub user_data: bx_u64,
    pub payload_lo: bx_u64,
    pub payload_hi: bx_u64,
    pub flags: bx_u32,
    pub _reserved: bx_u32,
}

impl InputSqe {
    pub const ZERO: Self = Self {
        op: 0, _pad: [0; 7],
        device: 0, user_data: 0,
        payload_lo: 0, payload_hi: 0,
        flags: 0, _reserved: 0,
    };
}
