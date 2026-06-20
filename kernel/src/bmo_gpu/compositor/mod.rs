//! BMO GPU Compositor — Ring 0 ↔ Ring 3 GPU composition.
//!
//! v1.7.9: stub. The compositor will be implemented in v1.8 alongside
//! the AMDGPU driver.

#![allow(dead_code)]

/// Drawing surface (placeholder).
pub struct Surface {
    pub width: u32,
    pub height: u32,
    pub format: SurfaceFormat,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SurfaceFormat {
    Xrgb8888,
    Argb8888,
    Rgba8888,
}

impl Surface {
    pub const fn new(width: u32, height: u32, format: SurfaceFormat) -> Self {
        Self { width, height, format }
    }
}

/// Initialize the compositor (v1.7.9: no-op).
pub fn init() {
    // v1.8: connect to AMDGPU, init ring buffer.
}
