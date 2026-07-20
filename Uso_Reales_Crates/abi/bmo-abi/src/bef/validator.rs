//! BEF validator estructural — verifica integridad y coherencia
//! del formato antes de que el loader lo procese.
//!
//! Cubre:
//! - Header (magic, version, arch, entry, total_size)
//! - Section table (bounds, duplicados, overlap)
//! - Code section (entry offset, alineación)
//! - Imports / Exports / Symbols (parseo, string bounds, binding targets)
//! - Relocations (offset dentro de target, symbol_idx válido)
//! - Signature (hashing structure)
//! - Flags semánticos consistentes por tipo de sección

use crate::bmo_abi::bef::{
    exports::ExportEntry,
    header::*,
    imports::ImportEntry,
    relocations::{Relocation, RelocationKind},
    sections::*,
    signing::{SectionHash, SignatureHeader},
    symbols::Symbol,
};
use alloc::format;
use alloc::string::String;
#[allow(unused_imports)]
use alloc::vec;
use alloc::vec::Vec;

/// Resultado individual de validación.
#[derive(Debug, Clone)]
pub struct ValidationIssue {
    pub severity: IssueSeverity,
    pub section: Option<usize>,
    pub message: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IssueSeverity {
    Error,
    Warning,
    Info,
}

/// Resultado completo de una validación.
#[derive(Debug)]
pub struct ValidationResult {
    pub issues: Vec<ValidationIssue>,
    pub is_valid: bool,
    pub is_loaded: bool,
}

impl ValidationResult {
    pub fn new() -> Self {
        Self {
            issues: Vec::new(),
            is_valid: true,
            is_loaded: false,
        }
    }

    pub fn error(&mut self, msg: impl Into<String>) {
        self.is_valid = false;
        self.issues.push(ValidationIssue {
            severity: IssueSeverity::Error,
            section: None,
            message: msg.into(),
        });
    }

    pub fn error_at(&mut self, section: usize, msg: impl Into<String>) {
        self.is_valid = false;
        self.issues.push(ValidationIssue {
            severity: IssueSeverity::Error,
            section: Some(section),
            message: msg.into(),
        });
    }

    pub fn warn(&mut self, msg: impl Into<String>) {
        self.issues.push(ValidationIssue {
            severity: IssueSeverity::Warning,
            section: None,
            message: msg.into(),
        });
    }

    pub fn warn_at(&mut self, section: usize, msg: impl Into<String>) {
        self.issues.push(ValidationIssue {
            severity: IssueSeverity::Warning,
            section: Some(section),
            message: msg.into(),
        });
    }

