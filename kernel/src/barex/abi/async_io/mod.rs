//! `async_io` — Submission Queue / Completion Queue para I/O async BMO ABI.
//!
//! Reemplaza:
//!   - Win32 `OVERLAPPED` + `IoCompletionPort` + `APC`
//!   - POSIX `aio_*` + signal callbacks
//!   - Callbacks de stack que sobreviven a la función
//!
//! Inspirado en Linux io_uring, pero más simple porque FastOS solo tiene
//! un kernel, una arquitectura, y SQE/CQE de tamaño fijo.

pub mod ring;

pub use ring::{Sqe, Cqe, SqRing, CqRing, OpCode};
