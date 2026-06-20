//! `security` — Ring 0 Security Subsystem
//!
//! Contains:
//! - ByteDefender: Pre-execution antivirus
//! - Restaurer: Real-time kernel snapshots

#![allow(dead_code)]

pub mod bytedefender;
pub mod restaurer;

/// Initialize all security systems
pub fn init() {
    bytedefender::init();
    restaurer::init();

    crate::drivers::serial::serial_write("[security] All Ring 0 security systems active\n");
}

/// Get security status summary
pub fn status() -> SecurityStatus {
    SecurityStatus {
        bytedefender_enabled: bytedefender::state().enabled,
        files_scanned: bytedefender::state().files_scanned,
        threats_blocked: bytedefender::state().threats_blocked,
        restaurer_enabled: restaurer::state().enabled,
        snapshot_count: restaurer::state().snapshot_count,
        total_rollbacks: restaurer::state().total_rollbacks,
    }
}

pub struct SecurityStatus {
    pub bytedefender_enabled: bool,
    pub files_scanned: u64,
    pub threats_blocked: u64,
    pub restaurer_enabled: bool,
    pub snapshot_count: u64,
    pub total_rollbacks: u64,
}
