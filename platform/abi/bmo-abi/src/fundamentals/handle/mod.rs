//! `handle` -- handles opacos del BMO ABI.
//!
//! Reemplaza `HANDLE` (Win32), `int fd` (POSIX), `IUnknown*` (COM) con un
//! unico tipo `BmoHandle` 64-bit que incluye **generacion**: detecta UAF
//! por construccion.
//!
//! -- EL SEMAFORO (L6g) y las dos preguntas de antes (L6e, L6f) --------
//!
//! Que cuesta que falle, por que falla ESTA pieza, y que arrastro si la
//! toco. La ley esta en `META-KERNEL_HARD.md`.
//!
//! [carril]  ROJO         el reparto, y hereda el color del carril que manda
//! [cuesta]  PUERTA       hereda de `kind.rs` y `opaque.rs`: lo de aqui esta
//!                        en binarios que ya existen
//! [riesgo]  UNICO        hereda: un numero de handle se congela una vez

pub mod kind;
pub mod opaque;
pub mod ops;

pub use opaque::BmoHandle;
