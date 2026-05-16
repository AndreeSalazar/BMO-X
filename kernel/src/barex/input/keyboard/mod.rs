//! Teclado — keycodes universales (USB HID Usage Page 0x07).

pub mod key;
pub mod modifiers;
pub mod reading;

pub use key::Key;
pub use modifiers::Modifiers;
pub use reading::KeyboardReading;
