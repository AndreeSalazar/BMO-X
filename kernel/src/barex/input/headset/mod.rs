//! Headset Redragon — botones de volumen/media como HID Consumer Page.

pub mod button;
pub mod event;

pub use button::HeadsetButton;
pub use event::HeadsetButtonEvent;
