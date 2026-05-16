use super::key::Key;
use super::modifiers::Modifiers;

#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct KeyboardReading {
    pub modifiers: Modifiers,
    /// 6-key rollover (USB Boot Protocol). N-key rollover usa report custom.
    pub keys_down: [Key; 6],
}

impl KeyboardReading {
    pub const EMPTY: Self = Self {
        modifiers: Modifiers::empty(),
        keys_down: [Key::None; 6],
    };

    #[inline(always)]
    pub fn is_down(&self, k: Key) -> bool {
        self.keys_down.iter().any(|&x| x as u8 == k as u8)
    }
}
