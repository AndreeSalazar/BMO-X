//! `BmoStatus` -- el "return value" universal del BMO ABI.
//!
//! 16 bytes empacados:
//!   - `code`  (4 B): 0 = OK; >0 = `BxError as u32`
//!   - `flags` (4 B): partial, retry, truncated, etc.
//!   - `value` (8 B): handle, contador, lo que aplique
//!
//! Cabe integro en `RAX:RDX`, sin tocar memoria. Reemplaza `HRESULT` y la
//! pareja "codigo + GetLastError + valor en out param" del C/Win32.
//!
//! -- EL SEMAFORO (L6g) y las dos preguntas de antes (L6e, L6f) --------
//!
//! Que cuesta que falle, por que falla ESTA pieza, y que arrastro si la
//! toco. La ley esta en `META-KERNEL_HARD.md`.
//!
//! [carril]  ROJO         `BmoStatus` es lo que VIAJA en rax/rdx al volver de
//!                        la puerta
//! [cuesta]  PUERTA       `into_registers` decide que ve Ring 3 al volver:
//!                        cambiarlo cambia todo retorno
//! [riesgo]  SILENCIO     `0` es lo unico que significa exito, y los 32 bits
//!                        altos son banderas. Confundir el campo da un `ok`
//!                        que no lo era

use crate::bmo_abi::primitives::{bx_u32, bx_u64};

#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BmoStatus {
    pub code: bx_u32,
    pub flags: bx_u32,
    pub value: bx_u64,
}
const _: () = assert!(core::mem::size_of::<BmoStatus>() == 16);

impl BmoStatus {
    pub const OK: Self = Self {
        code: 0,
        flags: 0,
        value: 0,
    };

    #[inline(always)]
    pub const fn ok_value(v: bx_u64) -> Self {
        Self {
            code: 0,
            flags: 0,
            value: v,
        }
    }

    #[inline(always)]
    pub const fn err(code: bx_u32) -> Self {
        Self {
            code,
            flags: 0,
            value: 0,
        }
    }

    #[inline(always)]
    pub const fn err_with_flags(code: bx_u32, flags: bx_u32) -> Self {
        Self {
            code,
            flags,
            value: 0,
        }
    }

    #[inline(always)]
    pub const fn is_ok(&self) -> bool {
        self.code == 0
    }

    #[inline(always)]
    pub const fn is_err(&self) -> bool {
        self.code != 0
    }

    #[inline(always)]
    pub const fn has_flag(&self, flag: bx_u32) -> bool {
        (self.flags & flag) != 0
    }

    /// Pack the fixed BMO x86-64 register representation:
    /// `RAX[31:0] = code`, `RAX[63:32] = flags`, `RDX = value`.
    #[inline(always)]
    pub const fn into_registers(self) -> (bx_u64, bx_u64) {
        (
            (self.code as bx_u64) | ((self.flags as bx_u64) << 32),
            self.value,
        )
    }

    /// Decode the fixed BMO x86-64 register representation.
    #[inline(always)]
    pub const fn from_registers(rax: bx_u64, rdx: bx_u64) -> Self {
        Self {
            code: rax as bx_u32,
            flags: (rax >> 32) as bx_u32,
            value: rdx,
        }
    }
}

bitflags::bitflags! {
    /// Flags auxiliares de `BmoStatus.flags`.
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub struct StatusFlags: bx_u32 {
        /// Operacion completada parcialmente (`value` indica cuanto se hizo).
        const PARTIAL    = 1 << 0;
        /// La operacion es reintentable (errno-like EAGAIN).
        const RETRY      = 1 << 1;
        /// El buffer de salida fue truncado.
        const TRUNCATED  = 1 << 2;
        /// La operacion se encolo (async) y se reportara por CQ.
        const QUEUED     = 1 << 3;
        /// Se requiere descomposicion de privilegios (capability faltante).
        const NEEDS_CAP  = 1 << 4;
        /// El valor devuelto es estimado, no exacto (ej. RTT estadistico).
        const ESTIMATED  = 1 << 5;
    }
}
