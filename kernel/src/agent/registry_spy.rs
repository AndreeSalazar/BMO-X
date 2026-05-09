//! Registry Spy — Extracts crypto keys, driver list, and machine info from NT hives.
//!
//! Uses raw hive binary parsing (nt-hive compatible structures) in no_std.
//! Reads SYSTEM and SOFTWARE hives previously loaded into memory.

use alloc::string::String;
use alloc::vec::Vec;
use alloc::format;

/// Information extracted from the SYSTEM hive.
pub struct SystemHiveInfo {
    pub hostname: String,
    pub drivers: Vec<DriverServiceEntry>,
}

/// Information extracted from the SOFTWARE hive.
pub struct SoftwareHiveInfo {
    pub machine_guid: String,
    pub product_name: String,
    pub build_lab: String,
    pub installed_certs: Vec<CertStoreEntry>,
}

/// A driver/service entry from SYSTEM\ControlSet001\Services.
pub struct DriverServiceEntry {
    pub service_name: String,
    pub image_path: String,
    pub start_type: u32,
    pub display_name: String,
}

/// A certificate entry from the registry certificate store.
pub struct CertStoreEntry {
    pub store_name: String,
    pub subject: String,
    pub thumbprint_hex: String,
}

/// Parse the SYSTEM registry hive from raw bytes.
/// This is a simplified parser that scans for known key patterns.
pub fn parse_system_hive(data: &[u8]) -> SystemHiveInfo {
    let hostname = find_registry_value_string(
        data,
        b"ComputerName",
    ).unwrap_or_else(|| String::from("[unknown]"));

    let drivers = extract_driver_services(data);

    SystemHiveInfo { hostname, drivers }
}

/// Parse the SOFTWARE registry hive from raw bytes.
pub fn parse_software_hive(data: &[u8]) -> SoftwareHiveInfo {
    let machine_guid = find_registry_value_string(
        data,
        b"MachineGuid",
    ).unwrap_or_else(|| String::from("[unknown]"));

    let product_name = find_registry_value_string(
        data,
        b"ProductName",
    ).unwrap_or_else(|| String::from("[unknown]"));

    let build_lab = find_registry_value_string(
        data,
        b"BuildLab",
    ).unwrap_or_else(|| String::from("[unknown]"));

    // Certificate stores are binary blobs; we just note their presence
    let installed_certs = scan_cert_stores(data);

    SoftwareHiveInfo {
        machine_guid,
        product_name,
        build_lab,
        installed_certs,
    }
}

/// Scan raw hive bytes for a UTF-16LE or ASCII value associated with a key name.
/// This is a brute-force scanner — works without full hive parsing.
fn find_registry_value_string(hive: &[u8], value_name: &[u8]) -> Option<String> {
    // Registry hives store value names as ASCII or UTF-16LE.
    // We scan for the ASCII value name and then look for nearby string data.

    let name_len = value_name.len();
    if hive.len() < name_len + 32 {
        return None;
    }

    let mut i = 0;
    while i + name_len + 32 < hive.len() {
        // Case-insensitive match for the value name
        if hive[i..i + name_len].eq_ignore_ascii_case(value_name) {
            // Found the name — look for string data after it
            // In a vk record, the data follows after the name + some header fields
            // Try to read nearby data as a string
            let search_start = i + name_len;
            let search_end = (search_start + 256).min(hive.len());

            // Look for printable ASCII/UTF-16LE string
            if let Some(s) = try_read_utf16le_string(&hive[search_start..search_end]) {
                if !s.is_empty() {
                    return Some(s);
                }
            }
            if let Some(s) = try_read_ascii_string(&hive[search_start..search_end]) {
                if !s.is_empty() {
                    return Some(s);
                }
            }
        }
        i += 1;
    }

    None
}

/// Try to read a null-terminated UTF-16LE string from bytes.
fn try_read_utf16le_string(data: &[u8]) -> Option<String> {
    let mut chars = Vec::new();
    let mut i = 0;
    let mut found_printable = false;

    while i + 1 < data.len() && chars.len() < 128 {
        let lo = data[i];
        let hi = data[i + 1];

        if lo == 0 && hi == 0 {
            break; // Null terminator
        }

        let ch = u16::from_le_bytes([lo, hi]);
        if ch >= 0x20 && ch < 0x7F {
            chars.push(ch as u8 as char);
            found_printable = true;
        } else if found_printable {
            break; // End of string
        }

        i += 2;
    }

    if found_printable && chars.len() >= 3 {
        Some(chars.iter().collect())
    } else {
        None
    }
}

