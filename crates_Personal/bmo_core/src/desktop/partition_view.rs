//! SSD partition layout display for the welcome screen.
//!
//! Shows the user the 3-partition layout (S: FASTOS-EFI, T: FastOS-Data,
//! X: Commit-Real) when they type `layout` in the CABINA.

extern crate alloc;

use alloc::string::String;

/// A single partition entry for display.
pub struct PartInfo {
    pub letter: char,
    pub label: String,
    pub size_mb: u32,
    pub role: PartRole,
}

#[derive(Clone, Copy)]
pub enum PartRole {
    Efi,
    Data,
    CommitReal,
    Other,
}

impl PartRole {
    pub fn as_str(&self) -> &'static str {
        match self {
            PartRole::Efi => "EFI/Boot",
            PartRole::Data => "Data/Apps",
            PartRole::CommitReal => "Commit-Real",
            PartRole::Other => "Other",
        }
    }
}

/// Render the layout as a human-readable string.
pub fn layout_text() -> String {
    use core::fmt::Write;
    let mut s = String::from("SSD Partition Layout:\n");
    s.push_str("  Drive  Label            Size      Role\n");
    s.push_str("  -----  ---------------  --------  ---------\n");
    let parts: [PartInfo; 3] = [
        PartInfo { letter: 'S', label: String::from("FASTOS-EFI"),  size_mb: 10 * 1024, role: PartRole::Efi },
        PartInfo { letter: 'T', label: String::from("FastOS-Data"), size_mb: 50 * 1024, role: PartRole::Data },
        PartInfo { letter: 'X', label: String::from("Commit-Real"), size_mb: 60 * 1024, role: PartRole::CommitReal },
    ];
    for p in parts.iter() {
        let _ = write!(s, "  {}      {:<15}  {:>5} MB  {}\n",
            p.letter, p.label, p.size_mb, p.role.as_str());
    }
    s
}
