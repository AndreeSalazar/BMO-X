//! Spy Manifest — JSON report generator for forensic intelligence.
//!
//! Generates a structured JSON report containing all extracted signatures,
//! certificates, drivers, and registry crypto information.

use alloc::string::String;
use alloc::vec::Vec;
use alloc::format;

use crate::agent::pe_sig::PeSignatureInfo;
use crate::agent::registry_spy::{DriverServiceEntry, CertStoreEntry};
use crate::agent::firmware::metadata::FirmwareRecord;
use crate::agent::firmware::registry::RegistryIntel;

/// Top-level spy report.
pub struct SpyReport {
    pub hostname: String,
    pub machine_guid: String,
    pub product_name: String,
    pub build_lab: String,
    pub signatures: Vec<SignatureRecord>,
    pub drivers: Vec<DriverRecord>,
    pub certificates: Vec<CertRecord>,
    pub firmware: Vec<FirmwareRecord>,
    pub gpu_registry: Vec<RegistryIntel>,
}

/// A file's Authenticode signature record.
pub struct SignatureRecord {
    pub file_path: String,
    pub file_size: u64,
    pub description: String,
    pub pe_timestamp: u32,
    pub machine_type: u16,
    pub sig_offset: u32,
    pub sig_size: u32,
    pub signer: String,
    pub issuer: String,
    pub cert_serial: String,
}

/// A driver service record from the registry.
pub struct DriverRecord {
    pub service_name: String,
    pub display_name: String,
    pub image_path: String,
    pub start_type: u32,
}

/// A certificate record.
pub struct CertRecord {
    pub store_name: String,
    pub subject: String,
    pub thumbprint: String,
}

impl SpyReport {
    pub fn new() -> Self {
        Self {
            hostname: String::new(),
            machine_guid: String::new(),
            product_name: String::new(),
            build_lab: String::new(),
            signatures: Vec::new(),
            drivers: Vec::new(),
            certificates: Vec::new(),
            firmware: Vec::new(),
            gpu_registry: Vec::new(),
        }
    }

    pub fn add_signature(
        &mut self,
        path: &str,
        size: u64,
        description: &str,
        info: &PeSignatureInfo,
    ) {
        self.signatures.push(SignatureRecord {
            file_path: String::from(path),
            file_size: size,
            description: String::from(description),
            pe_timestamp: info.pe_timestamp,
            machine_type: info.machine,
            sig_offset: info.signature_offset,
            sig_size: info.signature_size,
            signer: info.signer_name.clone(),
            issuer: info.issuer_name.clone(),
            cert_serial: info.cert_serial_hex.clone(),
        });
    }

    pub fn add_driver(&mut self, d: &DriverServiceEntry) {
        self.drivers.push(DriverRecord {
            service_name: d.service_name.clone(),
            display_name: d.display_name.clone(),
            image_path: d.image_path.clone(),
            start_type: d.start_type,
        });
    }

    pub fn add_cert(&mut self, c: &CertStoreEntry) {
        self.certificates.push(CertRecord {
            store_name: c.store_name.clone(),
            subject: c.subject.clone(),
            thumbprint: c.thumbprint_hex.clone(),
        });
    }

    pub fn add_firmware(&mut self, fw: FirmwareRecord) {
        self.firmware.push(fw);
    }

    pub fn add_gpu_registry(&mut self, reg: RegistryIntel) {
        self.gpu_registry.push(reg);
    }

