//! Ratón — alta resolución (16-bit deltas + wheel V/H). Raw deltas always.

pub mod buttons;
pub mod reading;
pub mod cursor;

pub use buttons::MouseButtons;
pub use reading::MouseReading;
pub use cursor::CursorMode;
