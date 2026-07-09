//! `timeback::repo` — Repository (the top-level API).
//!
//! A repo is a directory on the SSD (T: partition typically) that
//! stores all objects, refs, and HEAD. The API mirrors Git:
//!
//!   timeback::init("T:/TIMEBACK")        — create empty repo
//!   timeback::commit("msg", &author)      — create commit from index
//!   timeback::log()                      — walk HEAD history
//!   timeback::branch("name")             — create branch
//!   timeback::checkout("ref")            — switch branch/commit
//!   timeback::status()                   — show working tree status
//!   timeback::add("path")                — stage file
//!
//! All operations persist to SSD via the storage backend.

extern crate alloc;

use alloc::collections::BTreeMap;
use alloc::string::{String, ToString};
use alloc::vec::Vec;
use core::sync::atomic::{AtomicBool, Ordering};

use super::blob::Blob;
use super::commit::{Author, Commit};
use super::hash::Hash;
use super::r#ref::{RefEntry, RefKind};
use super::storage;
use super::tree::{FileMode, Tree, TreeEntry};

static REPO_PATH: spin::Mutex<Option<String>> = spin::Mutex::new(None);
static REPO_INITIALIZED: AtomicBool = AtomicBool::new(false);
static HEAD_REF: spin::Mutex<Option<String>> = spin::Mutex::new(None); // branch name or detached commit

/// Initialize a new repo at the given path. Path is something like
/// "T:/TIMEBACK" — the storage layer will mount the FAT32 volume and
/// create the directory structure.
pub fn init(path: &str) -> bool {
    let mut p = REPO_PATH.lock();
    *p = Some(String::from(path));
    drop(p);

    if storage::ensure_repo_dir(path) {
        REPO_INITIALIZED.store(true, Ordering::SeqCst);
        // Default branch = "main", HEAD = "main"
        let mut h = HEAD_REF.lock();
        *h = Some(String::from("main"));
        drop(h);
        storage::write_ref(path, "HEAD", "main");
        storage::write_ref(path, "refs/heads/main", "0000000000000000000000000000000000000000");
        true
    } else {
        false
    }
}

/// Is the repo initialized?
pub fn is_initialized() -> bool {
    REPO_INITIALIZED.load(Ordering::SeqCst)
}

/// Stage a file blob and add it to the index.
pub fn add(path: &str, content: &[u8]) -> Option<Hash> {
    let guard = REPO_PATH.lock();
    let rpath = guard.as_ref()?.clone();
    drop(guard);

    let blob = Blob::new(content.to_vec());
    if !storage::write_object(&rpath, &blob.hash, &blob.serialize()) { return None; }
    if !storage::index_add(&rpath, path, &blob.hash) { return None; }
    Some(blob.hash)
}

/// Create a commit from the current index.
pub fn commit(message: &str, author: Author) -> Option<Hash> {
    let guard = REPO_PATH.lock();
    let rpath = guard.as_ref()?.clone();
    drop(guard);

    // Build the tree from the index
    let tree = storage::index_to_tree(&rpath)?;
    if !storage::write_object(&rpath, &tree.hash, &tree.serialize()) { return None; }

    // Find parent (current HEAD commit)
    let head_ref = HEAD_REF.lock();
    let head_name = head_ref.clone();
    drop(head_ref);
    let parent = storage::read_ref(&rpath, &head_name.as_deref().unwrap_or("HEAD"))
        .and_then(|h| Hash::from_hex(&h))
        .filter(|h| !h.is_zero());

    let ts = storage::now_ns();
    let commit = Commit::new(parent, tree.hash, author, ts, message);
    if !storage::write_object(&rpath, &commit.hash, &commit.serialize()) { return None; }

    // Update HEAD ref
    if let Some(ref name) = head_name {
        storage::write_ref(&rpath, name, &commit.hash.to_hex());
    } else {
        storage::write_ref(&rpath, "HEAD", &commit.hash.to_hex());
    }

    // Clear the index
    storage::index_clear(&rpath);

    Some(commit.hash)
}

/// Walk the history from HEAD.
pub fn log() -> Vec<Commit> {
    let mut commits = Vec::new();
    let rpath = match REPO_PATH.lock().clone() {
        Some(r) => r,
        None => return commits,
    };
    let head_ref = HEAD_REF.lock().clone();
    let head_name = head_ref.unwrap_or_else(|| String::from("HEAD"));
    let head_str = match storage::read_ref(&rpath, &head_name) {
        Some(h) => h,
        None => return commits,
    };
    let mut current = match Hash::from_hex(&head_str) {
        Some(h) if !h.is_zero() => h,
        _ => return commits,
    };
    for _ in 0..1000 {
        let data = match storage::read_object(&rpath, &current) {
            Some(d) => d,
            None => break,
        };
        let commit = match Commit::parse(&data) {
            Some(c) => c,
            None => break,
        };
        let parent = commit.parent.clone();
        commits.push(commit);
        match parent {
            Some(p) => current = p,
            None => break,
        }
    }
    commits
}

