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

use crate::barex::abi::primitives::{bx_u32, bx_u64};

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
pub fn setup_for_thread(_template: &TlsTemplate, _data: &[u8]) -> Result<u64, &'static str> {
    // TODO: requiere allocator del kernel + WRMSR a IA32_FS_BASE.
    Err("tls::setup_for_thread no implementado todavía")
}
