//! `machinery` — cómo se COMPONE el código que usa el BMO ABI.
//!
//! Son los mecanismos que permiten construir programas complejos:
//! despacho dinámico, type system, exceptions, async I/O, sync.
//!
//! - [`calling`]    — convención de llamada (registros, stack, red zone).
//! - [`sync`]       — `BmoMutex`, `BmoAtomic*`, `BmoFutex`.
//! - [`type_system`]— descriptores universales (sustituye RTTI / `Type`).
//! - [`vtable`]     — despacho dinámico (sustituye vtables C++ / COM).
//! - [`closure`]    — closures de primera clase.
//! - [`exception`]  — modelo unificado de unwinding.
//! - [`async_io`]   — Submission/Completion Queues estilo io_uring.

pub mod sync;
pub mod type_system;
pub mod vtable;
pub mod closure;
pub mod exception;
pub mod async_io;

// `calling` se re-exporta desde un solo archivo, no necesita sub-módulo.
pub mod calling;
