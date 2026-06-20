//! Tabla central de intrínsecos: `keyword → bytes precisos`.
//!
//! Cada entrada documenta:
//!   - el keyword fuente BMO (token enum)
//!   - los bytes exactos x86-64 emitidos
//!   - intención semántica (qué problema resuelve)

use crate::bmo_gpu::{BxError, BxResult};
use super::super::lexer::TokenKind;
use super::super::emit::Emitter;

#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IntrinsicId {
    Nop      = 0,
    Pausa    = 1,
    Int3     = 2,
    Hlt      = 3,
    Cli      = 4,
    Sti      = 5,
    Rdtsc    = 6,
    Cpuid    = 7,
    Lfence   = 8,
    Mfence   = 9,
    Sfence   = 10,
    Syscall  = 11,
}

impl IntrinsicId {
    pub const fn from_token(t: TokenKind) -> Option<Self> {
        Some(match t {
            TokenKind::KwNop     => Self::Nop,
            TokenKind::KwPausa   => Self::Pausa,
            TokenKind::KwInt3    => Self::Int3,
            TokenKind::KwHlt     => Self::Hlt,
            TokenKind::KwCli     => Self::Cli,
            TokenKind::KwSti     => Self::Sti,
            TokenKind::KwRdtsc   => Self::Rdtsc,
            TokenKind::KwCpuid   => Self::Cpuid,
            TokenKind::KwLfence  => Self::Lfence,
            TokenKind::KwMfence  => Self::Mfence,
            TokenKind::KwSfence  => Self::Sfence,
            TokenKind::KwSyscall => Self::Syscall,
            _ => return None,
        })
    }
}

/// Devuelve los bytes exactos que produce cada intrínseco.
pub const fn bytes_for(id: IntrinsicId) -> &'static [u8] {
    match id {
        IntrinsicId::Nop     => &[0x90],
        IntrinsicId::Pausa   => &[0xF3, 0x90],
        IntrinsicId::Int3    => &[0xCC],
        IntrinsicId::Hlt     => &[0xF4],
        IntrinsicId::Cli     => &[0xFA],
        IntrinsicId::Sti     => &[0xFB],
        IntrinsicId::Rdtsc   => &[0x0F, 0x31],
        IntrinsicId::Cpuid   => &[0x0F, 0xA2],
        IntrinsicId::Lfence  => &[0x0F, 0xAE, 0xE8],
        IntrinsicId::Mfence  => &[0x0F, 0xAE, 0xF0],
        IntrinsicId::Sfence  => &[0x0F, 0xAE, 0xF8],
        IntrinsicId::Syscall => &[0x0F, 0x05],
    }
}

/// Emite el intrínseco al `Emitter`. Operativo, no stub.
pub fn emit_intrinsic(emit: &mut Emitter, id: IntrinsicId) -> BxResult<()> {
    emit.emit_raw(bytes_for(id));
    Ok(())
}

/// Prefijo LOCK (0xF0) — usado por el bloque `atomico { ... }`.
pub const LOCK_PREFIX: u8 = 0xF0;

/// Prefijo REP (0xF3) — usado por `repetir` con string ops.
pub const REP_PREFIX: u8 = 0xF3;

/// Devuelve los bytes de PAD (NOPs largos) para `align N`.
///
/// Usa los "multi-byte NOPs" recomendados por Intel para no ejecutar
/// múltiples 0x90 (más eficiente en frontend del decoder).
pub fn align_nops(n_bytes: u32) -> BxResult<&'static [u8]> {
    Ok(match n_bytes {
        0  => &[],
        1  => &[0x90],
        2  => &[0x66, 0x90],
        3  => &[0x0F, 0x1F, 0x00],
        4  => &[0x0F, 0x1F, 0x40, 0x00],
        5  => &[0x0F, 0x1F, 0x44, 0x00, 0x00],
        6  => &[0x66, 0x0F, 0x1F, 0x44, 0x00, 0x00],
        7  => &[0x0F, 0x1F, 0x80, 0x00, 0x00, 0x00, 0x00],
        8  => &[0x0F, 0x1F, 0x84, 0x00, 0x00, 0x00, 0x00, 0x00],
        9  => &[0x66, 0x0F, 0x1F, 0x84, 0x00, 0x00, 0x00, 0x00, 0x00],
        _  => return Err(BxError::Unsupported), // > 9 → emisor itera
    })
}
