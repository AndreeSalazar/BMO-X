//! `BmoStatus` — el "return value" universal del BMO ABI.
//!
//! 16 bytes empacados:
//!   - `code`  (4 B): 0 = OK; >0 = `BxError as u32`
//!   - `flags` (4 B): partial, retry, truncated, etc.
//!   - `value` (8 B): handle, contador, lo que aplique
//!
//! Cabe íntegro en `RAX:RDX`, sin tocar memoria. Reemplaza `HRESULT` y la
//! pareja "código + GetLastError + valor en out param" del C/Win32.

use crate::barex::abi::primitives::{bx_u32, bx_u64};

#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BmoStatus {
    pub code: bx_u32,
    pub flags: bx_u32,
    pub value: bx_u64,
}

impl BmoStatus {
    pub const OK: Self = Self { code: 0, flags: 0, value: 0 };

    #[inline(always)]
    pub const fn ok_value(v: bx_u64) -> Self {
        Self { code: 0, flags: 0, value: v }
    }

    #[inline(always)]
    pub const fn err(code: bx_u32) -> Self {
        Self { code, flags: 0, value: 0 }
    }

    #[inline(always)]
    pub const fn err_with_flags(code: bx_u32, flags: bx_u32) -> Self {
        Self { code, flags, value: 0 }
    }

    #[inline(always)]
    pub const fn is_ok(&self) -> bool { self.code == 0 }

    #[inline(always)]
    pub const fn is_err(&self) -> bool { self.code != 0 }

    #[inline(always)]
    pub const fn has_flag(&self, flag: bx_u32) -> bool {
        (self.flags & flag) != 0
    }
}

bitflags::bitflags! {
    /// Flags auxiliares de `BmoStatus.flags`.
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub struct StatusFlags: bx_u32 {
        /// Operación completada parcialmente (`value` indica cuánto se hizo).
        const PARTIAL    = 1 << 0;
        /// La operación es reintentable (errno-like EAGAIN).
        const RETRY      = 1 << 1;
        /// El buffer de salida fue truncado.
        const TRUNCATED  = 1 << 2;
        /// La operación se encoló (async) y se reportará por CQ.
        const QUEUED     = 1 << 3;
        /// Se requiere descomposición de privilegios (capability faltante).
        const NEEDS_CAP  = 1 << 4;
        /// El valor devuelto es estimado, no exacto (ej. RTT estadístico).
        const ESTIMATED  = 1 << 5;
    }
}
