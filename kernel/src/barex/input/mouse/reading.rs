use super::buttons::MouseButtons;

#[repr(C)]
#[derive(Debug, Clone, Copy, Default)]
pub struct MouseReading {
    pub buttons: MouseButtons,
    /// Delta X raw del último poll. **Sin aceleración del SO.**
    pub dx: i16,
    pub dy: i16,
    pub wheel_v: i8,
    pub wheel_h: i8,
}
