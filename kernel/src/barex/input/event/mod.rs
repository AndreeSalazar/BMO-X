//! Eventos discretos y snapshot por frame.

pub mod reading;
pub mod event;

pub use reading::InputReading;
pub use event::{InputEvent, InputEventKind};
