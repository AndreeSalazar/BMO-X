//! Layouts de teclado (US, ES, DVORAK, COLEMAK). Traducción
//! `Key` (USB HID) → carácter Unicode según layout activo.
//!
//! Reemplaza Win32 `HKL` / `LoadKeyboardLayout` / `ToUnicodeEx`.

pub mod layout;
pub mod entry;

