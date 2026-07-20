//! Thread Local Storage (TLS) layout BEF.
//!
//! Reemplaza:
//!   - PE: `IMAGE_TLS_DIRECTORY64` + `__declspec(thread)` + TLS callbacks.
//!   - ELF: `.tdata` + `.tbss` separadas + DTV (Dynamic Thread Vector) + dl_iterate_phdr.
//!
//! Modelo BEF: **un solo blob de template**. Cada thread recibe una copia
//! contigua al crearse. El `fs:0` (en x86-64) apunta al inicio del blob
//! del thread actual. Cero indirecciones DTV.

#![allow(dead_code)]

use crate::bmo_abi::primitives::{bx_u32, bx_u64};
use core::alloc::Layout;

/// Cabecera del template TLS.
#[repr(C, align(8))]
#[derive(Debug, Clone, Copy)]
pub struct TlsTemplate {
    /// Tamaño del template en bytes (datos inicializados).
    pub initialized_size: bx_u32,
    /// Tamaño extra zero-init que sigue al template (estilo .tbss).
    pub zero_size: bx_u32,
    /// Alineación requerida (potencia de 2).
    pub alignment: bx_u32,
    /// Reservado.
    pub _reserved: bx_u32,
    /// Offset dentro de la sección Tls donde empiezan los bytes inicializados.
    pub data_offset: bx_u64,
}
const _: () = assert!(core::mem::size_of::<TlsTemplate>() == 24);

impl TlsTemplate {
    pub const ZERO: Self = Self {
        initialized_size: 0,
        zero_size: 0,
        alignment: 8,
        _reserved: 0,
        data_offset: 0,
    };

    /// Tamaño total que el kernel debe asignar por thread.
    pub const fn total_size(&self) -> u64 {
        self.initialized_size as u64 + self.zero_size as u64
    }
}

/// Setup TLS para un thread nuevo.
///
/// Aloca un buffer de `template.total_size()`, copia los bytes inicializados
/// y deja el resto a cero. Devuelve la dirección que debe ir en `FSBASE`.
pub fn setup_for_thread(template: &TlsTemplate, data: &[u8]) -> Result<u64, &'static str> {
    let total = template.total_size() as usize;
    let align = (template.alignment as usize).max(8);

    if total == 0 {
        return Ok(0);
    }

    // Allocate aligned buffer from kernel heap.
    let layout = Layout::from_size_align(total, align).map_err(|_| "tls: invalid layout")?;
    let raw = unsafe { alloc::alloc::alloc(layout) };
    if raw.is_null() {
        return Err("tls: allocation failed");
    }

    // Copy initialized data (.tdata equivalent).
    let init_len = (template.initialized_size as usize).min(data.len());
    unsafe {
        core::ptr::copy_nonoverlapping(data.as_ptr(), raw, init_len);
        // Zero-fill .tbss portion.
        let zero_start = raw.add(init_len);
        let zero_len = total.saturating_sub(init_len);
        core::ptr::write_bytes(zero_start, 0, zero_len);
    }

    // FS base points to the start of the TLS block.
    // The compiler emits variable offsets relative to FS:0.
    let fs_base = raw as u64;

    // Write IA32_FS_BASE MSR (x86-64) for the current thread.
    #[cfg(target_arch = "x86_64")]
    unsafe {
        core::arch::asm!(
            "wrmsr",
            in("ecx") 0xC000_0100u32, // IA32_FS_BASE
            in("eax") (fs_base as u32),
            in("edx") ((fs_base >> 32) as u32),
            options(nostack),
        );
    }

    Ok(fs_base)
}

/// Teardown TLS for a thread — deallocates the TLS buffer.
///
/// SAFETY: `fs_base` must be a valid TLS buffer previously allocated
/// by `setup_for_thread`, and no other thread must reference it.
pub unsafe fn teardown_for_thread(fs_base: u64, template: &TlsTemplate) {
    if fs_base == 0 || template.total_size() == 0 {
        return;
    }

    let total = template.total_size() as usize;
    let align = (template.alignment as usize).max(8);
    let ptr = fs_base as *mut u8;
    let layout = core::alloc::Layout::from_size_align(total, align);
    if let Ok(layout) = layout {
        alloc::alloc::dealloc(ptr, layout);
    }
}
