//! FastOS/BMO v1.8.8
//!
//! Desarrolado por Salazar.
//!
//! ui — Servicios de interfaz de usuario de FastOS.
//!
//! Componentes de presentación visual del kernel:
//!
//! ```text
//!   ui::console  ← Consola de texto sobre framebuffer
//!   ui::fb       ← Framebuffer (abstracción de píxeles)
//!   ui::font     ← Fuente VGA bitmap 8x16
//! ```

#![allow(dead_code)]

pub mod console;
pub mod fb;
pub mod font;

