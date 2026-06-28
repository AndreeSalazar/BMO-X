use alloc::vec::Vec;
use alloc::vec;
use alloc::format;
use crate::bmo_abi::bef::{
    header::*,
    sections::*,
    signing::{SectionHash, SignatureHeader},
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IssueSeverity { Error, Warning, Info }

#[derive(Debug, Clone)]
pub struct ValidationIssue {
    pub severity: IssueSeverity,
    pub message: alloc::string::String,
}

#[derive(Debug)]
pub struct ValidationResult {
    pub issues: Vec<ValidationIssue>,
    pub is_valid: bool,
}

impl ValidationResult {
    pub fn new() -> Self { Self { issues: Vec::new(), is_valid: true } }

    pub fn error(&mut self, msg: impl Into<alloc::string::String>) {
        self.is_valid = false;
        self.issues.push(ValidationIssue { severity: IssueSeverity::Error, message: msg.into() });
    }

    pub fn warn(&mut self, msg: impl Into<alloc::string::String>) {
        self.issues.push(ValidationIssue { severity: IssueSeverity::Warning, message: msg.into() });
    }
}

pub fn validate(bytes: &[u8]) -> ValidationResult {
    let mut r = ValidationResult::new();

    if bytes.len() < BefHeader::SIZE {
        r.error("file too small for header");
        return r;
    }

    let header = unsafe { &*(bytes.as_ptr() as *const BefHeader) };

    if header.magic != BEF_MAGIC {
        r.error(format!("bad magic: expected {:#x}, got {:#x}", BEF_MAGIC, header.magic));
        return r;
    }

    if header.version_major != BEF_VERSION_MAJOR {
        r.error(format!("unsupported major version: {}", header.version_major));
        return r;
    }

    match header.arch {
        0x01 => {}
        0x02 => r.warn("AArch64 BEF not supported on x86-64 host"),
        0x03 => r.warn("RISC-V BEF not supported on x86-64 host"),
        _ => r.warn(format!("unknown arch: {:#x}", header.arch)),
    }

    if header.section_count == 0 { r.error("no sections"); return r; }
    if header.section_count > 255 { r.error(format!("too many sections: {}", header.section_count)); return r; }

    if header.total_size as usize != bytes.len() {
        r.warn(format!("total_size mismatch: header says {}, actual {}", header.total_size, bytes.len()));
    }

    let table_offset = header.section_table_offset as usize;
    let table_size = header.section_count as usize * SectionEntry::SIZE;
    if table_offset + table_size > bytes.len() {
        r.error("section table out of bounds");
        return r;
    }

    let table_ptr = &bytes[table_offset..table_offset + table_size];
    let entries = unsafe {
        core::slice::from_raw_parts(table_ptr.as_ptr() as *const SectionEntry, header.section_count as usize)
    };

    let mut seen = vec![false; 256];
    let mut has_code = false;

    for (i, entry) in entries.iter().enumerate() {
        let kind = entry.kind as usize;

        if entry.kind == SectionKind::Bss as u8 {
            if entry.mem_size == 0 { r.warn(format!("section[{}]: BSS with zero mem_size", i)); }
            seen[kind] = true;
            has_code |= entry.kind == SectionKind::Code as u8;
            continue;
        }

        let file_start = entry.file_offset as usize;
        let file_end = file_start + entry.file_size as usize;
        if file_start > bytes.len() || file_end > bytes.len() {
            r.error(format!("section[{}]: file range out of bounds [{}, {}]", i, file_start, file_end));
        }

        if entry.file_size == 0 && entry.mem_size > 0 && entry.kind != SectionKind::Bss as u8 {
            r.warn(format!("section[{}]: zero file_size but non-zero mem_size", i));
        }

        if entry.alignment > 0 && (entry.alignment & (entry.alignment - 1)) != 0 {
            r.warn(format!("section[{}]: alignment {} not power of 2", i, entry.alignment));
        }

        if seen[kind] { r.warn(format!("section[{}]: duplicate section kind {:#x}", i, entry.kind)); }
        seen[kind] = true;
        has_code |= entry.kind == SectionKind::Code as u8;
    }

    if !has_code { r.error("no Code section found"); }

    if let Some(sig) = entries.iter().find(|e| e.kind == SectionKind::Signature as u8) {
        validate_signature(bytes, sig, &mut r);
    }

    let has_manifest = entries.iter().any(|e| e.kind == SectionKind::Manifest as u8);
    if !has_manifest { r.warn("no Manifest section — capabilities unknown"); }

    r
}

fn validate_signature(bytes: &[u8], entry: &SectionEntry, r: &mut ValidationResult) {
    let start = entry.file_offset as usize;
    let end = start + entry.file_size as usize;
    if end > bytes.len() { r.error("Signature section out of bounds"); return; }
    let sig_bytes = &bytes[start..end];
    if sig_bytes.len() < core::mem::size_of::<SignatureHeader>() {
        r.error("Signature section too small for header");
        return;
    }
    let sig_header = unsafe { &*(sig_bytes.as_ptr() as *const SignatureHeader) };
    let hash_count = sig_header.hash_count as usize;
    let hashes_size = hash_count * core::mem::size_of::<SectionHash>();
    if sig_bytes.len() < core::mem::size_of::<SignatureHeader>() + hashes_size {
        r.error("Signature section too small for hashes");
        return;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::bmo_abi::bef::writer::{BefBuilder, BefSection};

    #[test]
    fn validate_valid() {
        let mut b = BefBuilder::new();
        b.add_section(BefSection::code(vec![0xC3; 16]));
        b.add_section(BefSection::rodata(b"test".to_vec()));
        let bytes = b.build().unwrap();
        let r = validate(&bytes);
        assert!(r.is_valid, "expected valid: {:?}", r.issues);
    }

    #[test]
    fn validate_bad_magic() {
        let r = validate(&[0; 48]);
        assert!(!r.is_valid);
    }

    #[test]
    fn validate_too_small() {
        let r = validate(&[0; 10]);
        assert!(!r.is_valid);
    }
}
