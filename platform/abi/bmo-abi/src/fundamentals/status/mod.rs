//! `status` -- codigos de retorno BMO ABI.
//!
//! `BmoStatus` reemplaza:
//!   - Win32 `HRESULT` (32-bit) + `GetLastError()` (TLS global)
//!   - POSIX `errno` (TLS global)
//!   - COM `IUnknown` reference counts mezclados con error codes
//!
//! Todo viaja **por valor** en `RAX:RDX` (16 bytes). Cero acceso a memoria.
//!
//! -- EL SEMAFORO (L6g) y las dos preguntas de antes (L6e, L6f) --------
//!
//! Que cuesta que falle, por que falla ESTA pieza, y que arrastro si la
//! toco. La ley esta en `META-KERNEL_HARD.md`.
//!
//! [carril]  ROJO         el reparto, y hereda el color del carril que manda
//! [cuesta]  PUERTA       hereda de `code.rs`: es lo que viaja en rax/rdx
//! [riesgo]  SILENCIO     hereda: un campo confundido da un `ok` que no lo
//!                        era

pub mod code;
pub mod error;

pub use code::BmoStatus;
pub use error::message as error_message;
