//! Loader BEF — entry point único que devora 3 formatos: BEF, PE, ELF.
//!
//! ```text
//!   bef::load(bytes) ──▶ detect_format ─┬──▶ native::load (BEF)
//!                                       ├──▶ pe::load     (Windows .exe/.dll)
//!                                       └──▶ elf::load    (Linux/Unix)
//!                              │
//!                              ▼
//!                          Image (representación BEF unificada)
//! ```

#![allow(dead_code)]

extern crate alloc;

pub mod native;
pub mod pe;
pub mod elf;
pub mod elf_dynamic;
pub mod elf_thunks;
pub mod meta_sections;
pub mod runtime;

// pe_imports and pe_thunks moved to crate::bmo_gpu::shims (v1.7.9)

use crate::bmo_core::bef::header::BefMagic;
use crate::bmo_core::bef::manifest::{Manifest, Provenance};

/// Formato detectado del binario.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BinaryFormat {
    /// BEF nativo de FastOS.
    BefNative,
    /// PE devorado y traducido a BEF interno.
    PeDevoured,
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

/// Imagen cargada en memoria — representación BEF unificada (cualquier
/// origen acaba aquí).
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

/// Punto de entrada universal — detecta formato y delega al sub-loader.
pub fn load(bytes: &[u8]) -> Result<Image, LoadError> {
    // Initialize runtime symbol table if not done.
    static INIT: core::sync::atomic::AtomicBool = core::sync::atomic::AtomicBool::new(false);
    if !INIT.load(core::sync::atomic::Ordering::Relaxed) {
        runtime::init();
        INIT.store(true, core::sync::atomic::Ordering::Relaxed);
    }

    match BefMagic::detect(bytes) {
        BefMagic::BefNative => native::load(bytes),
        BefMagic::PeWindows => pe::load(bytes),
        BefMagic::ElfUnix   => elf::load(bytes),
        BefMagic::Unknown   => Err(LoadError::UnknownFormat),
    }
}

/// Execute the entry point of a loaded image.
///
/// SAFETY: The image must have a valid entry_point and all relocations
/// resolved. This jumps to Ring 3 (user mode) and does not return.
pub unsafe fn run_entry_point(img: &Image) -> ! {
    let entry = img.entry_point;
    if entry == 0 {
        crate::cabina::fault("bef", "entry point is NULL");
        loop { core::arch::asm!("hlt"); }
    }

    crate::bmo_core::diag::info_u64("bef", "executing entry point", entry);

    // Build a minimal user stack (64 KB).
    let stack_layout = match core::alloc::Layout::from_size_align(65536, 16) {
        Ok(l) => l,
        Err(_) => {
            crate::cabina::fault("bef", "invalid stack layout");
            loop { core::arch::asm!("hlt"); }
        }
    };
    let stack_ptr = alloc::alloc::alloc_zeroed(stack_layout);
    if stack_ptr.is_null() {
        crate::cabina::fault("bef", "failed to allocate user stack");
        loop { core::arch::asm!("hlt"); }
    }
    let stack_top = stack_ptr as u64 + 65536;

    // Switch to user page table if the image has one.
    // For now, use kernel page table (identity-mapped).

    // Jump to Ring 3 via iretq.
    core::arch::asm!(
        "push qword ptr {user_ss}",
        "push {stack_top}",
        "push qword ptr 0x202",
        "push qword ptr {user_cs}",
        "push {entry}",
        "iretq",
        user_ss = const 0x1B_u64,
        user_cs = const 0x23_u64,
        stack_top = in(reg) stack_top,
        entry = in(reg) entry,
        options(noreturn),
    );
}

/// Helper compartido — sintetiza una `MappedSection` vacía.
pub(crate) fn placeholder_section(kind: u8) -> MappedSection {
    MappedSection { kind, virt_addr: 0, size: 0, flags: 0, data_ptr: 0 }
}

pub(crate) fn fake_provenance_image(prov: Provenance) -> Image {
    Image {
        format: match prov {
            Provenance::Native      => BinaryFormat::BefNative,
            Provenance::PeDevoured  => BinaryFormat::PeDevoured,
            Provenance::ElfDevoured => BinaryFormat::ElfDevoured,
        },
        manifest: Manifest::synthetic_for("(stub)", prov),
        entry_point: 0,
        baseess: 0,
        sections: alloc::vec::Vec::new(),
        tls_offset: 0,
        tls_size: 0,
    }
}


