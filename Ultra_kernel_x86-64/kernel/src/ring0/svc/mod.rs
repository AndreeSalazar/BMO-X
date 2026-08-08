//! Ring 0 capability services transported over BMO Channel.
//!
//! Each service binds to one estuary via `channel::register_service` and
//! speaks its own opcode space. Services never interpret user pointers --
//! everything travels by value in the `(opcode, a0, a1, a2)` entries.
//! Anything needing bulk data moves to Ring 3 servers (F4) with shared
//! memory granted by capability.

pub mod system;

/// Estuary assignments. Estuary 0 is the system service; the rest are
/// free for Ring 3 servers to claim as F4 lands.
pub const ESTUARY_SYSTEM: usize = 0;

/// Bind every built-in Ring 0 service. Boot-time only (pre-timer).
pub fn register_all() {
    crate::ring0::obj::channel::register_service(ESTUARY_SYSTEM, system::handle);
}
