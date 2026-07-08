//! `timeback::commit` — Commit graph (Git-like).
//!
//! Each commit has:
//! - A SHA-1-like hash (20 bytes, FNV-1a derived from content)
//! - A parent commit hash (or None for root)
//! - A tree hash (root of the filesystem tree at this commit)
//! - Author + timestamp
//! - A commit message
//!
//! Commits are stored as serialized blobs in the SSD repo. The repo
//! is a directory structure on the data partition (T:):
//!
//!   T:/TIMEBACK/
//!     objects/<aa>/<bb><cc>...   (raw compressed commit/tree/blob)
//!     refs/heads/<name>          (branch ref -> commit hash)
//!     refs/tags/<name>           (tag ref -> commit hash)
//!     HEAD                       (current branch or commit hash)
//!     INDEX                      (staging area)
//!     CONFIG                     (repo config)

use super::hash::Hash;

/// Author of a commit.
#[derive(Clone, Debug)]
pub struct Author {
    pub name: alloc::string::String,
    pub email: alloc::string::String,
}

impl Author {
    pub const fn kernel() -> Self {
        Self { name: alloc::string::String::new(), email: alloc::string::String::new() }
    }
    pub fn new(name: &str, email: &str) -> Self {
        Self {
            name: alloc::string::String::from(name),
            email: alloc::string::String::from(email),
        }
    }
}

/// A commit in the graph.
#[derive(Clone, Debug)]
pub struct Commit {
    pub hash: Hash,
    pub parent: Option<Hash>,
    pub tree: Hash,
    pub author: Author,
    pub timestamp_ns: u64,
    pub message: alloc::string::String,
}

impl Commit {
    /// Create a new commit (does not store it yet).
    pub fn new(
        parent: Option<Hash>,
        tree: Hash,
        author: Author,
        timestamp_ns: u64,
        message: &str,
    ) -> Self {
        let mut c = Self {
            hash: Hash::ZERO,
            parent,
            tree,
            author,
            timestamp_ns,
            message: alloc::string::String::from(message),
        };
        c.hash = c.compute_hash();
        c
    }

    /// Compute the hash from content (FNV-1a over serialized form).
    pub fn compute_hash(&self) -> Hash {
        let serialized = self.serialize();
        Hash::of(&serialized)
    }

    /// Serialize to a byte buffer for storage.
    pub fn serialize(&self) -> alloc::vec::Vec<u8> {
        let mut buf = alloc::vec::Vec::with_capacity(256);
        // Header line: "commit <tree> <parent|none> <timestamp>\n"
        buf.extend_from_slice(b"commit ");
        for &b in self.tree.bytes() { buf.push(b); }
        buf.push(b' ');
        match &self.parent {
            Some(p) => { for &b in p.bytes() { buf.push(b); } },
            None => buf.extend_from_slice(b"none"),
        }
        buf.push(b' ');
        for &b in self.timestamp_ns.to_le_bytes().iter() { buf.push(b); }
        buf.push(b'\n');
        // Author
        buf.extend_from_slice(b"author ");
        buf.extend_from_slice(self.author.name.as_bytes());
        buf.push(b' ');
        buf.extend_from_slice(self.author.email.as_bytes());
        buf.push(b'\n');
        // Message
        buf.extend_from_slice(b"msg ");
        buf.extend_from_slice(self.message.as_bytes());
        buf.push(b'\n');
        buf
    }

    /// Parse from serialized form.
    pub fn parse(data: &[u8]) -> Option<Self> {
        let mut lines = data.split(|&b| b == b'\n');
        let header = lines.next()?;
        let mut parts = header.split(|&b| b == b' ').filter(|s| !s.is_empty());
        if parts.next()? != b"commit" { return None; }
        let tree_bytes = parts.next()?;
        if tree_bytes.len() != 40 { return None; }
        let tree = Hash::from_hex(core::str::from_utf8(tree_bytes).ok()?)?;
        let parent_str = parts.next()?;
        let parent = if parent_str == b"none" {
            None
        } else {
            Some(Hash::from_hex(core::str::from_utf8(parent_str).ok()?)?)
        };
        let ts_bytes = parts.next()?;
        if ts_bytes.len() != 8 { return None; }
        let ts_arr: [u8; 8] = ts_bytes.try_into().ok()?;
        let timestamp_ns = u64::from_le_bytes(ts_arr);

        let author_line = lines.next()?;
        let mut a = author_line.split(|&b| b == b' ').filter(|s| !s.is_empty());
        if a.next()? != b"author" { return None; }
        let name = alloc::string::String::from_utf8(a.next()?.to_vec()).ok()?;
        let email = alloc::string::String::from_utf8(a.next()?.to_vec()).ok()?;

        let msg_line = lines.next()?;
        let mut m = msg_line.split(|&b| b == b' ').filter(|s| !s.is_empty());
        if m.next()? != b"msg" { return None; }
        let message = alloc::string::String::from_utf8(m.next()?.to_vec()).ok()?;

        let mut c = Self {
            hash: Hash::ZERO,
            parent,
            tree,
            author: Author { name, email },
            timestamp_ns,
            message,
        };
        c.hash = c.compute_hash();
        Some(c)
    }
}
