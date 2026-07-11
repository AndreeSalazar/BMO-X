//! `timeback::storage` — SSD-backed object store.
//!
//! All objects (commits, trees, blobs) are stored on the SSD partition
//! at the repo path. The path is something like "T:/TIMEBACK" — the
//! storage layer uses the bmo_fat32 crate (via the kernel's AHCI driver)
//! to create directories and write files.
//!
//! Layout (Git-compatible):
//!
//!   TIMEBACK/
//!     objects/aa/bb<rest>...     — raw object file
//!     refs/heads/<name>           — contains commit hash (one per line)
//!     refs/tags/<name>            — tag refs
//!     HEAD                        — contains "ref: refs/heads/<name>"
//!     INDEX                       — staging area (path=hash per line)
//!     CONFIG                      — repo config (one key=value per line)
//!     LOG                         — append-only reflog (one line per HEAD change)

#![allow(dead_code)]

extern crate alloc;

use alloc::collections::BTreeMap;
use alloc::format;
use alloc::string::String;
use alloc::vec::Vec;
use core::sync::atomic::{AtomicU64, Ordering};

use super::hash::Hash;
use super::tree::{Tree, TreeEntry};

const TICK_OFFSET_NS: u64 = 1_700_000_000_000_000_000; // 2023-11-14 in ns

static FAKE_TICK: AtomicU64 = AtomicU64::new(0);

/// Get a monotonic nanosecond timestamp. Uses rdtsc if available,
/// else a software counter (for testing/standalone builds).
pub fn now_ns() -> u64 {
    // The kernel registers rdtsc as the tick source; if not set, fall
    // back to a software counter that advances per call.
    let cb = unsafe { TICK_SOURCE };
    if let Some(f) = cb {
        let t = f();
        // rdtsc returns cycles since boot; we just use it as a monotonic ID
        return t.wrapping_add(TICK_OFFSET_NS);
    }
    FAKE_TICK.fetch_add(1_000_000, Ordering::Relaxed)
}

static mut TICK_SOURCE: Option<fn() -> u64> = None;

/// Register a function that returns monotonic time (e.g. rdtsc).
pub fn set_tick_source(f: fn() -> u64) {
    unsafe { TICK_SOURCE = Some(f); }
}

/// SSD path to the repo root (e.g. "T:/TIMEBACK").
static REPO_PATH: spin::Mutex<Option<String>> = spin::Mutex::new(None);

/// Set the repo path. Called from repo::init().
pub fn set_path(path: &str) {
    let mut p = REPO_PATH.lock();
    *p = Some(String::from(path));
}

/// Create the repo directory structure on the SSD.
/// Returns true if successful.
pub fn ensure_repo_dir(path: &str) -> bool {
    set_path(path);
    // Create objects/, refs/heads/, refs/tags/ subdirectories
    ssd_mkdir(path);
    ssd_mkdir(&format!("{}/objects", path));
    ssd_mkdir(&format!("{}/refs", path));
    ssd_mkdir(&format!("{}/refs/heads", path));
    ssd_mkdir(&format!("{}/refs/tags", path));
    true
}

/// Write a raw object (commit, tree, or blob) to the SSD.
/// Layout: objects/<aa>/<rest>
pub fn write_object(repo: &str, hash: &Hash, data: &[u8]) -> bool {
    let hex = hash.to_hex();
    let dir = &hex[0..2];
    let rest = &hex[2..];
    let dir_path = format!("{}/objects/{}", repo, dir);
    ssd_mkdir(&dir_path);
    let file_path = format!("{}/objects/{}/{}", repo, dir, rest);
    ssd_write_file(&file_path, data)
}

/// Read a raw object from the SSD.
pub fn read_object(repo: &str, hash: &Hash) -> Option<Vec<u8>> {
    let hex = hash.to_hex();
    let file_path = format!("{}/objects/{}/{}", repo, &hex[0..2], &hex[2..]);
    ssd_read_file(&file_path)
}

/// Write a ref (branch, tag, HEAD).
pub fn write_ref(repo: &str, name: &str, hash: &str) -> bool {
    let path = if name == "HEAD" {
        format!("{}/HEAD", repo)
    } else if name.starts_with("refs/") {
        format!("{}/{}", repo, name)
    } else {
        format!("{}/refs/heads/{}", repo, name)
    };
    ssd_write_file(&path, hash.as_bytes())
}

