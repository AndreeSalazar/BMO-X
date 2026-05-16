use super::super::keyboard::Key;

#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct KeymapEntry {
    pub key: Key,
    /// Carácter Unicode sin shift.
    pub plain: char,
    /// Carácter con shift.
    pub shift: char,
    /// Carácter con AltGr (layouts europeos).
    pub altgr: char,
}

impl KeymapEntry {
    pub const EMPTY: Self = Self {
        key: Key::None,
        plain: '\0', shift: '\0', altgr: '\0',
    };
}
