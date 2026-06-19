#[repr(C)]
#[derive(Debug, Clone, Copy, Default)]
pub struct WheelReading {
    /// Ángulo del volante en grados (-900..900 típico).
    pub steer_deg: f32,
    /// Pedales en [0.0, 1.0].
    pub throttle: f32,
    pub brake: f32,
    pub clutch: f32,
    /// Handbrake (drift). [0.0, 1.0].
    pub handbrake: f32,
    /// Bitmask de botones del volante (Logitech G29 / Thrustmaster T300).
    pub buttons: u32,
    /// HOTAS: throttle del joystick.
    pub throttle_lever: f32,
    /// Rudder pedals.
    pub rudder: f32,
}
