//! **Validating a Symbols section** -- out of `validator.rs` on 2026-09-17.
//!
//! Moved whole, without changing its logic, when the fix of its eight-byte
//! shift (entries read on top of `TablaCadenas`) made the parent file grow
//! past its L6a ceiling. It is a closed question -- one section's bytes in,
//! issues out -- and that is why it is a clean cut.

use super::*;

pub(super) fn validate_symbol_section(
    entry: &SectionEntry,
    bytes: &[u8],
    sections: &[SectionEntry],
    idx: usize,
    is_object: bool,
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
    // ** LA CABECERA MANDA. Antes esto era `data.len() / entry_sz`, que da por
    // hecho que TODO el dato son entradas -- y entonces las cadenas empiezan
    // donde acaba la seccion y miden cero. Ver `TablaCadenas`.
    let Some((count, string_start)) = TablaCadenas::leer(data, entry_sz) else {
        r.error_at(idx, "la cabecera de la seccion declara mas entradas de las que caben");
        return;
    };
    let strings = &data[string_start..];

    for i in 0..count {
        // *** THE ENTRIES START AFTER THE HEADER (2026-09-17). This read
        // `data.as_ptr().add(i)` -- entry 0 on top of the 8 bytes of
        // `TablaCadenas` -- so since 08-14 every Symbols section was validated
        // SHIFTED: the entry count read as a name offset, the name hash as a
        // kind. BMO C's tables passed by luck, because the shifted numbers
        // happened to be small. Found writing the first test with an
        // undefined symbol (`bef::objeto`). And it built a `&Symbol` over bytes
        // that are not guaranteed to be aligned, which is undefined behaviour
        // even in `unsafe`: now it is a copy, read unaligned.
        let at = TablaCadenas::SIZE + i * entry_sz;
        let sym: Symbol = unsafe { core::ptr::read_unaligned(data[at..at + entry_sz].as_ptr() as *const Symbol) };
        let off = sym.name_off as usize;
        // Names end in 0 -- the contract of the only producer, BMO C.
        if !strings.get(off..).map_or(false, |rest| rest.contains(&0)) {
            r.error_at(
                idx,
                format!("symbol[{}]: name_off {:#x} out of strings range, or no 0 ends the name", i, off),
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
            // An undefined symbol is what an object is FOR; in an image it
            // means something was never linked, and the image would jump into
            // nothing.
            crate::bef::symbols::SECTION_UNDEFINED if is_object => {}
            crate::bef::symbols::SECTION_UNDEFINED => r.error_at(
                idx,
                format!("symbol[{}]: undefined in an image -- it was never linked", i),
            ),
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