    /// Generate the full JSON report.
    pub fn to_json(&self) -> String {
        let mut j = String::with_capacity(4096);

        j.push_str("{\n");

        // ── Machine info ──
        j.push_str("  \"agent\": \"FastOS Spy v1.0\",\n");
        j.push_str("  \"machine\": {\n");
        j.push_str(&format!("    \"hostname\": \"{}\",\n", json_escape(&self.hostname)));
        j.push_str(&format!("    \"machine_guid\": \"{}\",\n", json_escape(&self.machine_guid)));
        j.push_str(&format!("    \"product_name\": \"{}\",\n", json_escape(&self.product_name)));
        j.push_str(&format!("    \"build_lab\": \"{}\"\n", json_escape(&self.build_lab)));
        j.push_str("  },\n");

        // ── Signatures ──
        j.push_str("  \"signatures\": [\n");
        for (i, s) in self.signatures.iter().enumerate() {
            j.push_str("    {\n");
            j.push_str(&format!("      \"file\": \"{}\",\n", json_escape(&s.file_path)));
            j.push_str(&format!("      \"size\": {},\n", s.file_size));
            j.push_str(&format!("      \"description\": \"{}\",\n", json_escape(&s.description)));
            j.push_str(&format!("      \"pe_timestamp\": {},\n", s.pe_timestamp));
            j.push_str(&format!("      \"machine_type\": \"0x{:04X}\",\n", s.machine_type));
            j.push_str(&format!("      \"sig_offset\": {},\n", s.sig_offset));
            j.push_str(&format!("      \"sig_size\": {},\n", s.sig_size));
            j.push_str(&format!("      \"signer\": \"{}\",\n", json_escape(&s.signer)));
            j.push_str(&format!("      \"issuer\": \"{}\",\n", json_escape(&s.issuer)));
            j.push_str(&format!("      \"cert_serial\": \"{}\"\n", json_escape(&s.cert_serial)));
            j.push_str("    }");
            if i < self.signatures.len() - 1 { j.push(','); }
            j.push('\n');
        }
        j.push_str("  ],\n");

        // ── Drivers ──
        j.push_str("  \"drivers\": [\n");
        for (i, d) in self.drivers.iter().enumerate() {
            j.push_str("    {\n");
            j.push_str(&format!("      \"service\": \"{}\",\n", json_escape(&d.service_name)));
            j.push_str(&format!("      \"display_name\": \"{}\",\n", json_escape(&d.display_name)));
            j.push_str(&format!("      \"image_path\": \"{}\",\n", json_escape(&d.image_path)));
            j.push_str(&format!("      \"start_type\": {}\n", d.start_type));
            j.push_str("    }");
            if i < self.drivers.len() - 1 { j.push(','); }
            j.push('\n');
        }
        j.push_str("  ],\n");

        // ── Certificates ──
        j.push_str("  \"certificates\": [\n");
        for (i, c) in self.certificates.iter().enumerate() {
            j.push_str("    {\n");
            j.push_str(&format!("      \"store\": \"{}\",\n", json_escape(&c.store_name)));
            j.push_str(&format!("      \"subject\": \"{}\",\n", json_escape(&c.subject)));
            j.push_str(&format!("      \"thumbprint\": \"{}\"\n", json_escape(&c.thumbprint)));
            j.push_str("    }");
            if i < self.certificates.len() - 1 { j.push(','); }
            j.push('\n');
        }
        j.push_str("  ],\n");

        // ── Firmware ──
        j.push_str("  \"firmware\": [\n");
        for (i, f) in self.firmware.iter().enumerate() {
            j.push_str("    {\n");
            j.push_str(&format!("      \"name\": \"{}\",\n", json_escape(&f.name)));
            j.push_str(&format!("      \"source\": \"{}\",\n", json_escape(&f.source)));
            j.push_str(&format!("      \"gpu_arch\": \"{}\",\n", json_escape(&f.gpu_arch)));
            j.push_str(&format!("      \"version\": \"{}\",\n", json_escape(&f.version)));
            j.push_str(&format!("      \"sha256\": \"{}\",\n", json_escape(&f.sha256)));
            j.push_str(&format!("      \"size\": {},\n", f.size));
            j.push_str(&format!("      \"embedded\": {},\n", if f.embedded { "true" } else { "false" }));
            j.push_str(&format!("      \"related_driver\": \"{}\",\n", json_escape(&f.related_driver)));
            j.push_str(&format!("      \"windows_build\": \"{}\"\n", json_escape(&f.windows_build)));
            j.push_str("    }");
            if i < self.firmware.len() - 1 { j.push(','); }
            j.push('\n');
        }
        j.push_str("  ],\n");

        // ── GPU Registry ──
        j.push_str("  \"gpu_registry\": [\n");
        for (i, r) in self.gpu_registry.iter().enumerate() {
            j.push_str("    {\n");
            j.push_str(&format!("      \"key_path\": \"{}\",\n", json_escape(&r.key_path)));
            
            j.push_str("      \"feature_flags\": [\n");
            for (fi, (name, val)) in r.feature_flags.iter().enumerate() {
                j.push_str(&format!("        {{\"{}\": {}}}", json_escape(name), val));
                if fi < r.feature_flags.len() - 1 { j.push(','); }
                j.push('\n');
            }
            j.push_str("      ]\n");
            j.push_str("    }");
            if i < self.gpu_registry.len() - 1 { j.push(','); }
            j.push('\n');
        }
        j.push_str("  ]\n");

        j.push_str("}\n");
        j
    }
}

/// Escape special characters for JSON strings.
fn json_escape(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c if c < '\x20' => {
                out.push_str(&format!("\\u{:04x}", c as u32));
            }
            _ => out.push(c),
        }
    }
    out
}