    pub fn info(&mut self, msg: impl Into<String>) {
        self.issues.push(ValidationIssue {
            severity: IssueSeverity::Info,
            section: None,
            message: msg.into(),
        });
    }
}

/// Valida un buffer BEF completo. Retorna `ValidationResult` con todos
/// los issues encontrados (no corta en el primer error).
pub fn validate(bytes: &[u8]) -> ValidationResult {
    let mut r = ValidationResult::new();

    if bytes.len() < BefHeader::SIZE {
        r.error(format!(
            "file too small: {} bytes, need at least {} for header",
            bytes.len(),
            BefHeader::SIZE
        ));
        return r;
    }

    // Safe: read header via unaligned access (buffer may not be 16-byte aligned)
    let header: BefHeader =
        unsafe { core::ptr::read_unaligned(bytes.as_ptr() as *const BefHeader) };
    validate_header(&header, bytes, &mut r);

    if !r.is_valid
        && r.issues
            .iter()
            .any(|i| matches!(i.severity, IssueSeverity::Error) && i.message.contains("magic"))
    {
        return r;
    }

    let table_offset = header.section_table_offset as usize;
    let table_size = header.section_count as usize * SectionEntry::SIZE;
    if table_offset + table_size > bytes.len() {
        r.error(format!(
            "section table out of bounds: offset {} size {} > file len {}",
            table_offset,
            table_size,
            bytes.len()
        ));
        return r;
    }

    // Read section entries via unaligned copies
    let mut entries: Vec<SectionEntry> = Vec::with_capacity(header.section_count as usize);
    let entry_raw = &bytes[table_offset..table_offset + table_size];
    for i in 0..header.section_count as usize {
        let off = i * SectionEntry::SIZE;
        let e: SectionEntry =
            unsafe { core::ptr::read_unaligned(entry_raw[off..].as_ptr() as *const SectionEntry) };
        entries.push(e);
    }

    validate_section_table(&entries, bytes, &mut r);
    if !r.is_valid {
        return r;
    }

    let code_idx = entries
        .iter()
        .position(|e| e.kind == SectionKind::Code as u8);
    if let Some(ci) = code_idx {
        validate_code_section(&entries[ci], &header, &mut r);
    }

    for (i, entry) in entries.iter().enumerate() {
        let kind = match entry.kind() {
            Some(k) => k,
            None => {
                r.warn_at(i, format!("unknown section kind {:#04x}", entry.kind));
                continue;
            }
        };
        match kind {
            SectionKind::Code => {}
            SectionKind::Imports => validate_import_section(entry, bytes, i, &mut r),
            SectionKind::Exports => validate_export_section(entry, bytes, i, &mut r),
            SectionKind::Symbols => validate_symbol_section(entry, bytes, &entries, i, &mut r),
            SectionKind::Relocs => validate_reloc_section(entry, bytes, &entries, i, &mut r),
            SectionKind::Bss => validate_bss_section(entry, i, &mut r),
            SectionKind::Signature => validate_signature(entry, bytes, &mut r),
            SectionKind::Manifest => validate_manifest_section(entry, bytes, i, &mut r),
            SectionKind::Tls => validate_tls_section(entry, bytes, i, &mut r),
            SectionKind::Unwind | SectionKind::Debug => {
                validate_optional_section(entry, bytes, i, &mut r)
            }
            _ => {}
        }
        validate_section_flags(entry, kind, i, &mut r);
    }

    r.is_loaded = r.is_valid;
    r
}

fn validate_header(header: &BefHeader, bytes: &[u8], r: &mut ValidationResult) {
    if header.magic != BEF_MAGIC {
        let actual = u32::from_le_bytes(bytes[..4].try_into().unwrap_or([0; 4]));
        r.error(format!(
            "bad magic: expected BEF1 ({:#x}), got {:#x}",
            BEF_MAGIC, actual
        ));
        return;
    }
    if header.version_major != BEF_VERSION_MAJOR {
        r.error(format!(
            "unsupported major version: {} (expected {})",
            header.version_major, BEF_VERSION_MAJOR
        ));
    }
    if header.version_minor > BEF_VERSION_MINOR {
        r.warn(format!(
            "newer minor version: {} (loader supports {})",
            header.version_minor, BEF_VERSION_MINOR
        ));
    }
    match header.arch {
        0x01 => {}
        0x00 => r.warn("arch is Reserved (0x00) — assuming x86-64"),
        0x02 => r.warn("AArch64 BEF — not supported on this host"),
        0x03 => r.warn("RISC-V BEF — not supported on this host"),
        _ => r.warn(format!("unknown arch: {:#04x}", header.arch)),
    }
    if header.section_count == 0 {
        r.error("no sections declared");
    }
    if header.section_count > 255 {
        r.error(format!(
            "too many sections: {} (max 255)",
            header.section_count
        ));
    }
    if header.total_size as usize != bytes.len() {
        r.warn(format!(
            "total_size mismatch: header says {}, actual file is {}",
            header.total_size,
            bytes.len()
        ));
    }
    if header.section_table_offset as usize != BefHeader::SIZE {
        r.info(format!(
            "section_table_offset is {:#x}, typical is {:#x}",
            header.section_table_offset,
            BefHeader::SIZE
        ));
    }
    if header.entry_offset == 0 && header.section_count > 0 {
        r.info("entry_offset is 0 — loader will use start of Code section");
    }
    let flags = BefFlags::from_bits_truncate(header.flags);
    if !flags.contains(BefFlags::EXECUTABLE) && !flags.contains(BefFlags::SHARED_LIBRARY) {
        r.warn("neither EXECUTABLE nor SHARED_LIBRARY flag set");
    }
    if header.flags & !(BefFlags::all().bits()) != 0 {
        r.warn(format!(
            "unknown header flags: {:#x}",
            header.flags & !BefFlags::all().bits()
        ));
    }
}

fn validate_section_table(entries: &[SectionEntry], bytes: &[u8], r: &mut ValidationResult) {
    let mut seen = [false; 256u16 as usize];
    let mut has_code = false;

    for (i, entry) in entries.iter().enumerate() {
        let kind = entry.kind as usize;

        if kind >= seen.len() {
            r.error_at(
                i,
                format!("section kind {:#04x} out of valid range", entry.kind),
            );
            continue;
        }

        if entry.kind == SectionKind::Bss as u8 {
            if entry.mem_size == 0 {
                r.warn_at(i, "BSS section with zero mem_size");
            }
            if entry.file_size > 0 {
                r.warn_at(
                    i,
                    "BSS section should have file_size = 0 (data is zero-filled at load)",
                );
            }
            if entry.file_offset > 0 && entry.file_size > 0 {
                let start = entry.file_offset as usize;
                let end = start + entry.file_size as usize;
                if end <= bytes.len() {
                    let slice = &bytes[start..end];
                    if slice.iter().any(|&b| b != 0) {
                        r.warn_at(i, "BSS section has non-zero data in file — will be ignored");
                    }
                }
            }
            seen[kind] = true;
            has_code |= entry.kind == SectionKind::Code as u8;
            continue;
        }

        let file_start = entry.file_offset as usize;
        let file_end = file_start + entry.file_size as usize;
        if entry.file_size > 0 {
            if file_start >= bytes.len() {
                r.error_at(
                    i,
                    format!("section file offset {:#x} beyond file end", file_start),
                );
                continue;
            }
            if file_end > bytes.len() {
                r.error_at(
                    i,
                    format!(
                        "section file range out of bounds: [{:#x}, {:#x}) > file len {}",
                        file_start,
                        file_end,
                        bytes.len()
                    ),
                );
                continue;
            }
        }

        if entry.file_size == 0 && entry.mem_size > 0 && entry.kind != SectionKind::Bss as u8 {
            r.warn_at(
                i,
                format!(
                    "zero file_size but non-zero mem_size ({}) — zero-filled at load",
                    entry.mem_size
                ),
            );
        }

        if entry.alignment > 0 && (entry.alignment & (entry.alignment - 1)) != 0 {
            r.warn_at(
                i,
                format!("alignment {} is not a power of 2", entry.alignment),
            );
        }

        if seen[kind] {
            r.warn_at(i, format!("duplicate section kind {:#04x}", entry.kind));
        }
        seen[kind] = true;
        has_code |= entry.kind == SectionKind::Code as u8;
    }

    if !has_code {
        r.error("no Code section found — BEF must have at least one executable section");
    }

    // Overlap detection: O(n²) but n ≤ 255, cheap.
    for i in 0..entries.len() {
        let a = &entries[i];
        if a.kind == SectionKind::Bss as u8 || a.file_size == 0 {
            continue;
        }
        let a_start = a.file_offset as usize;
        let a_end = a_start + a.file_size as usize;
        for j in (i + 1)..entries.len() {
            let b = &entries[j];
            if b.kind == SectionKind::Bss as u8 || b.file_size == 0 {
                continue;
            }
            let b_start = b.file_offset as usize;
            let b_end = b_start + b.file_size as usize;
            if a_start < b_end && b_start < a_end {
                r.error(format!(
                    "section overlap: section[{}] [{:#x}, {:#x}) overlaps section[{}] [{:#x}, {:#x})",
                    i, a_start, a_end, j, b_start, b_end
                ));
            }
        }
    }
}

fn validate_code_section(entry: &SectionEntry, header: &BefHeader, r: &mut ValidationResult) {
    if entry.file_size == 0 && entry.mem_size == 0 {
        r.error("Code section is empty (file_size=0, mem_size=0)");
    }
    let entry_off = header.entry_offset as usize;
    if entry_off >= entry.mem_size as usize && entry.mem_size > 0 {
        r.error(format!(
            "entry_offset {:#x} is beyond Code section mem_size {}",
            entry_off, entry.mem_size
        ));
    }
    if entry.flags & SectionFlags::EXEC.bits() == 0 {
        r.warn("Code section missing EXEC flag");
    }
    if entry.flags & SectionFlags::READ.bits() == 0 {
        r.warn("Code section missing READ flag (execution without read is unusual)");
    }
}

fn validate_import_section(
    entry: &SectionEntry,
    bytes: &[u8],
    idx: usize,
    r: &mut ValidationResult,
) {
    let data = match get_section_data(entry, bytes) {
        Some(d) => d,
        None => {
            r.error_at(idx, "Imports section data unavailable");
            return;
        }
    };
    let entry_sz = core::mem::size_of::<ImportEntry>();
    if data.len() < entry_sz {
        r.error_at(
            idx,
            format!(
                "Imports section too small: {} bytes, need at least {}",
                data.len(),
                entry_sz
            ),
        );
        return;
    }
    let raw = data.as_ptr() as *const ImportEntry;
    let count = data.len() / entry_sz;
    let string_start = count * entry_sz;
    let strings = &data[string_start..];

    for (i, imp) in (0..count).map(|i| unsafe { &*raw.add(i) }).enumerate() {
        let lib_off = imp.library_name_off as usize;
        let sym_off = imp.symbol_name_off as usize;
        if string_start + lib_off + 2 > data.len() {
            r.error_at(
                idx,
                format!(
                    "import[{}]: library_name_off {:#x} out of strings range",
                    i, lib_off
                ),
            );
        } else {
            let slen =
                u16::from_le_bytes(strings[lib_off..lib_off + 2].try_into().unwrap_or([0; 2]))
                    as usize;
            if string_start + lib_off + 2 + slen > data.len() {
                r.error_at(
                    idx,
                    format!(
                        "import[{}]: library name length {} exceeds strings",
                        i, slen
                    ),
                );
            }
        }
        if string_start + sym_off + 2 > data.len() {
            r.error_at(
                idx,
                format!(
                    "import[{}]: symbol_name_off {:#x} out of strings range",
                    i, sym_off
                ),
            );
        }
        if imp.binding_offset > 0 && imp.binding_offset < u64::MAX {
            // binding_offset should point to valid writable memory
            // (code or data section). We can't fully validate without section
            // addresses, but we can check it's not absurd.
            if imp.binding_offset > bytes.len() as u64 {
                r.warn_at(
                    idx,
                    format!(
                        "import[{}]: binding_offset {:#x} is beyond file size",
                        i, imp.binding_offset
                    ),
                );
            }
        }
    }
}

fn validate_export_section(
    entry: &SectionEntry,
    bytes: &[u8],
    idx: usize,
    r: &mut ValidationResult,
) {
    let data = match get_section_data(entry, bytes) {
        Some(d) => d,
        None => {
            r.error_at(idx, "Exports section data unavailable");
            return;
        }
    };
    let entry_sz = core::mem::size_of::<ExportEntry>();
    if data.len() < entry_sz {
        r.error_at(
            idx,
            format!("Exports section too small: {} bytes", data.len()),
        );
        return;
    }
    let raw = data.as_ptr() as *const ExportEntry;
    let count = data.len() / entry_sz;
    let string_start = count * entry_sz;

    for i in 0..count {
        let e = unsafe { &*raw.add(i) };
        let off = e.symbol_name_off as usize;
        let abs_off = string_start + off;
        if abs_off + 2 > data.len() {
            r.error_at(
                idx,
                format!(
                    "export[{}]: symbol_name_off {:#x} out of strings range",
                    i, off
                ),
            );
        } else {
            let slen = u16::from_le_bytes(data[abs_off..abs_off + 2].try_into().unwrap_or([0; 2]))
                as usize;
            if abs_off + 2 + slen > data.len() {
                r.error_at(
                    idx,
                    format!("export[{}]: symbol name length {} exceeds strings", i, slen),
                );
            }
        }
    }
}

fn validate_symbol_section(
    entry: &SectionEntry,
    bytes: &[u8],
    sections: &[SectionEntry],
    idx: usize,
    r: &mut ValidationResult,
) {
    let data = match get_section_data(entry, bytes) {
        Some(d) => d,
        None => {
            r.error_at(idx, "Symbols section data unavailable");
            return;
        }
    };
    let entry_sz = Symbol::SIZE;
    if data.len() < entry_sz {
        r.error_at(
            idx,
            format!("Symbols section too small: {} bytes", data.len()),
        );
        return;
    }
    let raw = data.as_ptr() as *const Symbol;
    let count = data.len() / entry_sz;
    let string_start = count * entry_sz;

    for i in 0..count {
        let sym = unsafe { &*raw.add(i) };
        let off = sym.name_off as usize;
        let abs_off = string_start + off;
        if abs_off + 2 > data.len() {
            r.error_at(
                idx,
                format!("symbol[{}]: name_off {:#x} out of strings range", i, off),
            );
        }
        if sym.kind() == None {
            r.warn_at(
                idx,
                format!("symbol[{}]: unknown symbol kind {:#04x}", i, sym.kind),
            );
        }
        if sym.binding() == None {
            r.warn_at(
                idx,
                format!("symbol[{}]: unknown binding {:#04x}", i, sym.binding),
            );
        }
        if !matches!(sym.visibility, 0x00..=0x03) {
            r.warn_at(
                idx,
                format!("symbol[{}]: unknown visibility {:#04x}", i, sym.visibility),
            );
        }
        match sym.section_idx {
            0xFF => {} // ABS
            0xFE => {} // COMMON
            si => {
                let si_u = si as usize;
                if si_u >= sections.len() {
                    r.error_at(
                        idx,
                        format!(
                            "symbol[{}]: section_idx {} out of range (sections count {})",
                            i,
                            si,
                            sections.len()
                        ),
                    );
                } else if sym.size > 0 && sections[si_u].mem_size > 0 {
                    let end = sym.virt_addr + sym.size;
                    if end > sections[si_u].mem_size {
                        r.warn_at(
                            idx,
                            format!(
                            "symbol[{}]: virt_addr {:#x} + size {} exceeds section[{}] mem_size {}",
                            i, sym.virt_addr, sym.size, si_u, sections[si_u].mem_size
                        ),
                        );
                    }
                }
            }
        }
    }
}

fn validate_reloc_section(
    entry: &SectionEntry,
    bytes: &[u8],
    sections: &[SectionEntry],
    idx: usize,
    r: &mut ValidationResult,
) {
    let data = match get_section_data(entry, bytes) {
        Some(d) => d,
        None => {
            r.error_at(idx, "Relocs section data unavailable");
            return;
        }
    };
    let entry_sz = Relocation::SIZE;
    if data.len() < entry_sz || data.len() % entry_sz != 0 {
        r.error_at(
            idx,
            format!(
                "Relocs section size {} is not a multiple of entry size {}",
                data.len(),
                entry_sz
            ),
        );
        return;
    }
    let raw = data.as_ptr() as *const Relocation;
    let count = data.len() / entry_sz;

    // Map target_section byte → section index lookup
    let target_map: [SectionKind; 3] = [SectionKind::Code, SectionKind::Data, SectionKind::RoData];

    for i in 0..count {
        let rel = unsafe { &*raw.add(i) };
        if rel.kind() == None {
            r.error_at(idx, format!("reloc[{}]: unknown kind {:#04x}", i, rel.kind));
        }
        // Validate target section
        let ts = rel.target_section as usize;
        if ts >= target_map.len() {
            r.error_at(
                idx,
                format!("reloc[{}]: target_section {} out of range (0..2)", i, ts),
            );
        } else {
            let target_kind = target_map[ts];
            let target_entry = sections.iter().find(|s| s.kind == target_kind as u8);
            if let Some(te) = target_entry {
                let patch_size: usize = match rel.kind() {
                    Some(RelocationKind::Abs64) => 8,
                    Some(_) => 4,
                    None => 4,
                };
                let end = rel.offset as usize + patch_size;
                if end > te.file_size as usize && end > te.mem_size as usize {
                    r.error_at(idx, format!(
                        "reloc[{}]: offset {:#x} + {} exceeds target section size (file {}, mem {})",
                        i, rel.offset, patch_size, te.file_size, te.mem_size
                    ));
                }
            } else {
                r.warn_at(
                    idx,
                    format!(
                        "reloc[{}]: target section '{:?}' (kind {:#04x}) not found",
                        i, target_kind, target_kind as u8
                    ),
                );
            }
        }
        // Validate symbol_idx range (0..Symbols section count or Imports count)
        let syms = sections
            .iter()
            .find(|s| s.kind == SectionKind::Symbols as u8);
        let imps = sections
            .iter()
            .find(|s| s.kind == SectionKind::Imports as u8);
        let has_syms = syms.map_or(false, |s| {
            s.file_size >= core::mem::size_of::<Symbol>() as u64
        });
        let has_imps = imps.map_or(false, |s| {
            s.file_size >= core::mem::size_of::<ImportEntry>() as u64
        });
        if rel.symbol_idx >= 1000 && !(has_syms || has_imps) {
            // Large symbol_idx with no Symbols/Imports section is suspicious
            r.warn_at(
                idx,
                format!(
                    "reloc[{}]: symbol_idx {} but no Symbols or Imports section found",
                    i, rel.symbol_idx
                ),
            );
        }
    }
}

fn validate_bss_section(entry: &SectionEntry, idx: usize, r: &mut ValidationResult) {
    if entry.mem_size == 0 {
        r.warn_at(idx, "BSS section with zero mem_size");
    }
    if entry.flags & SectionFlags::WRITE.bits() == 0 {
        r.warn_at(
            idx,
            "BSS section missing WRITE flag (cannot zero-fill without write permission)",
        );
    }
}

fn validate_signature(entry: &SectionEntry, bytes: &[u8], r: &mut ValidationResult) {
    let data = match get_section_data(entry, bytes) {
        Some(d) => d,
        None => {
            r.error("Signature section data unavailable");
            return;
        }
    };
    let hdr_sz = core::mem::size_of::<SignatureHeader>();
    let hash_sz = core::mem::size_of::<SectionHash>();
    if data.len() < hdr_sz {
        r.error(format!(
            "Signature section too small for header: {} < {}",
            data.len(),
            hdr_sz
        ));
        return;
    }
    let sig_header = unsafe { &*(data.as_ptr() as *const SignatureHeader) };
    let hash_count = sig_header.hash_count as usize;
    let needed = hdr_sz + hash_count * hash_sz;
    if data.len() < needed {
        r.error(format!(
            "Signature section too small: {} bytes, need {} for {} hashes",
            data.len(),
            needed,
            hash_count
        ));
    }
    if sig_header.sig_algo > 1 {
        r.warn(format!(
            "unknown signature algorithm: {}",
            sig_header.sig_algo
        ));
    }
}

fn validate_manifest_section(
    entry: &SectionEntry,
    bytes: &[u8],
    idx: usize,
    r: &mut ValidationResult,
) {
    let data = match get_section_data(entry, bytes) {
        Some(d) => d,
        None => {
            r.error_at(idx, "Manifest section data unavailable");
            return;
        }
    };
    if data.is_empty() {
        r.warn_at(idx, "Manifest section is empty");
        return;
    }
    if core::str::from_utf8(data).is_err() {
        r.warn_at(idx, "Manifest section is not valid UTF-8 (expected TOML)");
    }
}

fn validate_tls_section(entry: &SectionEntry, bytes: &[u8], idx: usize, r: &mut ValidationResult) {
    let data = match get_section_data(entry, bytes) {
        Some(d) => d,
        None => {
            r.error_at(idx, "TLS section data unavailable");
            return;
        }
    };
    if data.len() < 16 {
        r.warn_at(
            idx,
            format!(
                "TLS section very small: {} bytes (template header is 16)",
                data.len()
            ),
        );
    }
}

fn validate_optional_section(
    entry: &SectionEntry,
    bytes: &[u8],
    idx: usize,
    r: &mut ValidationResult,
) {
    let data = match get_section_data(entry, bytes) {
        Some(d) => d,
        None => {
            return;
        }
    };
    if data.is_empty() {
        r.info_at(idx, "section is empty (optional debug section)");
    }
}

fn validate_section_flags(
    entry: &SectionEntry,
    kind: SectionKind,
    idx: usize,
    r: &mut ValidationResult,
) {
    let flags = SectionFlags::from_bits_truncate(entry.flags);
    match kind {
        SectionKind::Code => {
            if !flags.contains(SectionFlags::EXEC) {
                r.error_at(idx, "Code section must have EXEC flag");
            }
        }
        SectionKind::Data => {
            if !flags.contains(SectionFlags::WRITE) {
                r.warn_at(idx, "Data section should have WRITE flag");
            }
        }
        SectionKind::Imports | SectionKind::Exports | SectionKind::Symbols => {
            if !flags.contains(SectionFlags::READ) {
                r.warn_at(idx, "metadata section should have READ flag");
            }
        }
        _ => {}
    }
}

/// Obtiene los bytes de datos de una sección (no-BSS, no vacía).
fn get_section_data<'a>(entry: &SectionEntry, bytes: &'a [u8]) -> Option<&'a [u8]> {
    if entry.kind == SectionKind::Bss as u8 || entry.file_size == 0 {
        return None;
    }
    let start = entry.file_offset as usize;
    let end = start + entry.file_size as usize;
    if end > bytes.len() {
        return None;
    }
    Some(&bytes[start..end])
}

