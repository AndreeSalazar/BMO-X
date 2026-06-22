//! `bmo_core::bef::compat` — Universal App Intake Layer.
//!
//! v1.8.8: **STUB**. Cero código de compat real.
//!
//! Esta capa traduce imports de PE (Windows) y ELF (Linux) a llamadas
//! BMO API. Es lo que permite que un `notepad.exe` o un `hello.elf`
//! corran en BMO Core sin reescribirse.
//!
//! ## Estado (v1.8.8)
//!
//! - Todos los archivos son stubs con `// TODO v1.9: ...`.
//! - Ninguna función thunked está implementada.
//! - Solo se documenta la intención.
//!
//! ## Roadmap
//!
//! Ver `SPEC.md` en esta carpeta.
//!
//! v1.9: minimum viable (hello world PE/ELF — 6 funciones).
//! v2.0: windowed app PE (notepad — 14 funciones).
//! v3.0: games (D3D/XInput/XAudio — 100+ funciones).

#![allow(dead_code)]

// Submódulos planeados. Todos son stubs.
pub mod win32;
pub mod linux;
pub mod common;
