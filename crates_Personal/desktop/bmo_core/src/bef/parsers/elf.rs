use super::{Image, LoadError, MappedSection, fake_provenance_image};
use crate::bef::format::manifest::Provenance;
use goblin::elf::{Elf, program_header, reloc};

pub fn load(bytes: &[u8]) -> Result<Image, LoadError> {
    let elf = Elf::parse(bytes).map_err(|_| LoadError::InvalidHeader)?;
    if !elf.is_64 {
        return Err(LoadError::UnsupportedArch);
    }

    let mut img = fake_provenance_image(Provenance::ElfDevoured);
    img.entry_point = elf.entry;

    // Validate entry point falls within a loaded segment.
    let entry_valid = elf.program_headers.iter()
        .filter(|ph| ph.p_type == program_header::PT_LOAD)
        .any(|ph| elf.entry >= ph.p_vaddr && elf.entry < ph.p_vaddr + ph.p_memsz);
    if !entry_valid && elf.entry != 0 {
        return Err(LoadError::InvalidHeader);
    }

    for phdr in &elf.program_headers {
        if phdr.p_type != program_header::PT_LOAD { continue; }
        let mut flags = 0u32;
        if phdr.p_flags & program_header::PF_R != 0 { flags |= 0x1; }
        if phdr.p_flags & program_header::PF_W != 0 { flags |= 0x2; }
        if phdr.p_flags & program_header::PF_X != 0 { flags |= 0x4; }
        let kind = pick_kind_from_flags(phdr.p_flags);

        let alloc_size = phdr.p_memsz as usize;
        if alloc_size > 0x0010_0000_0000 {
            continue; // Reject absurdly large segments (>256 GB).
        }
        let page_size = crate::mm::phys::page_size() as u64;
        let aligned_size = (alloc_size + page_size as usize - 1) & !(page_size as usize - 1);
        let pages = aligned_size / page_size as usize;
        let cr3 = crate::mm::virt::read_cr3();

        let data_ptr = if aligned_size > 0 {
            let phys = match crate::mm::phys::alloc_pages_contiguous(pages) {
                Some(p) => p,
                None => return Err(LoadError::SectionOutOfRange),
            };

            let mut pt_flags = crate::mm::virt::flags::PRESENT | crate::mm::virt::flags::USER;
            if flags & 0x2 != 0 { pt_flags |= crate::mm::virt::flags::WRITABLE; }
            if flags & 0x4 == 0 { pt_flags |= crate::mm::virt::flags::NO_EXECUTE; }

            if crate::mm::virt::map_user_range(cr3, phdr.p_vaddr, phys, pages, pt_flags).is_err() {
                return Err(LoadError::SectionOutOfRange);
            }

            let ptr = crate::mm::virt::phys_to_virt(phys) as *mut u8;
            let copy_len = (phdr.p_filesz as usize).min(bytes.len().saturating_sub(phdr.p_offset as usize));
            if copy_len > 0 {
                let src = &bytes[phdr.p_offset as usize..phdr.p_offset as usize + copy_len];
                unsafe { core::ptr::copy_nonoverlapping(src.as_ptr(), ptr, copy_len); }
            }
            ptr as u64
        } else {
            0
        };

        img.sections.push(MappedSection {
            kind,
            virt_addr: phdr.p_vaddr,
            size: phdr.p_memsz,
            flags,
            data_ptr,
        });
    }

    apply_elf_relocations(&elf, &mut img, bytes);
    resolve_plt_symbols(&elf, &mut img, bytes);

    let tls_segment = elf.program_headers.iter()
        .find(|ph| ph.p_type == program_header::PT_TLS);
    if let Some(tls) = tls_segment {
        img.tls_offset = tls.p_vaddr;
        img.tls_size = tls.p_memsz;
    }

    Ok(img)
}