/// Try to read a printable ASCII string from bytes.
fn try_read_ascii_string(data: &[u8]) -> Option<String> {
    let mut chars = Vec::new();
    let mut found_start = false;

    for &b in data.iter().take(128) {
        if b >= 0x20 && b < 0x7F {
            chars.push(b as char);
            found_start = true;
        } else if found_start {
            break;
        }
    }

    if chars.len() >= 3 {
        Some(chars.iter().collect())
    } else {
        None
    }
}

/// Scan for driver service entries by looking for common patterns.
fn extract_driver_services(hive: &[u8]) -> Vec<DriverServiceEntry> {
    let mut drivers = Vec::new();

    // Look for known driver names in the SYSTEM hive
    let known_drivers: &[(&[u8], &str)] = &[
        (b"nvlddmkm", "NVIDIA Display Driver"),
        (b"pci", "PCI Bus Driver"),
        (b"ksecdd", "Kernel Security Device"),
        (b"ntoskrnl", "NT Kernel"),
        (b"Tcpip", "TCP/IP Protocol"),
        (b"NDIS", "Network Driver Interface"),
        (b"disk", "Disk Driver"),
        (b"volmgr", "Volume Manager"),
        (b"Ntfs", "NTFS File System"),
        (b"iorate", "IO Rate Control"),
    ];

    for (name, display) in known_drivers {
        if contains_bytes_ci(hive, name) {
            let image_path = find_nearby_path(hive, name);
            drivers.push(DriverServiceEntry {
                service_name: String::from(core::str::from_utf8(name).unwrap_or("?")),
                image_path,
                start_type: 0, // Would need full hive parsing
                display_name: String::from(*display),
            });
        }
    }

    drivers
}

/// Case-insensitive search for a byte pattern in a larger buffer.
fn contains_bytes_ci(haystack: &[u8], needle: &[u8]) -> bool {
    if needle.is_empty() || haystack.len() < needle.len() {
        return false;
    }
    for i in 0..=haystack.len() - needle.len() {
        if haystack[i..i + needle.len()].eq_ignore_ascii_case(needle) {
            return true;
        }
    }
    false
}

/// Look for a file path (like \SystemRoot\System32\drivers\xxx.sys) near a name.
fn find_nearby_path(hive: &[u8], name: &[u8]) -> String {
    let name_len = name.len();
    let mut i = 0;
    while i + name_len < hive.len() {
        if hive[i..i + name_len].eq_ignore_ascii_case(name) {
            // Search nearby for a path string
            let start = i.saturating_sub(256);
            let end = (i + 512).min(hive.len());
            let region = &hive[start..end];

            // Look for "System32" or "drivers" as path indicator
            if let Some(path) = try_read_utf16le_string(region) {
                if path.contains("System32") || path.contains("system32") || path.contains("drivers") {
                    return path;
                }
            }
            break;
        }
        i += 1;
    }

    format!("System32\\drivers\\{}.sys", core::str::from_utf8(name).unwrap_or("?"))
}

/// Scan for certificate store entries in the SOFTWARE hive.
fn scan_cert_stores(hive: &[u8]) -> Vec<CertStoreEntry> {
    let mut certs = Vec::new();

    // Look for certificate blob markers
    // Root certificates in the registry are stored under
    // Microsoft\SystemCertificates\ROOT\Certificates\{thumbprint}
    // The thumbprint is a hex string used as the subkey name.

    let marker = b"Certificates";
    let mut i = 0;
    let mut found = 0;
    while i + marker.len() < hive.len() && found < 20 {
        if hive[i..i + marker.len()].eq_ignore_ascii_case(marker) {
            // Found a Certificates key reference — look for hex thumbprints nearby
            let search_end = (i + 512).min(hive.len());
            let region = &hive[i..search_end];
            if let Some(thumb) = find_hex_thumbprint(region) {
                certs.push(CertStoreEntry {
                    store_name: String::from("SystemCertificates"),
                    subject: String::from("[binary cert blob]"),
                    thumbprint_hex: thumb,
                });
                found += 1;
            }
        }
        i += 1;
    }

    certs
}

/// Look for a 40-character hex string (SHA-1 thumbprint) in a byte region.
fn find_hex_thumbprint(data: &[u8]) -> Option<String> {
    let mut hex_run = 0usize;
    let mut start = 0usize;

    for (i, &b) in data.iter().enumerate() {
        if b.is_ascii_hexdigit() {
            if hex_run == 0 {
                start = i;
            }
            hex_run += 1;
            if hex_run == 40 {
                let thumb_bytes = &data[start..start + 40];
                if let Ok(s) = core::str::from_utf8(thumb_bytes) {
                    return Some(String::from(s));
                }
            }
        } else {
            hex_run = 0;
        }
    }

    None
}