/// Create a new branch at the current commit.
pub fn branch(name: &str) -> bool {
    let rpath = match REPO_PATH.lock().clone() {
        Some(r) => r,
        None => return false,
    };
    let head_ref = HEAD_REF.lock().clone();
    let head_name = head_ref.unwrap_or_else(|| String::from("HEAD"));
    let current = match storage::read_ref(&rpath, &head_name) {
        Some(h) => h,
        None => return false,
    };
    storage::write_ref(&rpath, &alloc::format!("refs/heads/{}", name), &current)
}

/// Checkout a branch or commit (detached HEAD).
pub fn checkout(target: &str) -> bool {
    let rpath = match REPO_PATH.lock().clone() {
        Some(r) => r,
        None => return false,
    };
    // Try as branch first
    let branch_ref = alloc::format!("refs/heads/{}", target);
    let hash = storage::read_ref(&rpath, &branch_ref)
        .or_else(|| storage::read_ref(&rpath, target));
    match hash {
        Some(h) if !h.is_empty() => {
            let mut h = HEAD_REF.lock();
            *h = Some(String::from(target));
            drop(h);
            storage::write_ref(&rpath, "HEAD", target);
            true
        }
        _ => false,
    }
}

/// Get the current HEAD commit hash.
pub fn head() -> Option<Hash> {
    let rpath = REPO_PATH.lock().clone()?;
    let head_ref = HEAD_REF.lock().clone();
    let head_name = head_ref.unwrap_or_else(|| String::from("HEAD"));
    let h = storage::read_ref(&rpath, &head_name)?;
    Hash::from_hex(&h)
}

/// List all branches.
pub fn branches() -> Vec<RefEntry> {
    let mut out = Vec::new();
    let rpath = match REPO_PATH.lock().clone() {
        Some(r) => r,
        None => return out,
    };
    let refs = storage::list_refs(&rpath, "refs/heads/");
    for (name, hash_str) in refs {
        if let Some(h) = Hash::from_hex(&hash_str) {
            out.push(RefEntry::branch(&name, h));
        }
    }
    out
}

/// Show the working tree status.
pub fn status() -> String {
    let mut s = String::new();
    s.push_str("On branch: ");
    let head_name = HEAD_REF.lock().clone().unwrap_or_else(|| String::from("(none)"));
    s.push_str(&head_name);
    s.push('\n');
    if let Some(h) = head() {
        s.push_str(&alloc::format!("HEAD: {}\n", h.short()));
    }
    let rpath = REPO_PATH.lock().clone();
    if let Some(r) = rpath {
        let index = storage::index_list(&r);
        s.push_str(&alloc::format!("Staged: {} file(s)\n", index.len()));
    }
    s
}

/// Diff between two commits (returns textual diff of file lists).
pub fn diff(a: Hash, b: Hash) -> Vec<(String, DiffOp)> {
    let mut out = Vec::new();
    let rpath = match REPO_PATH.lock().clone() {
        Some(r) => r,
        None => return out,
    };
    let tree_a = storage::read_object(&rpath, &a)
        .and_then(|d| Tree::parse(&d));
    let tree_b = storage::read_object(&rpath, &b)
        .and_then(|d| Tree::parse(&d));

    let mut map_a: BTreeMap<String, Hash> = BTreeMap::new();
    let mut map_b: BTreeMap<String, Hash> = BTreeMap::new();
    if let Some(t) = tree_a {
        for e in t.entries { map_a.insert(e.name, e.hash); }
    }
    if let Some(t) = tree_b {
        for e in t.entries { map_b.insert(e.name, e.hash); }
    }
    for (name, hb) in &map_b {
        match map_a.get(name) {
            None => out.push((name.clone(), DiffOp::Added)),
            Some(ha) if ha != hb => out.push((name.clone(), DiffOp::Modified)),
            _ => {}
        }
    }
    for name in map_a.keys() {
        if !map_b.contains_key(name) {
            out.push((name.clone(), DiffOp::Removed));
        }
    }
    out
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DiffOp { Added, Removed, Modified }
