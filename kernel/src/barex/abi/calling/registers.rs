//! Especificación de registros y stack del BMO ABI.
//!
//! Estas constantes documentan la convención. La materialización real ocurre
//! en el codegen del compilador (rustc backend custom o flags) y en los
//! trampolines de `compat::thunks`.

use crate::barex::abi::primitives::bx_usize;

// ─── Stack ─────────────────────────────────────────────────────────────

/// Alineación del stack al ejecutar un `call` (cache line completa Zen 3).
pub const STACK_ALIGNMENT: bx_usize = 64;

/// "Shadow space" para los args registrados — siempre 0 en BMO ABI.
/// (Compárese: 32 bytes en Microsoft x64.)
pub const SHADOW_SPACE: bx_usize = 0;

/// Red zone bajo RSP usable por el callee sin reservar.
pub const RED_ZONE_SIZE: bx_usize = 256;

// ─── Registros de paso de args ─────────────────────────────────────────

/// Cantidad de GPRs disponibles para args int (vs 4 en MS x64, 6 en SysV).
pub const ARG_GPRS: usize = 7;

/// Orden de uso de GPRs para args (representado como nombres).
pub const ARG_GPRS_NAMES: [&str; ARG_GPRS] =
    ["RDI", "RSI", "RDX", "R10", "R8", "R9", "RAX_extra"];

/// Cantidad de XMMs para args float/vec.
pub const ARG_XMMS: usize = 8;

pub const ARG_XMMS_NAMES: [&str; ARG_XMMS] =
    ["XMM0","XMM1","XMM2","XMM3","XMM4","XMM5","XMM6","XMM7"];

// ─── Registros de retorno ──────────────────────────────────────────────

/// Pareja de GPRs para retornar valores ≤ 128 bits (`BmoStatus` los usa).
pub const RET_GPRS: [&str; 2] = ["RAX", "RDX"];

/// XMMs para retorno de vectores.
pub const RET_XMMS: [&str; 2] = ["XMM0", "XMM1"];

// ─── Caller-saved (volátiles) ──────────────────────────────────────────
pub const CALLER_SAVED_GPRS: &[&str] = &["RAX", "R10", "R11"];
pub const CALLER_SAVED_XMMS: &[&str] = &["XMM8","XMM9","XMM10","XMM11","XMM12","XMM13","XMM14","XMM15"];

// ─── Callee-saved (preservados) ────────────────────────────────────────
pub const CALLEE_SAVED_GPRS: &[&str] = &["RBX", "RBP", "RSP", "R12", "R13", "R14", "R15"];
/// XMM0..7: also used as args, but función debe preservarlos al spillar.
pub const CALLEE_SAVED_XMMS: &[&str] = &["XMM0","XMM1","XMM2","XMM3","XMM4","XMM5","XMM6","XMM7"];

// ─── Diff vs ABIs heredados ────────────────────────────────────────────

/// Tabla resumen para introspección/debug.
pub const ABI_COMPARISON: &str = "\
| Aspecto              | MS x64        | SysV AMD64    | BMO ABI       |
|----------------------|---------------|---------------|---------------|
| Args int             | 4 GPRs        | 6 GPRs        | 7 GPRs        |
| Shadow space         | 32 B          | 0 B           | 0 B           |
| Stack align          | 16 B          | 16 B          | 64 B          |
| Red zone             | 0 B           | 128 B         | 256 B         |
| Return ints          | RAX           | RAX:RDX       | RAX:RDX       |
";
