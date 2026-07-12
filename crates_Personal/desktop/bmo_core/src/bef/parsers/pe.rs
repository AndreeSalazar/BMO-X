use super::{Image, LoadError, MappedSection, fake_provenance_image};
use crate::bef::format::manifest::Provenance;
use goblin::pe::{PE, import};

pub fn load(bytes: &[u8]) -> Result<Image, LoadError> {
    let pe = PE::parse(bytes).map_err(|_| LoadError::InvalidHeader)?;
    if !pe.is_64 {
        return Err(LoadError::UnsupportedArch);
    }

    let mut img = fake_provenance_image(Provenance::PeDevoured);
    let image_base = pe.image_base;
    let entry_rva = pe.entry as u64;
    img.entry_point = image_base + entry_rva;
    img.baseess = image_base;

    let page_size = crate::mm::phys::page_size() as u64;
    let cr3 = crate::mm::virt::read_cr3();

    let iat_rvas = pe.import_data.as_ref().map(|d| {
        d.import_data.iter().map(|e| e.import_directory_entry.import_address_table_rva as u64).collect::<alloc::vec::Vec<_>>()
    }).unwrap_or_default();

    for section in &pe.sections {
        let mem_size = section.virtual_size.max(section.size_of_raw_data) as usize;
        if mem_size == 0 {
            continue;
        }

        let aligned_size = (mem_size + page_size as usize - 1) & !(page_size as usize - 1);
        let pages = aligned_size / page_size as usize;
        let va = image_base + section.virtual_address as u64;

        let phys = crate::mm::phys::alloc_pages_contiguous(pages)
            .ok_or(LoadError::SectionOutOfRange)?;

        let characteristics = section.characteristics;
        let is_code = characteristics & 0x2000_0000 != 0;
        let mut is_writable = characteristics & 0x8000_0000 != 0;
        let is_exec = characteristics & 0x2000_0000 != 0;

        let sec_va = image_base + section.virtual_address as u64;
        let sec_end = sec_va + (section.virtual_size.max(section.size_of_raw_data) as u64);
        let contains_iat = iat_rvas.iter().any(|&rva| {
            let iat_va = image_base + rva;
            iat_va >= sec_va && iat_va < sec_end
        });
        if contains_iat {
            is_writable = true;
        }

        let flags = {
            let mut f = 0u32;
            f |= 0x1; // Always readable
            if is_writable { f |= 0x2; }
            if is_exec { f |= 0x4; }
            f
        };

        let mut pt_flags = crate::mm::virt::flags::PRESENT | crate::mm::virt::flags::USER;
        if flags & 0x2 != 0 { pt_flags |= crate::mm::virt::flags::WRITABLE; }
        if flags & 0x4 == 0 { pt_flags |= crate::mm::virt::flags::NO_EXECUTE; }

        if crate::mm::virt::map_user_range(cr3, va, phys, pages, pt_flags).is_err() {
            return Err(LoadError::SectionOutOfRange);
        }

        let ptr = crate::mm::virt::phys_to_virt(phys) as *mut u8;
        unsafe { core::ptr::write_bytes(ptr, 0, aligned_size); }

        let file_off = section.pointer_to_raw_data as usize;
        let copy_len = (section.size_of_raw_data as usize).min(bytes.len().saturating_sub(file_off));
        if copy_len > 0 {
            let src = &bytes[file_off..file_off + copy_len];
            unsafe { core::ptr::copy_nonoverlapping(src.as_ptr(), ptr, copy_len); }
        }

        let kind = if is_code { 0x01 }
        else if is_writable { 0x03 }
        else { 0x02 };

        img.sections.push(MappedSection {
            kind,
            virt_addr: va,
            size: aligned_size as u64,
            flags,
            data_ptr: ptr as u64,
        });
    }

    if let Some(relocs) = pe.relocation_data {
        apply_pe_relocations(&mut img, &relocs, image_base);
    }

    if let Some(imports) = pe.import_data {
        resolve_pe_imports(&mut img, &imports, &pe.imports, &pe.libraries);
    }

    Ok(img)
}

fn apply_pe_relocations(img: &mut Image, relocs: &goblin::pe::relocation::RelocationData, image_base: u64) {
    let delta = img.baseess.wrapping_sub(image_base);
    if delta == 0 { return; }

    for block_res in relocs.blocks() {
        let block = match block_res {
            Ok(b) => b,
            Err(_) => continue,
        };
        for word_res in block.words() {
            let word = match word_res {
                Ok(w) => w,
                Err(_) => continue,
            };
            let offset = word.offset();
            let typ = word.reloc_type();
            if typ == 0 { continue; }

            let rva = block.rva as u64 + offset as u64;

            for section in &mut img.sections {
                if rva >= section.virt_addr && rva < section.virt_addr + section.size {
                    if section.data_ptr == 0 { break; }
                    let offset_in_sec = (rva - section.virt_addr) as usize;
                    if offset_in_sec + 8 > section.size as usize { break; }

                    let ptr = section.data_ptr as *mut u8;
                    match typ {
                        3 => {
                            let val = unsafe {
                                (ptr.add(offset_in_sec) as *const u32).read_unaligned()
                            };
                            unsafe {
                                (ptr.add(offset_in_sec) as *mut u32).write_unaligned(val.wrapping_add(delta as u32));
                            }
                        }
                        10 => {
                            let val = unsafe {
                                (ptr.add(offset_in_sec) as *const u64).read_unaligned()
                            };
                            unsafe {
                                (ptr.add(offset_in_sec) as *mut u64).write_unaligned(val.wrapping_add(delta));
                            }
                        }
                        _ => {}
                    }
                    break;
                }
            }
        }
    }
}

fn resolve_pe_imports(
    img: &mut Image,
    _import_data: &import::ImportData,
    goblin_imports: &[import::Import],
    _libraries: &[&str],
) {
    for imp in goblin_imports {
        let iat_va = img.baseess + imp.offset as u64;
        let name = &*imp.name;
        if name.is_empty() && imp.ordinal != 0 {
            continue;
        }

        let fn_ptr = super::pe_thunks::resolve_fn_ptr(imp.dll, name);

        let addr = match fn_ptr {
            Some(p) => p as u64,
            None => continue,
        };

        for section in &mut img.sections {
            if iat_va >= section.virt_addr && iat_va + 8 <= section.virt_addr + section.size {
                if section.data_ptr == 0 { break; }
                let offset_in_sec = (iat_va - section.virt_addr) as usize;
                unsafe {
                    let ptr = (section.data_ptr as *mut u8).add(offset_in_sec) as *mut u64;
                    core::ptr::write(ptr, addr);
                }
                break;
            }
        }
    }
}
