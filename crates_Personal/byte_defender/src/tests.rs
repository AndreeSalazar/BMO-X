//! `defense::tests` — Tests integrados de ByteDefender.
//!
//! Cobertura:
//! - **inspect_bef_valid**: BEF correcto → Allow.
//! - **inspect_bef_malformed**: bytes basura → Reject.
//! - **inspect_bef_empty**: BEF vacío → Reject.
//! - **capability_set**: grant/revoke/has.
//! - **scanner_wx**: detectar sección W+X.
//! - **verifier_fnv1a**: hash determinístico.

#![allow(dead_code)]

use crate::{Verdict, Capability, CapabilitySet};
use crate::scanner;
use crate::verifier;

pub struct TestResult {
    pub name: &'static str,
    pub passed: bool,
    pub message: alloc::string::String,
}

pub fn run_all() -> alloc::vec::Vec<TestResult> {
    let mut r = alloc::vec::Vec::new();
    r.push(test_inspect_bef_valid());
    r.push(test_inspect_bef_malformed());
    r.push(test_inspect_bef_empty());
    r.push(test_capability_set());
    r.push(test_scanner_wx());
    r.push(test_verifier_fnv1a());
    r.push(test_inspect_bef_short());
    r
}

fn test_inspect_bef_valid() -> TestResult {
    // BEF mínimo: header 48 bytes con magic + counts.
    let mut bytes = [0u8; 64];
    bytes[0..4].copy_from_slice(b"BEF1");
    bytes[4] = 1; // version major
    bytes[5] = 0;
    bytes[12..14].copy_from_slice(&2u16.to_le_bytes()); // 2 sections
    bytes[16..18].copy_from_slice(&0u16.to_le_bytes()); // 0 imports
    let v = defense::inspect_bef("test.bef", &bytes);
    match v {
        Verdict::Allow => pass("inspect_bef_valid", "valid BEF allowed"),
        Verdict::Reject(s) => fail("inspect_bef_valid",
                                    &alloc::format!("rejected: {}", s)),
        Verdict::Quarantine(s) => fail("inspect_bef_valid",
                                       &alloc::format!("quarantined: {}", s)),
    }
}

fn test_inspect_bef_malformed() -> TestResult {
    let bytes = [0u8; 64]; // magic vacío
    let v = defense::inspect_bef("bad.bef", &bytes);
    match v {
        Verdict::Reject(_) => pass("inspect_bef_malformed", "empty magic rejected"),
        _ => fail("inspect_bef_malformed", "should reject"),
    }
}

fn test_inspect_bef_empty() -> TestResult {
    let bytes = [0u8; 0];
    let v = defense::inspect_bef("empty.bef", &bytes);
    match v {
        Verdict::Reject(_) => pass("inspect_bef_empty", "0 bytes rejected"),
        _ => fail("inspect_bef_empty", "should reject"),
    }
}

fn test_inspect_bef_short() -> TestResult {
    let bytes = [0u8; 10]; // menos de 48
    let v = defense::inspect_bef("short.bef", &bytes);
    match v {
        Verdict::Reject(_) => pass("inspect_bef_short", "10 bytes rejected"),
        _ => fail("inspect_bef_short", "should reject"),
    }
}

fn test_capability_set() -> TestResult {
    let mut s = CapabilitySet::empty();
    if s.has(Capability::FileAccess) { return fail("capability_set", "empty has FileAccess?"); }
    s.grant(Capability::FileAccess);
    s.grant(Capability::Network);
    if !s.has(Capability::FileAccess) || !s.has(Capability::Network) {
        return fail("capability_set", "granted not detected");
    }
    s.revoke(Capability::Network);
    if s.has(Capability::Network) {
        return fail("capability_set", "revoke failed");
    }
    pass("capability_set", "grant/revoke/has all work")
}

fn test_scanner_wx() -> TestResult {
    let mut bytes = [0u8; 256];
    bytes[0..4].copy_from_slice(b"BEF1");
    bytes[12..14].copy_from_slice(&2u16.to_le_bytes());
    // Section 0 (offset 48): flags W (0x02) + X (0x04) = 0x06
    bytes[48 + 8..48 + 12].copy_from_slice(&0x06u32.to_le_bytes());
    let r = scanner::scan("wx.bef", &bytes);
    if r.has_wx {
        pass("scanner_wx", "W+X section detected")
    } else {
        fail("scanner_wx", "W+X not detected")
    }
}

fn test_verifier_fnv1a() -> TestResult {
    let a = verifier::fnv1a_64(b"hello");
    let b = verifier::fnv1a_64(b"hello");
    let c = verifier::fnv1a_64(b"world");
    if a == b && a != c {
        pass("verifier_fnv1a", &alloc::format!("hash=0x{:x}", a))
    } else {
        fail("verifier_fnv1a", "hash not deterministic or not unique")
    }
}

// ── Helpers ───────────────────────────────────────────────────────

fn pass(name: &'static str, msg: &str) -> TestResult {
    TestResult { name, passed: true, message: alloc::string::String::from(msg) }
}
fn fail(name: &'static str, msg: &str) -> TestResult {
    TestResult { name, passed: false, message: alloc::string::String::from(msg) }
}
