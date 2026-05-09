//! Forensic manifest (JSON index) generator.

use alloc::string::String;
use alloc::format;
use alloc::vec::Vec;

pub struct ManifestEntry {
    pub path: String,
    pub size: usize,
    pub description: String,
}

pub struct Manifest {
    pub entries: Vec<ManifestEntry>,
}

impl Manifest {
    pub fn new() -> Self {
        Self { entries: Vec::new() }
    }

    pub fn add_entry(&mut self, path: &str, size: usize, description: &str) {
        self.entries.push(ManifestEntry {
            path: String::from(path),
            size,
            description: String::from(description),
        });
    }

    pub fn to_json(&self) -> String {
        let mut json = String::from("{\n  \"files\": [\n");
        for (i, entry) in self.entries.iter().enumerate() {
            json.push_str("    {\n");
            json.push_str(&format!("      \"path\": \"{}\",\n", entry.path));
            json.push_str(&format!("      \"size\": {},\n", entry.size));
            json.push_str(&format!("      \"description\": \"{}\"\n", entry.description));
            json.push_str("    }");
            if i < self.entries.len() - 1 {
                json.push_str(",");
            }
            json.push_str("\n");
        }
        json.push_str("  ]\n}");
        json
    }
}
