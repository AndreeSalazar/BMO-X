//! Firmware knowledge database metadata structures.

use alloc::string::String;
use alloc::vec::Vec;
use core::fmt;

/// A record representing a piece of NVIDIA firmware knowledge.
#[derive(Clone)]
pub struct FirmwareRecord {
    pub name: String,
    pub source: String,       // e.g., "DriverStore", "Embedded"
    pub gpu_arch: String,     // e.g., "GA106", "Ampere"
    pub version: String,
    pub sha256: String,       // Hex string
    pub size: u64,
    pub embedded: bool,
    pub related_driver: String,
    pub windows_build: String,
}

impl FirmwareRecord {
    pub fn new(name: &str, source: &str, size: u64, embedded: bool) -> Self {
        Self {
            name: String::from(name),
            source: String::from(source),
            gpu_arch: String::from("Unknown"),
            version: String::from("Unknown"),
            sha256: String::from("pending"),
            size,
            embedded,
            related_driver: String::from("Unknown"),
            windows_build: String::from("Unknown"),
        }
    }
}
