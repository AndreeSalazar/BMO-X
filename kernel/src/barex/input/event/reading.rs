use super::super::keyboard::KeyboardReading;
use super::super::mouse::MouseReading;

#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct InputReading {
    pub frame_index: u64,
    pub timestamp_ns: u64,
    pub keyboard: KeyboardReading,
    pub mouse: MouseReading,
}

impl InputReading {
    pub const EMPTY: Self = Self {
        frame_index: 0,
        timestamp_ns: 0,
        keyboard: KeyboardReading::EMPTY,
        mouse: MouseReading {
            buttons: super::super::mouse::MouseButtons::empty(),
            dx: 0, dy: 0, wheel_v: 0, wheel_h: 0,
        },
    };

    #[inline(always)]
    pub fn key_down(&self, k: super::super::keyboard::Key) -> bool {
        self.keyboard.is_down(k)
    }
}
