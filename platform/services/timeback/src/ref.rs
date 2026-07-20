//! `timeback::ref` — Branch and tag references (Git-like).

use super::hash::Hash;
use alloc::string::String;

/// A reference (branch or tag) — points to a commit hash.
#[derive(Clone, Debug)]
pub struct RefEntry {
    pub name: String,
    pub hash: Hash,
    pub kind: RefKind,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RefKind {
    Branch,
    Tag,
}

impl RefEntry {
    pub fn branch(name: &str, hash: Hash) -> Self {
        Self { name: String::from(name), hash, kind: RefKind::Branch }
    }
    pub fn tag(name: &str, hash: Hash) -> Self {
        Self { name: String::from(name), hash, kind: RefKind::Tag }
    }
}
