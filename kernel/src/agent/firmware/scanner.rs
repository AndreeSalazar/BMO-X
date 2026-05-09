//! DriverStore Firmware Scanner.
//!
//! Scans the Windows/System32/DriverStore/FileRepository for NVIDIA firmware files
//! (`gsp_*.bin`, `sec2.bin`, `nvfw`, etc.) and collects metadata.

use alloc::string::String;
use alloc::vec::Vec;
use crate::agent::firmware::metadata::FirmwareRecord;

/// Check if a given path within the DriverStore is a relevant NVIDIA firmware file.
pub fn is_nvidia_firmware_file(path: &str) -> bool {
    let lower = path.to_ascii_lowercase();
    // Look for Ampere firmware or general nvidia fw blobs
    lower.contains("gsp_ga10") ||
    lower.contains("sec2") ||
    lower.contains("pmu") ||
    lower.contains("fecs") ||
    lower.contains("gpccs") ||
    lower.ends_with(".bin") ||
    lower.ends_with(".rom") ||
    lower.ends_with(".fw")
}

/// Creates a FirmwareRecord from a found DriverStore file.
pub fn create_record_from_file(path: &str, size: u64, file_data: &[u8]) -> FirmwareRecord {
    // Extract base name from path
    let name = path.split('\\').last().unwrap_or(path);
    
    let mut record = FirmwareRecord::new(name, "DriverStore", size, false);
    
    // Simple GPU arch heuristics based on name
    let lower_name = name.to_ascii_lowercase();
    if lower_name.contains("ga10") {
        record.gpu_arch = String::from("Ampere");
    } else if lower_name.contains("tu10") {
        record.gpu_arch = String::from("Turing");
    } else if lower_name.contains("ad10") {
        record.gpu_arch = String::from("Ada");
    }

    // Try to extract version or hash based on the first few bytes
    // (In a full implementation, we'd SHA256 the file_data)
    if file_data.len() > 16 {
        // Just take a tiny sample as a pseudo-hash for now
        let mut pseudo_hash = String::new();
        for b in &file_data[0..8] {
            use core::fmt::Write;
            let _ = write!(&mut pseudo_hash, "{:02X}", b);
        }
        record.sha256 = pseudo_hash;
    }

    record
}
