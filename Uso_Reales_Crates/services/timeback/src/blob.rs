//! `timeback::blob` — Raw file content (Git-like).
//!
//! A blob is just the raw bytes of a file, stored verbatim in the
//! object store and addressed by the hash of those bytes.

use super::hash::Hash;

/// Raw file content.
#[derive(Clone, Debug)]
pub struct Blob {
    pub hash: Hash,
    pub data: alloc::vec::Vec<u8>,
}

impl Blob {
    /// Create a blob from raw bytes.
    pub fn new(data: alloc::vec::Vec<u8>) -> Self {
        let hash = Hash::of(&data);
        Self { hash, data }
    }

    /// Compute the hash of the data.
    pub fn compute_hash(&self) -> Hash {
        Hash::of(&self.data)
    }

    /// Serialize for storage: "blob\n<data>".
    pub fn serialize(&self) -> alloc::vec::Vec<u8> {
        let mut buf = alloc::vec::Vec::with_capacity(5 + self.data.len());
        buf.extend_from_slice(b"blob\n");
        buf.extend_from_slice(&self.data);
        buf
    }

    /// Parse from serialized form.
    pub fn parse(data: &[u8]) -> Option<Self> {
        if data.len() < 5 || &data[0..5] != b"blob\n" { return None; }
        let content = data[5..].to_vec();
        let hash = Hash::of(&content);
        Some(Self { hash, data: content })
    }
}