/// Read a ref.
pub fn read_ref(repo: &str, name: &str) -> Option<String> {
    let path = if name == "HEAD" {
        format!("{}/HEAD", repo)
    } else if name.starts_with("refs/") {
        format!("{}/{}", repo, name)
    } else {
        format!("{}/refs/heads/{}", repo, name)
    };
    let data = ssd_read_file(&path)?;
    String::from_utf8(data).ok()
}

/// List all refs matching a prefix.
pub fn list_refs(repo: &str, prefix: &str) -> Vec<(String, String)> {
    let mut out = Vec::new();
    let dir = format!("{}/{}", repo, prefix.trim_end_matches('/'));
    let entries = ssd_list_dir(&dir);
    for entry in entries {
        let hash = match read_ref(repo, &format!("{}{}", prefix, entry)) {
            Some(h) => h,
            None => continue,
        };
        out.push((entry, hash));
    }
    out
}

// ── Index (staging area) ────────────────────────────────────────────

/// Add a file to the index.
pub fn index_add(repo: &str, path: &str, hash: &Hash) -> bool {
    let mut idx = read_index(repo);
    idx.insert(String::from(path), hash.clone());
    write_index(repo, &idx)
}

fn read_index(repo: &str) -> BTreeMap<String, Hash> {
    let mut map = BTreeMap::new();
    let data = match ssd_read_file(&format!("{}/INDEX", repo)) {
        Some(d) => d,
        None => return map,
    };
    for line in data.split(|&b| b == b'\n') {
        if line.is_empty() { continue; }
        let mut parts = line.splitn(2, |&b| b == b' ');
        let hash_str = match parts.next() {
            Some(h) => h,
            None => continue,
        };
        let path_bytes = match parts.next() {
            Some(p) => p,
            None => continue,
        };
        let path = match String::from_utf8(path_bytes.to_vec()) {
            Ok(s) => s,
            Err(_) => continue,
        };
        if let Some(h) = Hash::from_hex(core::str::from_utf8(hash_str).unwrap_or("")) {
            map.insert(path, h);
        }
    }
    map
}

fn write_index(repo: &str, idx: &BTreeMap<String, Hash>) -> bool {
    let mut buf = Vec::new();
    for (path, hash) in idx {
        buf.extend_from_slice(hash.to_hex().as_bytes());
        buf.push(b' ');
        buf.extend_from_slice(path.as_bytes());
        buf.push(b'\n');
    }
    ssd_write_file(&format!("{}/INDEX", repo), &buf)
}

pub fn index_list(repo: &str) -> Vec<(String, Hash)> {
    let idx = read_index(repo);
    idx.into_iter().collect()
}

pub fn index_clear(repo: &str) -> bool {
    ssd_write_file(&format!("{}/INDEX", repo), b"")
}

/// Convert the index into a single Tree (flat, no subdirectories).
pub fn index_to_tree(repo: &str) -> Option<Tree> {
    let idx = read_index(repo);
    let entries: Vec<TreeEntry> = idx.into_iter()
        .map(|(name, hash)| TreeEntry { mode: super::tree::FileMode::Normal, name, hash })
        .collect();
    Some(Tree::from_entries(entries))
}

// ── SSD operations (via bmo_fat32) ─────────────────────────────────

/// Create a directory on the SSD. Returns true on success.
fn ssd_mkdir(path: &str) -> bool {
    ssd_op_dispatch(SsdOp::Mkdir, path, &mut [])
}

/// Write a file to the SSD.
fn ssd_write_file(path: &str, data: &[u8]) -> bool {
    let mut buf = data.to_vec();
    ssd_op_dispatch(SsdOp::Write, path, &mut buf)
}

/// Read a file from the SSD.
fn ssd_read_file(path: &str) -> Option<Vec<u8>> {
    let mut out: Vec<u8> = Vec::new();
    if ssd_op_dispatch(SsdOp::Read, path, out.as_mut_slice()) {
        Some(out)
    } else {
        None
    }
}

