//! **EL OBJETO (`.bo`)** -- a compiled unit that is not a program yet.
//!
//! E1 of `docs/plan/PLAN_EL_ENLAZADOR.md`, 2026-09-17. The owner decided that
//! BMO-X links STATICALLY: everything a `.bex` executes travels inside it, so
//! its signature covers all of it and it runs the same on any BMO-X. This file
//! is the CONTRACT between a frontend that writes objects and the host tool
//! that joins them (`bmo-enlazar`, E3) -- a format, not a brain (rule 2): no
//! IR, no shared optimizer. Each language keeps its own codegen and only has to
//! write this shape.
//!
//! ## The shape, all of it already in BEF
//!
//! ```text
//!    header     flags = OBJECT (bit 11), never EXECUTABLE; entry_offset ignored
//!    Code       0x01 \
//!    RoData     0x02  |  the unit's bytes, offsets relative to EACH section
//!    Data       0x03  |
//!    Bss        0x04 /   size only
//!    Symbols    0x08     [TablaCadenas][Symbol; n][names, each ending in 0]
//!    Relocs     0x07     [Relocation; n]
//! ```
//!
//! ## Symbols
//!
//! ```text
//!    section_idx   index in the SECTION TABLE of this file, or
//!                  SECTION_UNDEFINED (0xFD): used here, defined elsewhere
//!    virt_addr     offset inside that section
//!    binding       Local  -- only this unit sees it (`static` in C)
//!                  Global -- the linker offers it to the other units
//!                  Weak   -- REFUSED for now: one promise less to keep
//!    kind          Function, Object, or Section
//! ```
//!
//! ** A SECTION symbol is what "somewhere inside this unit" points at: one
//! Local symbol per loadable section, offset 0. A `lea [rip+string]` becomes
//! `Rel32` against `.rodata` with the offset as addend, because where that
//! section lands is only known once every unit is laid out. It is always
//! Local: a section of one unit is not something another unit can name.
//!
//! An undefined symbol must be Global: a local nobody defines is a bug of the
//! unit, not a question for the linker.
//!
//! ## Relocations
//!
//! `target_section` is where the bytes to patch live, with the RELOCATION codes
//! (`0` code, `1` data, `2` rodata -- see the warning in `relocations.rs`; Bss
//! has no bytes to patch).
//!
//! ```text
//!    Rel32         symbol_idx = a symbol; writes S + A - P  (4 bytes: call, lea rip)
//!    Abs64         symbol_idx = a symbol; writes S + A      (8 bytes: a pointer)
//!    SeccionAbs64  symbol_idx = a section CODE of this unit; writes that
//!                  section's address + A (8 bytes) -- what BMO C already emits
//!    Got64         REFUSED: a GOT only exists for dynamic linking
//! ```
//!
//! ## What the linker will refuse, and this reader already does
//!
//! A malformed object is refused HERE, once, with its reason, so the linker
//! only ever reasons about well-formed units. Resolving across units --a
//! symbol defined twice, one nobody defines-- is the linker's question, not
//! this file's.

use alloc::vec::Vec;

use crate::bmo_abi::bef::header::BefFlags;
use crate::bmo_abi::bef::relocations::{Relocation, RelocationKind};
use crate::bmo_abi::bef::sections::{SectionEntry, SectionKind, TablaCadenas};
use crate::bmo_abi::bef::symbols::{Symbol, SymbolBinding, SymbolKind, SECTION_UNDEFINED};

/// Bytes of the BEF header, and where the fields this reader needs live.
const HEADER_LEN: usize = 48;
const OFF_MAGIC: usize = 0;
const OFF_FLAGS: usize = 8;
const OFF_TABLE: usize = 32;
const OFF_COUNT: usize = 40;
const MAGIC: [u8; 4] = *b"BEF1";

/// The relocation section codes, as `relocations.rs` numbers them.
pub const REL_CODE: u8 = 0;
pub const REL_DATA: u8 = 1;
pub const REL_RODATA: u8 = 2;

