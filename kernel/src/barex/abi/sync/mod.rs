//! `sync` — primitivas de sincronización del BMO ABI.
//!
//! Reemplaza `<stdatomic.h>`, `<threads.h>`, Win32 `Interlocked*` /
//! `CRITICAL_SECTION`, POSIX `pthread_mutex_t`. Modelo único, lock-free
//! cuando es posible, futex-backed cuando hay contención.

pub mod atomic;
pub mod futex;
pub mod mutex;

pub use atomic::{BmoAtomicU32, BmoAtomicU64, BmoAtomicBool, MemOrder};
pub use futex::{BmoFutex, FutexOp};
pub use mutex::BmoMutex;
