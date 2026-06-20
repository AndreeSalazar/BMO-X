//! Execution hooks for ByteDefender
//!
//! Intercepts file execution at the syscall level to enforce pre-execution scanning.

#![allow(dead_code)]

use super::Action;

/// Hook point for file execution
pub fn hook_exec(path: &[u8], path_len: usize) -> bool {
    let actual_path = if path_len <= path.len() {
        &path[..path_len]
    } else {
        path
    };

    // Don't scan system critical paths
    if is_system_path(actual_path) {
        return true;
    }

    // Perform pre-execution scan
    let result = super::scanner::scan_file(actual_path);

    match result.recommended_action {
        Action::Allow => true,
        Action::Block => {
            crate::drivers::serial::serial_write("[bytedefender] BLOCKED: ");
            write_slice(actual_path);
            crate::drivers::serial::serial_write("\n");
            false
        }
        Action::Alert => {
            // Allow but alert
            crate::drivers::serial::serial_write("[bytedefender] ALERT: ");
            write_slice(actual_path);
            crate::drivers::serial::serial_write("\n");
            true
        }
        Action::Quarantine => {
            quarantine_file(actual_path);
            false
        }
    }
}

/// Hook point for dynamic code loading (e.g., DLL injection detection)
pub fn hook_load_library(base: u64, size: u64) -> bool {
    if size == 0 { return true; }

    // Read the loaded memory region
    let data = unsafe {
        core::slice::from_raw_parts(base as *const u8, size as usize)
    };

    let result = super::scanner::scan_memory(data);

    result.level == super::ThreatLevel::Clean || result.level == super::ThreatLevel::Low
}

/// Check if path is system-critical (skip scanning)
fn is_system_path(path: &[u8]) -> bool {
    let system_paths: &[&[u8]] = &[
        b"\\system\\",
        b"\\boot\\",
        b"\\efi\\",
        b"bmo:\\system",
        b"bmo:\\boot",
        b"bmo:\\efi",
    ];

    for sys_path in system_paths {
        if path_contains(path, sys_path) {
            return true;
        }
    }

    false
}

fn path_contains(haystack: &[u8], needle: &[u8]) -> bool {
    if needle.len() > haystack.len() { return false; }
    for i in 0..=haystack.len() - needle.len() {
        if &haystack[i..i + needle.len()] == needle {
            return true;
        }
    }
    false
}

/// Write a byte slice to serial
fn write_slice(data: &[u8]) {
    for &byte in data {
        crate::drivers::serial::serial_write_byte(byte);
    }
}

/// Quarantine a suspicious file (move to quarantine area)
fn quarantine_file(path: &[u8]) {
    // In production: move file to quarantine directory
    // For now: log the quarantine attempt
    crate::drivers::serial::serial_write("[bytedefender] QUARANTINE: ");
    write_slice(path);
    crate::drivers::serial::serial_write("\n");

    crate::bmo_core::diag::info("bytedefender", "File quarantined");
}

/// Integrity check: verify file hasn't been tampered with
pub fn verify_integrity(data: &[u8], expected_hash: &[u8; 32]) -> bool {
    let actual_hash = super::scanner::compute_hash(data);
    &actual_hash == expected_hash
}