/// Why an object is refused. Each one names a different thing to fix.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Fault {
    /// Not even a BEF header, or a section table outside the file.
    NotBef,
    /// A BEF, but not flagged OBJECT.
    NotAnObject,
    /// OBJECT and EXECUTABLE at once: it cannot be both.
    AlsoExecutable,
    /// SHARED_LIBRARY: BMO-X links statically.
    SharedLibrary,
    /// Two sections of the same kind: which one does a symbol mean?
    DuplicateSection(u8),
    /// A section's bytes fall outside the file.
    SectionOutsideFile(u8),
    /// The Symbols section does not parse (`TablaCadenas`).
    SymbolsMalformed,
    /// Symbol `n`: its name is out of range, not NUL-terminated or not UTF-8.
    SymbolName(usize),
    /// Symbol `n`: `section_idx` names no loadable section of this file.
    SymbolSection(usize),
    /// Symbol `n`: `offset + size` falls outside its section.
    SymbolOutsideSection(usize),
    /// Symbol `n`: undefined but not Global.
    UndefinedLocal(usize),
    /// Symbol `n`: Weak, or a binding/kind this contract does not know.
    SymbolKindOrBinding(usize),
    /// Relocs is not a whole number of 24-byte entries.
    RelocsMalformed,
    /// Relocation `n`: a kind this contract does not accept (Got64, unknown).
    RelocKind(usize),
    /// Relocation `n`: `target_section` is not code, data or rodata of this file.
    RelocSection(usize),
    /// Relocation `n`: the patched bytes fall outside their section.
    RelocOutside(usize),
    /// Relocation `n`: `symbol_idx` names no symbol (or no section, for
    /// SeccionAbs64).
    RelocTarget(usize),
}

/// One symbol, read and checked.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ObjectSymbol<'a> {
    pub name: &'a str,
    /// `None` = undefined here.
    pub section: Option<SectionKind>,
    pub offset: u64,
    pub size: u64,
    pub global: bool,
    pub function: bool,
    /// The anchor of a whole section of this unit (see the header).
    pub seccion_ancla: bool,
}

/// One unit, read and checked. Borrowed from the file bytes: nothing copied.
#[derive(Debug)]
pub struct Object<'a> {
    pub code: &'a [u8],
    pub rodata: &'a [u8],
    pub data: &'a [u8],
    pub bss: u64,
    pub symbols: Vec<ObjectSymbol<'a>>,
    pub relocs: Vec<Relocation>,
}

fn u32_at(b: &[u8], at: usize) -> Option<u32> {
    Some(u32::from_le_bytes(b.get(at..at.checked_add(4)?)?.try_into().ok()?))
}

fn u64_at(b: &[u8], at: usize) -> Option<u64> {
    Some(u64::from_le_bytes(b.get(at..at.checked_add(8)?)?.try_into().ok()?))
}