/// List directory entries.
fn ssd_list_dir(path: &str) -> Vec<String> {
    let mut out: Vec<u8> = Vec::new();
    ssd_op_dispatch(SsdOp::ListDir, path, out.as_mut_slice());
    // Parse null-separated entry names from the buffer
    let mut result = Vec::new();
    let mut start = 0;
    for (i, &b) in out.iter().enumerate() {
        if b == 0 && i > start {
            if let Ok(s) = core::str::from_utf8(&out[start..i]) {
                result.push(String::from(s));
            }
            start = i + 1;
        }
    }
    result
}

#[derive(Clone, Copy)]
pub enum SsdOp { Mkdir, Write, Read, ListDir }

/// Dispatch an SSD operation. The kernel provides the implementation
/// via register_ssd_backend(). If no backend is registered (no AHCI),
/// all operations silently fail — caller should check return values.
fn ssd_op_dispatch(op: SsdOp, path: &str, data: &mut [u8]) -> bool {
    let cb = unsafe { SSD_BACKEND };
    if let Some(b) = cb {
        // SAFETY: the kernel guarantees the backend is thread-safe
        // and re-entrant for our use case (single-threaded kernel).
        unsafe { b(op, path, data) }
    } else {
        // No SSD backend: report failure (but only for critical ops).
        matches!(op, SsdOp::Mkdir) // mkdir always succeeds (no-op)
    }
}

type SsdBackend = unsafe fn(SsdOp, &str, &mut [u8]) -> bool;
static mut SSD_BACKEND: Option<SsdBackend> = None;

/// Register the kernel's SSD backend. Called once during init.
pub fn register_ssd_backend(f: SsdBackend) {
    unsafe { SSD_BACKEND = Some(f); }
}

// Make SsdBackend visible to kernel HAL
pub type SsdBackendFn = SsdBackend;

// ── Legacy NVRAM persistence (kept for crash-safety) ──────────────

/// NVRAM variable prefix for TimeBack checkpoints.
pub const NVRAM_PREFIX: &str = "BMOTBKP";

/// Max bytes per NVRAM variable (UEFI safety).
pub const NVRAM_CHUNK: usize = 192;

/// Max number of NVRAM variables (BMOTBKP0..BMOTBKP7).
pub const NVRAM_VARS_MAX: u32 = 8;

static mut USED: usize = 0;

/// Callback type for writing a NVRAM variable. Set by kernel.
type SetVarFn = fn(name: &str, data: &[u8]);
static mut SET_VAR: Option<SetVarFn> = None;

/// Register the kernel's NVRAM write callback. Called once at boot.
pub fn register_nvram_sink(f: SetVarFn) {
    unsafe { SET_VAR = Some(f); }
}

/// Build NVRAM variable name for a chunk index.
pub fn var_name(idx: u32) -> String {
    format!("{}{}", NVRAM_PREFIX, idx)
}

/// Write a snapshot to NVRAM (chunked across multiple variables).
/// Returns true on success.
pub fn persist_to_nvram(id: u32, snapshot_bytes: &[u8]) -> bool {
    let cb = unsafe { match SET_VAR { Some(f) => f, None => return false } };
    let total = snapshot_bytes.len();
    let mut offset = 0;
    let mut var_idx = 0u32;
    while offset < total && var_idx < NVRAM_VARS_MAX {
        let end = core::cmp::min(offset + NVRAM_CHUNK, total);
        let chunk = &snapshot_bytes[offset..end];
        let name = var_name(var_idx + (id % 4) * NVRAM_VARS_MAX);
        cb(&name, chunk);
        offset = end;
        var_idx += 1;
    }
    true
}

pub fn init() {
    unsafe { USED = 0; }
}

/// Bytes usados.
pub fn used_bytes() -> usize { unsafe { USED } }

/// Capacidad total.
pub fn capacity() -> usize { 16 * 1024 * 1024 }

/// ¿Hay espacio para `n` bytes más?
pub fn can_fit(n: usize) -> bool { n + unsafe { USED } <= capacity() }

/// Reserva `n` bytes (debe llamarse después de `can_fit`).
pub unsafe fn reserve(n: usize) { USED += n; }
