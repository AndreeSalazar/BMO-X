//! `defense` — ByteDefender: El Escudo de FastOS/BMO.
//!
//! v1.8.8: módulo hermano de `cabina` y `timeback` (la "Trilogía").
//! ByteDefender analiza y protege:
//!
//! - **Pre-execution**: BEF antes de cargar (headers, relocations, imports,
//!   capabilities, secciones W/X).
//! - **Runtime guard**: vigila syscalls peligrosas en Ring 3.
//!
//! ## Estructura
//!
//! ```text
//! defense/
//!   mod.rs         ← API pública + init()
//!   bytedefender.rs ← orquestador (pre-exec + runtime)
//!   policy.rs      ← reglas (qué se permite, qué se bloquea)
//!   scanner.rs     ← análisis estático de BEF
//!   verifier.rs    ← valida integridad (hash, firma)
//!   capability.rs  ← capabilities por proceso
//!   report.rs      ← SecurityReport
//!   quarantine.rs  ← apps en cuarentena
//! ```
//!
//! ## Regla de oro
//!
//! - ByteDefender **no pinta UI**. Solo analiza y reporta.
//! - Cabina muestra los reportes de ByteDefender.
//! - TimeBack puede crear checkpoints antes de ejecutar apps defendidas.

#![allow(dead_code)]

pub mod bytedefender;
pub mod policy;
pub mod scanner;
pub mod verifier;
pub mod capability;
pub mod report;
pub mod quarantine;

pub use bytedefender::ByteDefender;
pub use policy::{Policy, PolicyAction};
pub use report::{SecurityReport, Verdict};
pub use capability::{Capability, CapabilitySet};
pub use quarantine::Quarantine;

use core::sync::atomic::{AtomicBool, Ordering};

static INIT: AtomicBool = AtomicBool::new(false);

/// Inicializa ByteDefender. Llamar una vez al boot.
pub fn init() {
    if INIT.swap(true, Ordering::SeqCst) { return; }
    policy::init();
    capability::init();
    quarantine::init();
}

/// Decisión final sobre un BEF.
pub fn inspect_bef(name: &str, bytes: &[u8]) -> Verdict {
    let r = scanner::scan(name, bytes);
    if !r.is_well_formed() { return Verdict::Reject("malformed BEF".into()); }
    if !verifier::checksum_ok(bytes) { return Verdict::Reject("checksum mismatch".into()); }
    let req_caps = capability::required_from_bef(bytes);
    if !policy::has_all_caps(&req_caps) {
        return Verdict::Quarantine("missing capabilities".into());
    }
    if scanner::has_wx_section(bytes) {
        return Verdict::Quarantine("W+X section detected".into());
    }
    Verdict::Allow
}

/// Hook de runtime: llamado desde cada syscall peligroso.
pub fn on_syscall(nr: u16, _arg0: u64, _arg1: u64) -> PolicyAction {
    policy::check_syscall(nr)
}
