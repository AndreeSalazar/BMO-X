//! (1) `BxDevice` — singleton. Sustituye `ID3D12Device14` + DXGI.

use crate::barex::{BxError, BxResult};
use super::queue::{BxQueue, QueueKind};

pub struct BxDevice {
    _private: (),
}

impl BxDevice {
    /// Único punto de entrada — equivalente a `D3D12CreateDevice` sin
    /// dependencia de una GPU concreta. El backend inicial es GOP/software.
    pub fn primary() -> BxResult<Self> {
        Err(BxError::NotImplemented)
    }

    pub fn create_queue(&self, _kind: QueueKind) -> BxResult<BxQueue> {
        Err(BxError::NotImplemented)
    }
}
