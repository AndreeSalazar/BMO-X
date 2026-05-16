use super::buttons::GamepadButtons;
use super::family::GamepadFamily;

#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct GamepadReading {
    pub family: GamepadFamily,
    pub _pad: [u8; 3],
    pub buttons: GamepadButtons,
    /// Sticks en [-1.0, 1.0].
    pub stick_l: [f32; 2],
    pub stick_r: [f32; 2],
    /// Triggers analógicos en [0.0, 1.0].
    pub trigger_l: f32,
    pub trigger_r: f32,
    /// Acelerómetro (PS/Switch). [m/s²]
    pub accel: [f32; 3],
    /// Giroscopio. [rad/s]
    pub gyro: [f32; 3],
}

impl GamepadReading {
    pub const EMPTY: Self = Self {
        family: GamepadFamily::Generic,
        _pad: [0; 3],
        buttons: GamepadButtons::empty(),
        stick_l: [0.0; 2], stick_r: [0.0; 2],
        trigger_l: 0.0, trigger_r: 0.0,
        accel: [0.0; 3], gyro: [0.0; 3],
    };
}
