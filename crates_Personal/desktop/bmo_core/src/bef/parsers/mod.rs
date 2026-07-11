//! Loader BEF - entry point unico que devora 2 formatos: BEF, ELF.
//!
//! `	ext
//!   bef::load(bytes) ---> detect_format ---> native::load (BEF)
//!                                       ---> elf::load    (Linux/Unix)
//!                              |
//!                              v
//!                          Image (representacion BEF unificada)
//! `

#![allow(dead_code)]

extern crate alloc;

pub mod native;
pub mod elf;
pub mod elf_dynamic;
pub mod elf_thunks;
pub mod meta_sections;
pub mod runtime;

#[cfg(test)]
pub mod tests;

use crate::bef::format::header::BefMagic;
use crate::bef::format::manifest::{Manifest, Provenance};

/// Formato detectado del binario.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BinaryFormat {
    /// BEF nativo de BMO.
    BefNative,
    /// ELF devorado y traducido a BEF interno.
    ElfDevoured,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LoadError {
    UnknownFormat,
    Truncated,
    InvalidHeader,
    UnsupportedArch,
    UnsupportedAbi,
    SectionOutOfRange,
    HashMismatch,
    NotImplemented,
}

/// Imagen cargada en memoria - representacion BEF unificada (cualquier
/// origen acaba aqui).
pub struct Image {
    pub format: BinaryFormat,
    pub manifest: Manifest,
    pub entry_point: u64,
    pub baseess: u64,
    pub sections: alloc::vec::Vec<MappedSection>,
    /// TLS template offset (0 = no TLS).
    pub tls_offset: u64,
    /// TLS template size.
    pub tls_size: u64,
}

#[derive(Debug, Clone, Copy)]
pub struct MappedSection {
    pub kind: u8,           // SectionKind as u8
    pub virt_addr: u64,
    pub size: u64,
    pub flags: u32,
    /// Pointer to the actual data in memory (0 = metadata only).
    pub data_ptr: u64,
}

/// Punto de entrada universal - detecta formato y delega al sub-loader.
pub fn load(bytes: &[u8]) -> Result<Image, LoadError> {
    // Initialize runtime symbol table if not done.
    static INIT: core::sync::atomic::AtomicBool = core::sync::atomic::AtomicBool::new(false);
    if INIT.compare_exchange(false, true, core::sync::atomic::Ordering::AcqRel, core::sync::atomic::Ordering::Acquire).is_ok() {
        runtime::init();
    }

    match BefMagic::detect(bytes) {
        BefMagic::BefNative => native::load(bytes),
        BefMagic::ElfUnix   => elf::load(bytes),
        _                   => Err(LoadError::UnknownFormat),
    }
}

/// Execute the entry point of a loaded image.
///
/// SAFETY: The image must have a valid entry_point and all relocations
/// resolved, AND every section's `virt_addr` range must be mapped as
/// USER-accessible in the current page table.  This jumps to Ring 3
/// (user mode) and does not return.
///
/// NOTE: The stack is allocated from identity-mapped physical pages
/// (same as `jump_to_ring3`).  The kernel heap (HIGH_MEM_BASE) cannot
/// be used because it lacks the USER page-table flag.
pub unsafe fn run_entry_point(img: &Image) -> ! {
    let entry = img.entry_point;
    if entry == 0 {
        crate::cabina::fault("bef", "entry point is NULL");
        loop { core::arch::asm!("hlt"); }
    }

    crate::cabina::info_u64("bef", "executing entry point", entry);

    use crate::mm::phys;
    use crate::mm::virt;

    // Allocate a 64 KB user stack from identity-mapped physical pages.
    const STACK_PAGES: usize = 16;
    const PAGE_SIZE: u64 = 4096;
    let stack_phys = match phys::alloc_pages_contiguous(STACK_PAGES) {
        Some(p) => p,
        None => {
            crate::cabina::fault("bef", "OOM for user stack");
            loop { core::arch::asm!("hlt"); }
        }
    };

    // Mark stack pages USER-accessible.
    let _ = virt::mark_current_identity_user_range(
        stack_phys, STACK_PAGES * PAGE_SIZE as usize,
    );

    // Zero the stack (identity-mapped).
    core::ptr::write_bytes(stack_phys as *mut u8, 0, STACK_PAGES * PAGE_SIZE as usize);

    let stack_top = stack_phys + (STACK_PAGES as u64) * PAGE_SIZE;

    crate::ring3::transition::ring3_transition(entry, stack_top);
}

/// Helper compartido - sintetiza una MappedSection vacia.
pub(crate) fn placeholder_section(kind: u8) -> MappedSection {
    MappedSection { kind, virt_addr: 0, size: 0, flags: 0, data_ptr: 0 }
}

pub(crate) fn fake_provenance_image(prov: Provenance) -> Image {
    Image {
        format: match prov {
            Provenance::Native      => BinaryFormat::BefNative,
            Provenance::ElfDevoured => BinaryFormat::ElfDevoured,
            _                       => BinaryFormat::BefNative,
        },
        manifest: Manifest::synthetic_for("(stub)", prov),
        entry_point: 0,
        baseess: 0,
        sections: alloc::vec::Vec::new(),
        tls_offset: 0,
        tls_size: 0,
    }
}
