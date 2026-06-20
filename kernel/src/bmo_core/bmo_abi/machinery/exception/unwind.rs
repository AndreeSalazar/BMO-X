//! Contexto de unwinding y razones.

use crate::bmo_core::bmo_abi::primitives::bx_u64;

#[repr(u32)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UnwindReason {
    /// Excepción lanzada en este frame.
    Throw       = 0,
    /// Llegó desde un frame inferior (propagación).
    Propagate   = 1,
    /// Stack walk informativo (no destructivo).
    Inspect     = 2,
    /// `longjmp`-style — saltar sin destruir intermedios.
    Jump        = 3,
    /// Cleanup forzado (proceso muriendo).
    ForceClean  = 4,
}

#[repr(u32)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UnwindAction {
    /// Continuar buscando handler.
    Continue   = 0,
    /// Handler encontrado, parar y ejecutar.
    Caught     = 1,
    /// Cleanup necesario (drop / finally).
    Cleanup    = 2,
    /// Aborta — no hay handler en toda la pila.
    Abort      = 3,
}

#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct UnwindContext {
    pub reason: UnwindReason,
    /// RIP del frame que se está examinando.
    pub rip: bx_u64,
    /// RSP del frame.
    pub rsp: bx_u64,
    /// RBP del frame (frame pointer).
    pub rbp: bx_u64,
    /// Padding para futuros registros.
    pub _reserved: [bx_u64; 4],
}

impl UnwindContext {
    pub const fn empty() -> Self {
        Self {
            reason: UnwindReason::Inspect,
            rip: 0, rsp: 0, rbp: 0,
            _reserved: [0; 4],
        }
    }
}
