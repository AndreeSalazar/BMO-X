use super::{Image, LoadError, MappedSection, fake_provenance_image};
use crate::bmo_core::bef::format::manifest::Provenance;
use goblin::pe::PE;

pub const FAKE_DLLS: &[&str] = &[
    "d3d12.dll", "d3d11.dll", "d3d9.dll", "dxgi.dll",
    "xinput1_4.dll", "xaudio2_9.dll", "ws2_32.dll",
    "winhttp.dll", "kernel32.dll", "user32.dll", "ntdll.dll",
];

pub fn load(bytes: &[u8]) -> Result<Image, LoadError> {
    let pe = PE::parse(bytes).map_err(|_| LoadError::InvalidHeader)?;
    if !pe.is_64 {
        return Err(LoadError::UnsupportedArch);
    }
    let base = pe.image_base;
    let entry = pe.entry as u64;

    let mut img = fake_provenance_image(Provenance::PeDevoured);
    img.entry_point = base.wrapping_add(entry);
    img.baseess = base;

    for s in &pe.sections {
        let va = s.virtual_address as u64;
        let vsz = s.virtual_size.max(s.size_of_raw_data) as u64;
        let chr = s.characteristics;
        let mut flags = 0u32;
        if chr & 0x4000_0000 != 0 { flags |= 0x1; }
        if chr & 0x8000_0000 != 0 { flags |= 0x2; }
        if chr & 0x2000_0000 != 0 { flags |= 0x4; }

        let kind = pick_section_kind(&s.name, chr);

        let raw_data_size = s.size_of_raw_data as usize;
        let alloc_size = raw_data_size.max(s.virtual_size as usize);
        let align = 4096usize;
        let aligned_size = (alloc_size + align - 1) & !(align - 1);

        let data_ptr = if aligned_size > 0 {
            let layout = core::alloc::Layout::from_size_align(aligned_size, align)
                .map_err(|_| LoadError::SectionOutOfRange)?;
            let ptr = unsafe { alloc::alloc::alloc_zeroed(layout) };
            if ptr.is_null() {
                return Err(LoadError::SectionOutOfRange);
            }
            if raw_data_size > 0 && (s.pointer_to_raw_data as usize + raw_data_size) <= bytes.len() {
                let src = &bytes[s.pointer_to_raw_data as usize..s.pointer_to_raw_data as usize + raw_data_size];
                unsafe { core::ptr::copy_nonoverlapping(src.as_ptr(), ptr, raw_data_size); }
            }
            ptr as u64
        } else {
            0
        };

        img.sections.push(MappedSection {
            kind,
            virt_addr: base.wrapping_add(va),
            size: vsz,
            flags,
            data_ptr,
        });
    }

    if let Some(ref reloc_data) = pe.relocation_data {
        apply_pe_relocations(&mut img, reloc_data, base);
    }

    for import in &pe.imports {
        let (target, fn_ptr) = crate::bmo_gpu::shims::pe_thunks::resolve_fn(import.dll, &import.name);
        let addr = match target {
            crate::bmo_gpu::shims::pe_thunks::ThunkTarget::SilentStub =>
                crate::bmo_gpu::shims::pe_thunks::silent_stub as *const () as u64,
            crate::bmo_gpu::shims::pe_thunks::ThunkTarget::LogStub =>
                crate::bmo_gpu::shims::pe_thunks::log_stub as *const () as u64,
            _ => fn_ptr,
        };
        let static_dll: &'static str = leak_str(import.dll);
        let static_fn: &'static str = leak_str(&import.name);
        super::runtime::register_symbol(static_dll, static_fn, addr,
            super::runtime::SYM_PE_THUNK | super::runtime::SYM_EAGER);
    }

    Ok(img)
}

fn apply_pe_relocations(
    img: &mut Image,
    reloc_data: &goblin::pe::relocation::RelocationData,
    base: u64,
) {
    for block_result in reloc_data.blocks() {
        let block = match block_result {
            Ok(b) => b,
            _ => continue,
        };
        let block_rva = block.rva as u64;
        for word_result in block.words() {
            let word = match word_result {
                Ok(w) => w,
                _ => continue,
            };
            if word.reloc_type() as u16 != goblin::pe::relocation::IMAGE_REL_BASED_DIR64 {
                continue;
            }
            let target_rva = block_rva + word.offset() as u64;
            for section in &img.sections {
                let sec_rva = section.virt_addr - img.baseess;
                if target_rva >= sec_rva && target_rva < sec_rva + section.size {
                    let offset_in_section = (target_rva - sec_rva) as usize;
                    if section.data_ptr != 0 && offset_in_section + 8 <= section.size as usize {
                        unsafe {
                            let ptr = section.data_ptr as *mut u64;
                            let val = ptr.add(offset_in_section / 8).read();
                            ptr.add(offset_in_section / 8).write(val.wrapping_add(base));
                        }
                    }
                    break;
                }
            }
        }
    }
}

fn leak_str(s: &str) -> &'static str {
    let len = s.len();
    let layout = core::alloc::Layout::from_size_align(len, 1).ok().unwrap_or(return "");
    let ptr = unsafe { alloc::alloc::alloc(layout) };
    if ptr.is_null() { return ""; }
    unsafe {
        core::ptr::copy_nonoverlapping(s.as_ptr(), ptr, len);
        core::str::from_utf8_unchecked(core::slice::from_raw_parts(ptr, len))
    }
}

fn pick_section_kind(name: &[u8; 8], chr: u32) -> u8 {
    use crate::bmo_core::bef::sections::SectionKind;
    let n = core::str::from_utf8(name).unwrap_or("");
    if n.starts_with(".text") || (chr & 0x2000_0000) != 0 { return SectionKind::Code as u8 }
    if n.starts_with(".rdata") || n.starts_with(".rodata") { return SectionKind::RoData as u8 }
    if n.starts_with(".data") { return SectionKind::Data as u8 }
    if n.starts_with(".bss") { return SectionKind::Bss as u8 }
    if n.starts_with(".idata") { return SectionKind::Imports as u8 }
    if n.starts_with(".edata") { return SectionKind::Exports as u8 }
    if n.starts_with(".reloc") { return SectionKind::Relocs as u8 }
    if n.starts_with(".rsrc") { return SectionKind::Resources as u8 }
    if n.starts_with(".tls") { return SectionKind::Tls as u8 }
    if n.starts_with(".pdata") { return SectionKind::Unwind as u8 }
    if n.starts_with(".debug") { return SectionKind::Debug as u8 }
    SectionKind::Data as u8
}
