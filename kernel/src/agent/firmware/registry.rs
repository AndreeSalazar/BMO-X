//! NVIDIA Registry Analyzer.
//!
//! Extracts GPU configurations, firmware paths, and feature flags from the SYSTEM hive
//! under `CurrentControlSet\Services\nvlddmkm`.

use alloc::string::String;
use alloc::vec::Vec;

/// A record representing an NVIDIA registry configuration.
#[derive(Clone)]
pub struct RegistryIntel {
    pub key_path: String,
    pub feature_flags: Vec<(String, u32)>,
    pub string_values: Vec<(String, String)>,
}

/// Parses the SYSTEM hive to find `nvlddmkm` configurations.
pub fn extract_nvidia_registry_intel(hive_data: &[u8]) -> Option<RegistryIntel> {
    // In a real implementation, this would walk the hive structure.
    // We will do a brute force pattern scan similar to `registry_spy::find_registry_value_string`.
    
    let mut intel = RegistryIntel {
        key_path: String::from("SYSTEM\\CurrentControlSet\\Services\\nvlddmkm"),
        feature_flags: Vec::new(),
        string_values: Vec::new(),
    };

    // Example feature flags we might search for (basic pattern matching for now)
    let features_to_find: &[&[u8]] = &[
        b"RmFirmware",
        b"GspEnable",
        b"Sec2Enable",
    ];

    for feat in features_to_find {
        // Brute force search
        for i in 0..hive_data.len().saturating_sub(feat.len()) {
            if &hive_data[i..i+feat.len()] == *feat {
                let name = String::from(core::str::from_utf8(feat).unwrap_or("Unknown"));
                intel.feature_flags.push((name, 1)); // Dummy value 1
                break;
            }
        }
    }

    if intel.feature_flags.is_empty() && intel.string_values.is_empty() {
        None
    } else {
        Some(intel)
    }
}
