//! Kernel bypass — la app habla **directo** a la NIC sin pasar por
//! socket layer, sin checksum offload del kernel, sin context switches.
//!
//! Solo apps con `NetCapabilities::RAW_KERNEL_BYPASS` lo pueden usar.
//! Casos de uso: HFT, trading, gaming low-latency, captura de paquetes.
//!
//! Estilo DPDK / AF_XDP / netmap. La app mapea las RX/TX queues del NIC
//! directamente en su address space.

#![allow(dead_code)]

use crate::barex::{BxError, BxResult};
use crate::barex::abi::primitives::{bx_u32, bx_u64};

/// Anillo zero-copy mapeado al address space del usuario.
#[repr(C, align(64))]
pub struct BypassRing {
    /// Buffer base (mapeado de la NIC).
    pub buf_ptr: bx_u64,
    pub buf_len: bx_u64,
    pub head: bx_u32,
    pub tail: bx_u32,
    /// Cola RX o TX.
    pub kind: bx_u32,
    pub _pad: bx_u32,
}

pub const BYPASS_KIND_RX: bx_u32 = 0;
pub const BYPASS_KIND_TX: bx_u32 = 1;

impl BypassRing {
    pub const ZERO: Self = Self {
        buf_ptr: 0, buf_len: 0,
        head: 0, tail: 0,
        kind: BYPASS_KIND_RX, _pad: 0,
    };

    /// Mapea un anillo nuevo. Requiere capability + driver compatible.
    pub fn map(_queue_idx: u32, _kind: u32) -> BxResult<Self> {
        Err(BxError::NotImplemented)
    }
}
