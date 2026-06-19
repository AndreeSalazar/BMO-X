//! BareX Blueprint — diseño de la API completa de FastOS.
//!
//! v1.2.0: Todo el código que describe la **API futura** de BareX
//! (audio, graphics, input, net, shader backends) se consolidó aquí.
//!
//! ## ¿Qué es esto?
//!
//! Son esqueletos de la API que existirá cuando FastOS tenga:
//! - Ring 3 (procesos de usuario)
//! - GPU acceleration (actualmente solo GOP/software)
//! - Audio engine (actualmente solo beep PC speaker)
//! - Network stack (existe en el kernel pero el wrapper BareX no)
//!
//! Cada `pub fn` retorna `BxError::NotImplemented` — **compilan pero
//! no hacen nada útil en runtime**. Son **documentación ejecutable**:
//! el código es la spec, y la spec no se desactualiza.
//!
//! ## Estructura
//!
//! ```text
//!   _blueprint/
//!   ├── mod.rs            ← este archivo
//!   ├── audio/            ← bx_audio API (40 archivos, ~30K líneas)
//!   ├── graphics/         ← BxDevice, BxSwapchain, etc. (17 archivos)
//!   ├── input/            ← HID, gamepad, keyboard (39 archivos)
//!   ├── net/              ← TCP/UDP/QUIC/TLS (33 archivos)
//!   └── shader/           ← spirv/dxil/dxbc/ir/native/cache/stage
//! ```
//!
//! ## Roadmap de implementación
//!
//! | Subsistema  | Trigger                                    | Esfuerzo |
//! |-------------|--------------------------------------------|----------|
//! | `shader::*` | Cuando `nexo-sh` (Ring 3) esté integrado   | Bajo     |
//! | `net::*`    | Cuando exista un usuario Ring 3 que lo use | Medio    |
//! | `input::*`  | Cuando se quieran APIs BareX (no Win32)    | Bajo     |
//! | `audio::*`  | Cuando se quiera audio nativo (no USB)     | Alto     |
//! | `graphics::*` | Cuando haya GPU acceleration            | Alto     |
//!
//! ## Cómo usar esto desde código de producción
//!
//! **No lo hagas.** Si necesitas funcionalidad real, usa:
//! - `kernel::bmo_abi::*` para el ABI nativo
//! - `kernel::drivers::*` para drivers reales
//! - `kernel::bef::loader::pe` para cargar PE con thunks
//!
//! Los blueprints existen para que cuando llegue el momento, el código
//! de la API ya esté escrito y revisado.

#![allow(dead_code)]

pub mod audio;
pub mod graphics;
pub mod input;
pub mod net;
pub mod shader;
