#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct ListenerPose {
    /// Posición en metros (mundo).
    pub pos: [f32; 3],
    /// Vector forward unitario.
    pub forward: [f32; 3],
    /// Vector up unitario.
    pub up: [f32; 3],
}

impl ListenerPose {
    pub const ORIGIN: Self = Self {
        pos: [0.0; 3],
        forward: [0.0, 0.0, -1.0],
        up: [0.0, 1.0, 0.0],
    };
}
