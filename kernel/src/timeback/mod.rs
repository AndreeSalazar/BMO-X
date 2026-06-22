//! `timeback` — TimeBack: El Reloj de FastOS/BMO.
//!
//! v1.8.8: módulo hermano de `cabina` y `defense` (la "Trilogía").
//! TimeBack permite retroceder en el tiempo:
//!
//! - **Checkpoints**: puntos de retorno nombrados.
//! - **Snapshots**: estado del sistema en un instante.
//! - **Deltas**: cambios incrementales entre snapshots.
//! - **Journal**: log de operaciones para reproducir o revertir.
//! - **Rollback**: volver a un checkpoint anterior.
//!
//! ## Estructura
//!
//! ```text
//! timeback/
//!   mod.rs         ← API pública + init()
//!   checkpoint.rs  ← crear/listar/borrar checkpoints
//!   snapshot.rs    ← captura del estado
//!   delta.rs       ← diff entre snapshots
//!   journal.rs     ← log de operaciones
//!   rollback.rs    ← revertir a un checkpoint
//!   storage.rs     ← dónde se guardan los snapshots
//!   policy.rs      ← reglas de retención
//! ```
//!
//! ## Regla de oro
//!
//! - TimeBack **no decide políticas de seguridad** (eso es de ByteDefender).
//! - Cabina puede pedir un rollback desde el HUD.
//! - ByteDefender puede crear un checkpoint antes de ejecutar una app.
//!
//! ## v1.8.8: estado
//!
//! - API completa (stubs).
//! - Storage en RAM (sin USB/FS todavía).
//! - Journal en ring buffer.

#![allow(dead_code)]

pub mod checkpoint;
pub mod snapshot;
pub mod delta;
pub mod journal;
pub mod rollback;
pub mod storage;
pub mod policy;
pub mod tests;

pub use checkpoint::CheckpointId;
pub use snapshot::Snapshot;
pub use delta::Delta;
pub use journal::{JournalEntry, JournalOp};
pub use rollback::RollbackResult;

use core::sync::atomic::{AtomicBool, AtomicU64, Ordering};

static INIT: AtomicBool = AtomicBool::new(false);
static CURRENT_EPOCH: AtomicU64 = AtomicU64::new(0);

/// Inicializa TimeBack. Llamar una vez al boot.
pub fn init() {
    if INIT.swap(true, Ordering::SeqCst) { return; }
    storage::init();
    journal::init();
    policy::init();
}

/// Época actual (monotónicamente creciente). Se incrementa en cada checkpoint.
pub fn current_epoch() -> u64 { CURRENT_EPOCH.load(Ordering::SeqCst) }

/// Crea un checkpoint con un nombre. Retorna el ID.
pub fn create_checkpoint(name: &str) -> CheckpointId {
    let id = checkpoint::create(name);
    CURRENT_EPOCH.fetch_add(1, Ordering::SeqCst);
    id
}

/// Revierte el sistema a un checkpoint. Retorna el resultado.
pub fn rollback(id: CheckpointId) -> RollbackResult {
    rollback::to(id)
}