fn apply_elf_relocations(elf: &Elf, img: &mut Image, bytes: &[u8]) {
    for reloc in elf.dynrelas.iter() {
        let bef_kind = match elf_reloc_to_bef(reloc.r_type) {
            Some(k) => k,
            None => continue,
        };
        apply_one_reloc(img, &reloc, bef_kind, bytes);
    }
    for reloc in elf.pltrelocs.iter() {
        let bef_kind = match elf_reloc_to_bef(reloc.r_type) {
            Some(k) => k,
            None => continue,
        };
        apply_one_reloc(img, &reloc, bef_kind, bytes);
    }
    for (_idx, sec) in &elf.shdr_relocs {
        for reloc in sec.iter() {
            let bef_kind = match elf_reloc_to_bef(reloc.r_type) {
                Some(k) => k,
                None => continue,
            };
            apply_one_reloc(img, &reloc, bef_kind, bytes);
        }
    }
}

fn apply_one_reloc(
    img: &mut Image,
    reloc: &reloc::Reloc,
    bef_kind: u8,
    _bytes: &[u8],
) {
    let target_va = reloc.r_offset;

    for section in &mut img.sections {
        if target_va >= section.virt_addr
            && target_va < section.virt_addr + section.size
        {
            if section.data_ptr == 0 { break; }
            let offset_in_section = (target_va - section.virt_addr) as usize;
            if offset_in_section + 8 > section.size as usize { break; }

            let addend = reloc.r_addend.unwrap_or(0);
            let symbol_addr = if reloc.r_type == reloc::R_X86_64_RELATIVE {
                target_va.wrapping_add(addend as u64)
            } else {
                0
            };

            let target_slice = unsafe {
                core::slice::from_raw_parts_mut(
                    section.data_ptr as *mut u8,
                    section.size as usize,
                )
            };

            let bef_reloc = crate::bef::relocations::Relocation {
                offset: offset_in_section as u64,
                symbol_idx: 0,
                kind: bef_kind,
                target_section: 0,
                _pad: [0; 2],
                addend,
            };

            let _ = crate::bef::relocations::apply(
                &bef_reloc,
                target_slice,
                target_va,
                symbol_addr,
            );
            break;
        }
    }
}

pub fn elf_reloc_to_bef(r_type: u32) -> Option<u8> {
    use crate::bef::relocations::RelocationKind;
    match r_type {
        reloc::R_X86_64_64 | reloc::R_X86_64_RELATIVE => Some(RelocationKind::Abs64 as u8),
        reloc::R_X86_64_PC32 | reloc::R_X86_64_PLT32 => Some(RelocationKind::Rel32 as u8),
        reloc::R_X86_64_GLOB_DAT | reloc::R_X86_64_JUMP_SLOT => Some(RelocationKind::Got64 as u8),
        _ => None,
    }
}

fn pick_kind_from_flags(p_flags: u32) -> u8 {
    use crate::bef::sections::SectionKind;
    let x = p_flags & 1 != 0;
    let w = p_flags & 2 != 0;
    match (x, w) {
        (true, _)      => SectionKind::Code as u8,
        (false, true)  => SectionKind::Data as u8,
        (false, false) => SectionKind::RoData as u8,
    }
}

fn resolve_plt_symbols(elf: &Elf, img: &mut Image, _bytes: &[u8]) {
    let def_lib = elf.libraries.first().copied().unwrap_or("libc.so.6");
    for reloc in elf.pltrelocs.iter() {
        if reloc.r_type != reloc::R_X86_64_JUMP_SLOT {
            continue;
        }
        let sym = elf.dynsyms.get(reloc.r_sym);
        let sym_name = sym.and_then(|s| {
            elf.dynstrtab.get_at(s.st_name)
                .filter(|n| !n.is_empty())
        });
        let name = match sym_name {
            Some(n) => n,
            _ => continue,
        };
        let fn_ptr = super::elf_thunks::resolve_fn_ptr(def_lib, name);
        let addr = fn_ptr.unwrap_or(super::elf_thunks::silent_stub as *const ()) as u64;
        let got_va = reloc.r_offset;
        for section in &img.sections {
            if got_va >= section.virt_addr && got_va < section.virt_addr + section.size {
                if section.data_ptr != 0 {
                    let offset_in_section = (got_va - section.virt_addr) as usize;
                    if offset_in_section + 8 <= section.size as usize {
                        unsafe {
                            let ptr = (section.data_ptr as *mut u8).add(offset_in_section) as *mut u64;
                            core::ptr::write(ptr, addr);
                        }
                    }
                }
                break;
            }
        }
    }
}
