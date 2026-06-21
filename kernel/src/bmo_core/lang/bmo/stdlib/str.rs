//! BMO std::str — Operaciones con strings.

#![allow(dead_code)]

pub fn len(s: &str) -> usize { s.len() }
pub fn eq(a: &str, b: &str) -> bool { a == b }
pub fn concat(a: &str, b: &str) -> alloc::string::String {
    let mut r = alloc::string::String::with_capacity(a.len() + b.len()); r.push_str(a); r.push_str(b); r
}
pub fn contains(haystack: &str, needle: &str) -> bool { haystack.contains(needle) }
pub fn from_byte(b: u8) -> alloc::string::String { alloc::string::String::from(b as char) }
pub fn parse_u64(s: &str) -> Option<u64> {
    let mut r: u64 = 0;
    for &b in s.as_bytes() { if b >= b'0' && b <= b'9' { r = r.checked_mul(10)?.checked_add((b - b'0') as u64)?; } else { return None; } }
    Some(r)
}
