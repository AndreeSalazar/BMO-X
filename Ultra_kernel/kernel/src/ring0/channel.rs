//! BMO Channel — stub.
//!
//! The legacy lock-free Ring 0↔Ring 3 IPC over a shared page is
//! replaced by a no-op for the Ring 0 base. Real IPC is a future-phase
//! feature.

pub fn init() {}
pub fn register<T>(_channel: *mut T) -> bool { false }
pub fn process_now() -> usize { 0 }
pub fn tick_all() {}
