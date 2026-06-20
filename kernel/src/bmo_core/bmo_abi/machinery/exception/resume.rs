//! Resumable exceptions — algo que ningún C ABI tiene.
//!
//! Permite a un handler decidir "vuelve al lugar donde se lanzó y continúa
//! con este valor". Útil para condition systems estilo Common Lisp,
//! resumable warnings de Python, effect systems de OCaml/Koka.

use crate::bmo_core::barex::{BxError, BxResult};
use crate::bmo_core::bmo_abi::primitives::bx_u64;

#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct ResumeToken {
    /// Dirección de retorno al sitio del throw.
    pub resume_rip: bx_u64,
    /// Stack pointer a restaurar.
    pub resume_rsp: bx_u64,
    /// Valor a depositar en RAX al volver.
    pub return_value: bx_u64,
}

impl ResumeToken {
    pub const NULL: Self = Self {
        resume_rip: 0,
        resume_rsp: 0,
        return_value: 0,
    };

    #[inline(always)]
    pub const fn is_null(&self) -> bool { self.resume_rip == 0 }
}

/// Resume de la excepción. Stub — necesita soporte arch-specific.
pub fn resume(_token: ResumeToken) -> BxResult<()> {
    Err(BxError::NotImplemented)
}
