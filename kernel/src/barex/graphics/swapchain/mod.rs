//! (8) `BxSwapchain` — conectado al compositor FastOS (sin DXGI).

use super::types::Format;

pub struct BxSwapchain {
    pub width: u32,
    pub height: u32,
    pub format: Format,
}
