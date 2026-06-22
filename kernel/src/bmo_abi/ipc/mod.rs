//! `bmo_abi::ipc` — Inter-Process Communication (ports y mensajes).
//!
//! Modelo: **ports con derechos de capacidad**. Cada port es una cola
//! de mensajes unidireccional. Para hacer bidirectional, se crean dos
//! ports.
//!
//! ## Syscalls (ver `syscalls/mod.rs`)
//!
//! - `NR_IPC_PORT_CREATE` (0x1A0) → `bmo_ipc_port_create() -> BmoPort`
//! - `NR_IPC_PORT_SEND`   (0x1A1) → `bmo_ipc_port_send(p, msg)`
//! - `NR_IPC_PORT_RECV`   (0x1A2) → `bmo_ipc_port_recv(p, buf) -> usize`
//! - `NR_IPC_PORT_CLOSE`  (0x1A3) → `bmo_ipc_port_close(p)`
//!
//! ## Zero-copy
//!
//! `BmoMessage` se compone de un header fijo (32 bytes) + un payload
//! variable que se pasa como `(ptr, len)`. El kernel copia el payload
//! al port (no comparte memoria entre procesos por seguridad).

#![allow(dead_code)]

use crate::bmo_abi::fundamentals::handle::BmoHandle;

// ─── Handle ─────────────────────────────────────────────────────────

/// Handle a un port IPC. Proceso-local.
pub type BmoPort = BmoHandle;

// ─── Rights ─────────────────────────────────────────────────────────

/// Derechos de un port. Se piden al crear el port y se transfieren al
/// peer cuando se le pasa el handle.
#[repr(u32)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BmoPortRight {
    /// Puede leer del port (recv).
    Recv   = 1 << 0,
    /// Puede escribir al port (send).
    Send   = 1 << 1,
    /// Puede cerrar el port.
    Close  = 1 << 2,
    /// Puede transferir el handle a otro proceso.
    Grant  = 1 << 3,
}

impl BmoPortRight {
    pub const NONE: u32 = 0;
    pub const RDWR: u32 = 0b1111;
    pub const RD:   u32 = 1 << 0;
    pub const WR:   u32 = (1 << 0) | (1 << 1);
}

// ─── Message ────────────────────────────────────────────────────────

/// Header de mensaje IPC. Tamaño fijo: 32 bytes.
///
/// El payload **no** está incluido en el header: se pasa por separado
/// como `(ptr, len)`.
#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct BmoMessage {
    /// Tag arbitrario (definido por el programa, no por el kernel).
    pub tag: u32,
    /// Flags (prioridad, etc.).
    pub flags: u32,
    /// ID del proceso que envía.
    pub sender: u32,
    /// ID del proceso destino (0 = cualquiera).
    pub target: u32,
    /// Tamaño del payload en bytes.
    pub payload_len: u32,
    /// Tipo de mensaje (definido por el programa).
    pub msg_type: u32,
    /// Reservado (alineación a 8 bytes).
    pub _pad: u32,
    /// Handle transferido (0 si ninguno).
    pub transferred: BmoHandle,
}

impl BmoMessage {
    pub const HEADER_SIZE: usize = 32;

    /// Tamaño total en bytes (header + payload).
    #[inline]
    pub fn total_size(&self) -> usize { Self::HEADER_SIZE + self.payload_len as usize }
}

/// Flags para mensajes.
#[repr(transparent)]
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct BmoMsgFlags(pub u32);

impl BmoMsgFlags {
    pub const NONE:     Self = Self(0);
    /// Mensaje prioritario (se entrega antes que los normales).
    pub const PRIORITY: Self = Self(1 << 0);
    /// Mensaje de control (no payload).
    pub const CONTROL:  Self = Self(1 << 1);
    /// Solicita respuesta.
    pub const REPLY:    Self = Self(1 << 2);
    /// Es la respuesta a un `REPLY`.
    pub const RESPONSE: Self = Self(1 << 3);
}
