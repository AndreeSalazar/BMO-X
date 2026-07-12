#![allow(dead_code)]

use alloc::vec::Vec;

const MAX_SYMBOLS: usize = 4096;

#[derive(Debug, Clone, Copy)]
pub struct SymbolEntry {
    pub lib: &'static str,
    pub name: &'static str,
    pub addr: u64,
    pub hash: u32,
    pub flags: u32,
}

pub const SYM_EXPORT: u32 = 1 << 0;
pub const SYM_THUNK: u32 = 1 << 1;

/// Global symbol registry.
pub struct Registry;

static mut SYMBOLS: Vec<SymbolEntry> = Vec::new();
static mut LOCK: bool = false;

fn lock() {
    unsafe {
        while LOCK {
            core::hint::spin_loop();
        }
        LOCK = true;
        core::sync::atomic::fence(core::sync::atomic::Ordering::Acquire);
    }
}

fn unlock() {
    unsafe {
        core::sync::atomic::fence(core::sync::atomic::Ordering::Release);
        LOCK = false;
    }
}

fn to_static(s: &str) -> &'static str {
    if s.is_empty() { return ""; }
    let layout = core::alloc::Layout::from_size_align(s.len(), 1).unwrap();
    let ptr = unsafe { alloc::alloc::alloc(layout) };
    if ptr.is_null() { return ""; }
    unsafe {
        core::ptr::copy_nonoverlapping(s.as_ptr(), ptr, s.len());
        core::str::from_utf8_unchecked(core::slice::from_raw_parts(ptr, s.len()))
    }
}

impl Registry {
    pub fn insert(lib: &str, name: &str, addr: u64) {
        let hash = fnv1a_32(name.as_bytes());
        lock();
        unsafe {
            for entry in SYMBOLS.iter_mut() {
                if entry.hash == hash && entry.name == name {
                    entry.addr = addr;
                    unlock();
                    return;
                }
            }
            if SYMBOLS.len() < MAX_SYMBOLS {
                SYMBOLS.push(SymbolEntry {
                    lib: to_static(lib),
                    name: to_static(name),
                    addr,
                    hash,
                    flags: SYM_EXPORT,
                });
            }
        }
        unlock();
    }

    pub fn lookup(lib: &str, name: &str) -> u64 {
        let hash = fnv1a_32(name.as_bytes());
        lock();
        let result = unsafe {
            let mut found = 0u64;
            for entry in SYMBOLS.iter() {
                if entry.hash == hash && entry.name == name {
                    let lib_ok = if lib.is_empty() || entry.lib.is_empty() {
                        true
                    } else {
                        eq_ci(entry.lib, lib)
                    };
                    if lib_ok {
                        found = entry.addr;
                        break;
                    }
                }
            }
            found
        };
        unlock();
        result
    }

    pub fn lookup_by_hash(hash: u32, name: &str) -> u64 {
        lock();
        let result = unsafe {
            let mut found = 0u64;
            for entry in SYMBOLS.iter() {
                if entry.hash == hash && entry.name == name {
                    found = entry.addr;
                    break;
                }
            }
            found
        };
        unlock();
        result
    }

    pub fn clear() {
        lock();
        unsafe { SYMBOLS.clear(); }
        unlock();
    }

    pub fn len() -> usize {
        lock();
        let len = unsafe { SYMBOLS.len() };
        unlock();
        len
    }
}

fn fnv1a_32(bytes: &[u8]) -> u32 {
    let mut h: u32 = 0x811c_9dc5;
    for &b in bytes {
        h ^= b as u32;
        h = h.wrapping_mul(0x0100_0193);
    }
    h
}

fn eq_ci(a: &str, b: &str) -> bool {
    if a.len() != b.len() { return false; }
    a.bytes().zip(b.bytes()).all(|(x, y)| x.eq_ignore_ascii_case(&y))
}
