//! `timeback::tree` — Filesystem tree (Git-like).
//!
//! A Tree is a collection of named entries. Each entry is either:
//! - A blob (file content)
//! - Another tree (subdirectory)
//!
//! Trees are content-addressed by their hash. The repo stores them as
//! serialized blobs in objects/<aa>/<rest>.

use super::hash::Hash;
use alloc::string::String;
use alloc::vec::Vec;

/// File mode (Git-compatible subset).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FileMode {
    /// Regular file, executable.
    Blaze,
    /// Regular file.
    Normal,
    /// Symbolic link.
    Symlink,
    /// Subdirectory (tree).
    Tree,
}

impl FileMode {
    pub fn as_str(&self) -> &'static str {
        match self {
            FileMode::Blaze => "100755",
            FileMode::Normal => "100644",
            FileMode::Symlink => "120000",
            FileMode::Tree => "040000",
        }
    }
    pub fn from_str(s: &str) -> Option<Self> {
        match s {
            "100755" => Some(FileMode::Blaze),
            "100644" => Some(FileMode::Normal),
            "120000" => Some(FileMode::Symlink),
            "040000" => Some(FileMode::Tree),
            _ => None,
        }
    }
}

/// A single tree entry.
#[derive(Clone, Debug)]
pub struct TreeEntry {
    pub mode: FileMode,
    pub name: String,
    pub hash: Hash,
}

/// A tree (directory) in the filesystem.
#[derive(Clone, Debug)]
pub struct Tree {
    pub hash: Hash,
    pub entries: Vec<TreeEntry>,
}

impl Tree {
    /// Create an empty tree.
    pub fn empty() -> Self {
        let mut t = Self { hash: Hash::ZERO, entries: Vec::new() };
        t.hash = t.compute_hash();
        t
    }

    /// Create a tree from entries.
    pub fn from_entries(entries: Vec<TreeEntry>) -> Self {
        let mut t = Self { hash: Hash::ZERO, entries };
        t.hash = t.compute_hash();
        t
    }

    /// Compute the hash from the serialized form.
    pub fn compute_hash(&self) -> Hash {
        Hash::of(&self.serialize())
    }

    /// Serialize: "tree\n<mode> <name> <hash>\n..." per line.
    pub fn serialize(&self) -> Vec<u8> {
        let mut buf = Vec::with_capacity(128);
        buf.extend_from_slice(b"tree\n");
        // Sort entries by name for deterministic hashing
        let mut sorted = self.entries.clone();
        sorted.sort_by(|a, b| a.name.cmp(&b.name));
        for e in &sorted {
            buf.extend_from_slice(e.mode.as_str().as_bytes());
            buf.push(b' ');
            buf.extend_from_slice(e.name.as_bytes());
            buf.push(b' ');
            buf.extend_from_slice(e.hash.to_hex().as_bytes());
            buf.push(b'\n');
        }
        buf
    }

    /// Parse from serialized form.
    pub fn parse(data: &[u8]) -> Option<Self> {
        let mut lines = data.split(|&b| b == b'\n');
        let header = lines.next()?;
        if header != b"tree" { return None; }
        let mut entries = Vec::new();
        for line in lines {
            if line.is_empty() { continue; }
            let mut parts = line.splitn(3, |&b| b == b' ');
            let mode_str = core::str::from_utf8(parts.next()?).ok()?;
            let name_bytes = parts.next()?;
            let hash_str = core::str::from_utf8(parts.next()?).ok()?;
            let name = String::from_utf8(name_bytes.to_vec()).ok()?;
            let hash = Hash::from_hex(hash_str.trim())?;
            let mode = FileMode::from_str(mode_str)?;
            entries.push(TreeEntry { mode, name, hash });
        }
        let mut t = Self { hash: Hash::ZERO, entries };
        t.hash = t.compute_hash();
        Some(t)
    }

    /// Find an entry by name.
    pub fn find(&self, name: &str) -> Option<&TreeEntry> {
        self.entries.iter().find(|e| e.name == name)
    }
}
