//! FastOS SSD partition layout (S/T/X).
//!
//! The 120GB SSD is divided into 3 named partitions:
//!
//!   S: FASTOS-EFI    (10 GB) — Boot partition. UEFI System Partition.
//!                          Contains BOOTX64.EFI, kernel.elf, crash.log.
//!                          FAT32. Mounted by UEFI firmware.
//!
//!   T: FastOS-Data   (~50 GB) — Data partition. Apps, user data, /home.
//!                          FAT32. Mounted by bmo_fat32 at runtime.
//!                          TimeBack can mirror or stage here.
//!
//!   X: Commit-Real   (~60 GB) — TimeBack git repo. Object store for
//!                          commits, trees, blobs, refs.
//!                          FAT32. Mounted by bmo_fat32 at runtime.
//!                          The "Git literal" of the kernel.
//!
//! This module exposes a `layout()` function that the welcome screen
//! can call to show the user which partition is which.

use core::sync::atomic::{AtomicBool, Ordering};

/// Is the partition layout system enabled?
static ENABLED: AtomicBool = AtomicBool::new(true);

/// Set whether the partition layout system is enabled.
pub fn set_enabled(on: bool) {
    ENABLED.store(on, Ordering::Relaxed);
}

/// Get whether the partition layout system is enabled.
pub fn is_enabled() -> bool {
    ENABLED.load(Ordering::Relaxed)
}

/// A single partition entry in the layout.
#[derive(Clone, Copy)]
pub struct Partition {
    /// Drive letter (e.g. b'S', b'T', b'X').
    pub letter: u8,
    /// Human label (null-terminated, max 15 chars + NUL).
    pub label: [u8; 16],
    /// Total size in MB (approx).
    pub size_mb: u32,
    /// Role: 0=EFI, 1=Data, 2=Commit-Real, 3=Other
    pub role: u8,
}

impl Partition {
    pub const fn new(letter: u8, label: &str, size_mb: u32, role: u8) -> Self {
        let mut l = [0u8; 16];
        let bytes = label.as_bytes();
        let n = if bytes.len() < 16 { bytes.len() } else { 15 };
        let mut i = 0;
        while i < n {
            l[i] = bytes[i];
            i += 1;
        }
        Self { letter, label: l, size_mb, role }
    }

    /// Get the label as a &str (truncated at first NUL).
    pub fn label_str(&self) -> &str {
        let mut end = 0;
        while end < self.label.len() && self.label[end] != 0 { end += 1; }
        core::str::from_utf8(&self.label[..end]).unwrap_or("?")
    }
}

/// The 3-partition layout.
pub const LAYOUT: [Partition; 3] = [
    Partition::new(b'S', "FASTOS-EFI", 10 * 1024, 0),
    Partition::new(b'T', "FastOS-Data", 50 * 1024, 1),
    Partition::new(b'X', "Commit-Real", 60 * 1024, 2),
];

/// Render the layout as a human-readable string.
pub fn layout_text() -> alloc::string::String {
    use alloc::string::String;
    use alloc::format;
    let mut s = String::from("SSD Partition Layout:\n");
    s.push_str("  Drive  Label            Size      Role\n");
    s.push_str("  -----  ---------------  --------  ---------\n");
    for p in LAYOUT.iter() {
        let role = match p.role {
            0 => "EFI/Boot",
            1 => "Data/Apps",
            2 => "Commit-Real",
            _ => "Other",
        };
        s.push_str(&format!(
            "  {}      {:<15}  {:>5} MB  {}\n",
            p.letter as char,
            p.label_str(),
            p.size_mb,
            role,
        ));
    }
    s
}

/// Look up a partition by its role.
pub fn by_role(role: u8) -> Option<&'static Partition> {
    LAYOUT.iter().find(|p| p.role == role)
}

/// Look up a partition by its drive letter.
pub fn by_letter(letter: u8) -> Option<&'static Partition> {
    LAYOUT.iter().find(|p| p.letter == letter)
}