// ── Extra methods for ValidationResult ──

impl ValidationIssue {
    pub fn fmt(&self) -> String {
        let sev = match self.severity {
            IssueSeverity::Error => "ERROR",
            IssueSeverity::Warning => "WARN",
            IssueSeverity::Info => "INFO",
        };
        match self.section {
            Some(s) => format!("[{}] section[{}]: {}", sev, s, self.message),
            None => format!("[{}] {}", sev, self.message),
        }
    }
}

impl ValidationResult {
    fn info_at(&mut self, section: usize, msg: impl Into<String>) {
        self.issues.push(ValidationIssue {
            severity: IssueSeverity::Info,
            section: Some(section),
            message: msg.into(),
        });
    }
}

// ── Tests ──

#[cfg(test)]
mod tests {
    use super::*;
    use crate::bmo_abi::bef::writer::{BefBuilder, BefSection};

    #[test]
    fn validate_minimal_valid() {
        let mut b = BefBuilder::new();
        b.add_section(BefSection::code(vec![0xC3; 16]));
        b.add_section(BefSection::rodata(b"test\0".to_vec()));
        let bytes = b.build().unwrap();
        let r = validate(&bytes);
        assert!(r.is_valid, "expected valid: {:?}", r.issues);
    }

    #[test]
    fn validate_all_sections() {
        let mut b = BefBuilder::new();
        b.add_section(BefSection::code(vec![0xC3; 16]));
        b.add_section(BefSection::rodata(b"data".to_vec()));
        b.add_section(BefSection::data(vec![0x00; 32]));
        b.add_section(BefSection::bss(256));
        b.add_section(BefSection::manifest_toml(
            b"[identity]\nname = \"test\"\n".to_vec(),
        ));
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

    #[test]
    fn validate_entry_out_of_bounds() {
        let mut b = BefBuilder::new();
        b.add_section(BefSection::code(vec![0xC3; 16]));
        b.entry_offset = 999;
        let bytes = b.build().unwrap();
        let r = validate(&bytes);
        assert!(!r.is_valid, "entry_offset beyond section should fail");
    }

    #[test]
    fn validate_no_code() {
        let mut b = BefBuilder::new();
        b.add_section(BefSection::rodata(b"data".to_vec()));
        let bytes = b.build().unwrap();
        let r = validate(&bytes);
        assert!(!r.is_valid, "no Code section should fail");
    }

    #[test]
    fn validate_empty_code() {
        let mut b = BefBuilder::new();
        b.add_section(BefSection::code(vec![]));
        let bytes = b.build().unwrap();
        let r = validate(&bytes);
        assert!(!r.is_valid, "empty Code section should fail");
    }

    #[test]
    fn validate_duplicate_sections() {
        let mut b = BefBuilder::new();
        b.add_section(BefSection::code(vec![0xC3; 16]));
        b.add_section(BefSection::code(vec![0xCC; 16]));
        let bytes = b.build().unwrap();
        let r = validate(&bytes);
        assert!(
            r.is_valid,
            "duplicate Code is a warning, not error: {:?}",
            r.issues
        );
        let has_warn = r.issues.iter().any(|i| {
            matches!(i.severity, IssueSeverity::Warning) && i.message.contains("duplicate")
        });
        assert!(has_warn, "should warn about duplicate section");
    }

    #[test]
    fn validate_section_overlap() {
        let mut b = BefBuilder::new();
        b.add_section(BefSection::code(vec![0xC3; 128]));
        b.add_section(BefSection::rodata(b"data".to_vec()));
        let bytes = b.build().unwrap();

        // Patch section[1] file_offset to match section[0] file_offset (overlap)
        let mut corrupted = bytes.clone();
        let hdr: BefHeader =
            unsafe { core::ptr::read_unaligned(corrupted.as_ptr() as *const BefHeader) };
        let table_start = hdr.section_table_offset as usize;
        // Read section[0] file_offset
        let sec0_file_off = u64::from_le_bytes(
            corrupted[table_start + 8..table_start + 16]
                .try_into()
                .unwrap(),
        );
        // Write that same offset into section[1]'s file_offset field
        let entry1_file_off_pos = table_start + 48 + 8;
        let entry1_file_sz_pos = table_start + 48 + 16;
        corrupted[entry1_file_off_pos..entry1_file_off_pos + 8]
            .copy_from_slice(&sec0_file_off.to_le_bytes());
        corrupted[entry1_file_sz_pos..entry1_file_sz_pos + 8].copy_from_slice(&32u64.to_le_bytes());

        let r = validate(&corrupted);
        let has_overlap = r.issues.iter().any(|i| i.message.contains("overlap"));
        assert!(
            has_overlap,
            "should detect overlapping sections: {:?}",
            r.issues
        );
    }

    #[test]
    fn validate_section_flags_missing_exec() {
        let mut b = BefBuilder::new();
        let mut code = BefSection::code(vec![0xC3; 16]);
        code.flags = SectionFlags::READ; // remove EXEC
        b.add_section(code);
        let bytes = b.build().unwrap();
        let r = validate(&bytes);
        let has_flag_err = r.issues.iter().any(|i| i.message.contains("EXEC"));
        assert!(
            has_flag_err,
            "should flag missing EXEC on Code: {:?}",
            r.issues
        );
    }

    #[test]
    fn validate_manifest_utf8() {
        use crate::bmo_abi::bef::writer::BefBuilder;
        use crate::bmo_abi::bef::writer::BefSection;
        let mut b = BefBuilder::new();
        b.add_section(BefSection::code(vec![0xC3; 16]));
        b.add_section(BefSection::manifest_toml(b"\xFF\xFE\x00".to_vec())); // invalid UTF-8
        let bytes = b.build().unwrap();
        let r = validate(&bytes);
        let has_utf8_warn = r.issues.iter().any(|i| i.message.contains("UTF-8"));
        assert!(
            has_utf8_warn,
            "should warn about non-UTF-8 manifest: {:?}",
            r.issues
        );
    }
}
