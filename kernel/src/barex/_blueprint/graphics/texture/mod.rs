//! (10) `BxTexture` — 1D / 2D / 3D / Cube / Array.

use super::types::Format;

pub struct BxTexture {
    pub width: u32,
    pub height: u32,
    pub depth_or_array: u32,
    pub format: Format,
}
