//! BMO Input Subsystem -- HAL-based keyboard/mouse driver.
//!
//! ## Architecture
//!
//! ```text
//! bmo_input::hal          <- InputHal trait (contract)
//! bmo_input::hal_ps2      <- PS/2 backend (ports 0x60/0x64)
//! bmo_input::event        <- InputEvent + InputEventQueue (lock-free ring)
//! bmo_input::keyboard     <- Scancode + HID Usage -> VK translation
//! bmo_input::mouse        <- Pointer state tracking
//! ```
//!
//! ## Usage from kernel
//!
//! ```ignore
//! use bmo_input::hal_ps2::Ps2Hal;
//! use bmo_input::hal::InputHal;
//!
//! let mut ps2 = Ps2Hal::new();
//! ps2.init();
//! let mut buf = [bmo_input::event::InputEvent::empty(); 32];
//! let n = ps2.poll(&mut buf);
//! ```

#![no_std]

pub mod hal;
#[cfg(feature = "ps2")]
pub mod hal_ps2;
pub mod event;
/// El FOCO: quien recibe las teclas cuando hay mas de una ventana. Vive aqui
/// porque enrutar entrada es el oficio de este crate -- y porque aqui se puede
/// probar: el compositor es `no_main` para un target sin sistema operativo y no
/// corre un test.
pub mod foco;
pub mod keyboard;
pub mod mouse;

pub use hal::InputHal;
pub use event::{InputEvent, InputEventKind, InputEventQueue};
pub use foco::{Foco, Modo};
