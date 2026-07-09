//! BMO/BMO v1.8.8
//!
//! Desarrolado por Salazar.
//!
//! ui â€” Servicios de interfaz de usuario de BMO.
//!
//! Componentes de presentaciÃ³n visual del kernel:
//!
//! ```text
//!   ui::console  â† Consola de texto sobre framebuffer
//!   ui::fb       â† Framebuffer (abstracciÃ³n de pÃ­xeles)
//!   ui::font     â† Fuente VGA bitmap 8x16
//! ```

#![allow(dead_code)]

pub mod console;
pub mod fb;
pub mod font;
pub mod animation;