/// Reads `bytes` as an object, or says why it is not one.
pub fn read(bytes: &[u8]) -> Result<Object<'_>, Fault> {
    if bytes.len() < HEADER_LEN || bytes[OFF_MAGIC..OFF_MAGIC + 4] != MAGIC {
        return Err(Fault::NotBef);
    }
    let flags = BefFlags::from_bits_truncate(u32_at(bytes, OFF_FLAGS).ok_or(Fault::NotBef)?);
    if flags.contains(BefFlags::SHARED_LIBRARY) {
        return Err(Fault::SharedLibrary);
    }
    if !flags.contains(BefFlags::OBJECT) {
        return Err(Fault::NotAnObject);
    }
    if flags.contains(BefFlags::EXECUTABLE) {
        return Err(Fault::AlsoExecutable);
    }

    // -- The section table: every entry inside the file, one of each kind. --
    let table = u64_at(bytes, OFF_TABLE).ok_or(Fault::NotBef)? as usize;
    let count = u32_at(bytes, OFF_COUNT).ok_or(Fault::NotBef)? as usize;
    let end = count
        .checked_mul(SectionEntry::SIZE)
        .and_then(|n| n.checked_add(table))
        .ok_or(Fault::NotBef)?;
    if end > bytes.len() {
        return Err(Fault::NotBef);
    }
    // (kind, file offset, file size, mem size) per table index.
    let mut entries: Vec<(u8, usize, usize, u64)> = Vec::with_capacity(count);
    for i in 0..count {
        let e = table + i * SectionEntry::SIZE;
        let kind = bytes[e];
        let off = u64_at(bytes, e + 8).ok_or(Fault::NotBef)? as usize;
        let size = u64_at(bytes, e + 16).ok_or(Fault::NotBef)? as usize;
        let mem = u64_at(bytes, e + 24).ok_or(Fault::NotBef)?;
        if off.checked_add(size).map_or(true, |f| f > bytes.len()) {
            return Err(Fault::SectionOutsideFile(kind));
        }
        if entries.iter().any(|(k, ..)| *k == kind) {
            return Err(Fault::DuplicateSection(kind));
        }
        entries.push((kind, off, size, mem));
    }
    let section = |kind: SectionKind| -> &[u8] {
        entries
            .iter()
            .find(|(k, ..)| *k == kind as u8)
            .map(|&(_, off, size, _)| &bytes[off..off + size])
            .unwrap_or(&[])
    };
    let code = section(SectionKind::Code);
    let rodata = section(SectionKind::RoData);
    let data = section(SectionKind::Data);
    let bss = entries
        .iter()
        .find(|(k, ..)| *k == SectionKind::Bss as u8)
        .map_or(0, |e| e.3);
    // How many bytes a symbol may span in each loadable kind.
    let extent = |kind: SectionKind| -> u64 {
        match kind {
            SectionKind::Code => code.len() as u64,
            SectionKind::RoData => rodata.len() as u64,
            SectionKind::Data => data.len() as u64,
            SectionKind::Bss => bss,
            _ => 0,
        }
    };

    // -- Symbols. --
    let mut symbols = Vec::new();
    let raw = section(SectionKind::Symbols);
    if !raw.is_empty() {
        let (n, strings_at) = TablaCadenas::leer(raw, Symbol::SIZE).ok_or(Fault::SymbolsMalformed)?;
        let strings = &raw[strings_at..];
        for i in 0..n {
            let at = TablaCadenas::SIZE + i * Symbol::SIZE;
            // Read field by field: the section bytes are not guaranteed aligned.
            let name_off = u32_at(raw, at).ok_or(Fault::SymbolsMalformed)? as usize;
            let offset = u64_at(raw, at + 8).ok_or(Fault::SymbolsMalformed)?;
            let size = u64_at(raw, at + 16).ok_or(Fault::SymbolsMalformed)?;
            let (kind, binding, section_idx) = (raw[at + 24], raw[at + 25], raw[at + 27]);

            let name = strings
                .get(name_off..)
                .and_then(|rest| rest.iter().position(|&b| b == 0).map(|z| &rest[..z]))
                .and_then(|n| core::str::from_utf8(n).ok())
                .filter(|n| !n.is_empty())
                .ok_or(Fault::SymbolName(i))?;

            let global = match binding {
                b if b == SymbolBinding::Local as u8 => false,
                b if b == SymbolBinding::Global as u8 => true,
                _ => return Err(Fault::SymbolKindOrBinding(i)),
            };
            let function = match kind {
                k if k == SymbolKind::Function as u8 => true,
                k if k == SymbolKind::Object as u8 => false,
                // A section anchor: always Local, and it names a section of
                // THIS unit, so it can never be undefined.
                k if k == SymbolKind::Section as u8 => {
                    if global || section_idx == SECTION_UNDEFINED {
                        return Err(Fault::SymbolKindOrBinding(i));
                    }
                    false
                }
                _ => return Err(Fault::SymbolKindOrBinding(i)),
            };

            let section = if section_idx == SECTION_UNDEFINED {
                if !global {
                    return Err(Fault::UndefinedLocal(i));
                }
                None
            } else {
                let (k, ..) = entries.get(section_idx as usize).ok_or(Fault::SymbolSection(i))?;
                let kind = match *k {
                    x if x == SectionKind::Code as u8 => SectionKind::Code,
                    x if x == SectionKind::RoData as u8 => SectionKind::RoData,
                    x if x == SectionKind::Data as u8 => SectionKind::Data,
                    x if x == SectionKind::Bss as u8 => SectionKind::Bss,
                    _ => return Err(Fault::SymbolSection(i)),
                };
                if offset.checked_add(size).map_or(true, |f| f > extent(kind)) {
                    return Err(Fault::SymbolOutsideSection(i));
                }
                Some(kind)
            };
            symbols.push(ObjectSymbol {
                name,
                section,
                offset,
                size,
                global,
                function,
                seccion_ancla: kind == SymbolKind::Section as u8,
            });
        }
    }

    // -- Relocations. --
    let mut relocs = Vec::new();
    let raw = section(SectionKind::Relocs);
    if raw.len() % Relocation::SIZE != 0 {
        return Err(Fault::RelocsMalformed);
    }
    for i in 0..raw.len() / Relocation::SIZE {
        let at = i * Relocation::SIZE;
        let r = Relocation {
            offset: u64_at(raw, at).ok_or(Fault::RelocsMalformed)?,
            symbol_idx: u32_at(raw, at + 8).ok_or(Fault::RelocsMalformed)?,
            kind: raw[at + 12],
            target_section: raw[at + 13],
            _pad: [0; 2],
            addend: u64_at(raw, at + 16).ok_or(Fault::RelocsMalformed)? as i64,
        };
        let width = match RelocationKind::from_u8(r.kind) {
            Some(RelocationKind::Rel32) => 4,
            Some(RelocationKind::Abs64) | Some(RelocationKind::SeccionAbs64) => 8,
            _ => return Err(Fault::RelocKind(i)),
        };
        let patched = match r.target_section {
            REL_CODE => code,
            REL_DATA => data,
            REL_RODATA => rodata,
            _ => return Err(Fault::RelocSection(i)),
        };
        if r.offset.checked_add(width).map_or(true, |f| f > patched.len() as u64) {
            return Err(Fault::RelocOutside(i));
        }
        let target_ok = if r.kind == RelocationKind::SeccionAbs64 as u8 {
            matches!(r.symbol_idx as u8, REL_CODE | REL_DATA | REL_RODATA) && r.symbol_idx <= 2
        } else {
            (r.symbol_idx as usize) < symbols.len()
        };
        if !target_ok {
            return Err(Fault::RelocTarget(i));
        }
        relocs.push(r);
    }

    Ok(Object { code, rodata, data, bss, symbols, relocs })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::bmo_abi::bef::symbols::{name_hash, SymbolVisibility};
    use crate::bmo_abi::bef::writer::{BefBuilder, BefSection};
    use alloc::string::String;
    use alloc::vec;

    fn sym(name_off: u32, name: &str, section_idx: u8, offset: u64, size: u64, global: bool, function: bool) -> Symbol {
        Symbol {
            name_off,
            name_hash: name_hash(name),
            virt_addr: offset,
            size,
            kind: if function { SymbolKind::Function } else { SymbolKind::Object } as u8,
            binding: if global { SymbolBinding::Global } else { SymbolBinding::Local } as u8,
            visibility: SymbolVisibility::Default as u8,
            section_idx,
            _reserved: 0,
        }
    }

    /// `main` (global, defined) calls `suma` (global, UNDEFINED), and loads the
    /// address of `contador` from data. Sections are added code(0), data(1).
    fn good(flags: u32, relocs: Vec<Relocation>, symbols: Vec<Symbol>) -> Vec<u8> {
        let mut b = BefBuilder::new();
        b.header.flags = flags;
        b.add_section(BefSection::code(vec![0xE8, 0, 0, 0, 0, 0xC3, 0x90, 0x90]));
        b.add_section(BefSection::data(vec![7, 0, 0, 0, 0, 0, 0, 0]));
        let mut names = Vec::new();
        for n in ["main", "suma", "contador"] {
            names.extend_from_slice(n.as_bytes());
            names.push(0);
        }
        b.add_section(BefSection::symbols(symbols, names));
        b.add_section(BefSection::relocs(relocs));
        b.build().expect("the test object must build")
    }

    fn symbols() -> Vec<Symbol> {
        vec![
            sym(0, "main", 0, 0, 6, true, true),
            sym(5, "suma", SECTION_UNDEFINED, 0, 0, true, true),
            sym(10, "contador", 1, 0, 8, false, false),
        ]
    }

    fn call_suma() -> Relocation {
        Relocation { offset: 1, symbol_idx: 1, kind: RelocationKind::Rel32 as u8, target_section: REL_CODE, _pad: [0; 2], addend: -4 }
    }

    const OBJ: u32 = BefFlags::OBJECT.bits();

    #[test]
    fn a_good_object_reads_whole() {
        let bytes = good(OBJ, vec![call_suma()], symbols());
        let o = read(&bytes).expect("a good object");
        assert_eq!(o.code.len(), 8);
        assert_eq!(o.data.len(), 8);
        let names: Vec<String> = o.symbols.iter().map(|s| s.name.into()).collect();
        assert_eq!(names, ["main", "suma", "contador"]);
        assert_eq!(o.symbols[1].section, None, "suma is used here and defined elsewhere");
        assert_eq!(o.symbols[2].section, Some(SectionKind::Data));
        assert!(!o.symbols[2].global);
        assert_eq!(o.relocs.len(), 1);
    }

    #[test]
    fn it_is_an_object_or_it_is_nothing() {
        let e = BefFlags::EXECUTABLE.bits();
        assert_eq!(read(&good(e, vec![], symbols())).err(), Some(Fault::NotAnObject));
        assert_eq!(read(&good(OBJ | e, vec![], symbols())).err(), Some(Fault::AlsoExecutable));
        assert_eq!(read(&good(OBJ | BefFlags::SHARED_LIBRARY.bits(), vec![], symbols())).err(), Some(Fault::SharedLibrary));
        assert_eq!(read(b"no soy un BEF").err(), Some(Fault::NotBef));
    }

    #[test]
    fn a_symbol_must_say_where_it_lives_and_fit_there() {
        let mut s = symbols();
        s[1] = sym(5, "suma", SECTION_UNDEFINED, 0, 0, false, true);
        assert_eq!(read(&good(OBJ, vec![], s)).err(), Some(Fault::UndefinedLocal(1)));

        let mut s = symbols();
        s[2] = sym(10, "contador", 1, 4, 8, false, false);
        assert_eq!(read(&good(OBJ, vec![], s)).err(), Some(Fault::SymbolOutsideSection(2)));

        let mut s = symbols();
        s[0] = sym(0, "main", 9, 0, 6, true, true);
        assert_eq!(read(&good(OBJ, vec![], s)).err(), Some(Fault::SymbolSection(0)));

        let mut s = symbols();
        s[0].binding = SymbolBinding::Weak as u8;
        assert_eq!(read(&good(OBJ, vec![], s)).err(), Some(Fault::SymbolKindOrBinding(0)));

        let mut s = symbols();
        s[0].name_off = 999;
        assert_eq!(read(&good(OBJ, vec![], s)).err(), Some(Fault::SymbolName(0)));
    }

    #[test]
    fn a_relocation_must_patch_inside_and_point_at_something() {
        let mut r = call_suma();
        r.offset = 6; // 6 + 4 > 8
        assert_eq!(read(&good(OBJ, vec![r], symbols())).err(), Some(Fault::RelocOutside(0)));

        let mut r = call_suma();
        r.symbol_idx = 3;
        assert_eq!(read(&good(OBJ, vec![r], symbols())).err(), Some(Fault::RelocTarget(0)));

        let mut r = call_suma();
        r.kind = RelocationKind::Got64 as u8;
        assert_eq!(read(&good(OBJ, vec![r], symbols())).err(), Some(Fault::RelocKind(0)), "a GOT is dynamic linking");

        let mut r = call_suma();
        r.target_section = 7;
        assert_eq!(read(&good(OBJ, vec![r], symbols())).err(), Some(Fault::RelocSection(0)));

        let seccion = Relocation { offset: 0, symbol_idx: REL_DATA as u32, kind: RelocationKind::SeccionAbs64 as u8, target_section: REL_DATA, _pad: [0; 2], addend: 0 };
        assert!(read(&good(OBJ, vec![seccion], symbols())).is_ok());
        let mala = Relocation { symbol_idx: 5, ..seccion };
        assert_eq!(read(&good(OBJ, vec![mala], symbols())).err(), Some(Fault::RelocTarget(0)));
    }

    /// The general validator -- the gate every frontend calls before writing --
    /// accepts an object, and refuses the same bytes flagged as an IMAGE: an
    /// undefined symbol in an executable is a jump into nothing.
    #[test]
    fn the_validator_knows_an_object_from_an_image() {
        use crate::bmo_abi::bef::validator::validate;
        let obj = good(OBJ, vec![call_suma()], symbols());
        let r = validate(&obj);
        assert!(r.is_valid, "{:?}", r.issues);
        let img = good(BefFlags::EXECUTABLE.bits(), vec![], symbols());
        let r = validate(&img);
        assert!(!r.is_valid, "an image with an undefined symbol must be refused");
        let both = good(OBJ | BefFlags::EXECUTABLE.bits(), vec![], symbols());
        assert!(!validate(&both).is_valid);
        let shared = good(BefFlags::SHARED_LIBRARY.bits(), vec![], symbols());
        assert!(!validate(&shared).is_valid, "BMO-X links statically");
    }

    /// The reader faces bytes a compiler wrote -- today ours, tomorrow a
    /// third party's. Same hostile pass as every other reader of BEF.
    #[test]
    fn hostile_objects_never_panic() {
        let good = good(OBJ, vec![call_suma()], symbols());
        bmo_hostile::attack("objeto", bmo_hostile::DEFAULT_SEED, 20_000, &[&good], 2048, |x| {
            if let Ok(o) = read(x) {
                for s in &o.symbols {
                    let _ = (s.name.len(), s.section, s.offset);
                }
            }
        });
    }
}
