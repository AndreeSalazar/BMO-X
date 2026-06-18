//! Trampolines C ABI ↔ BMO ABI.
//!
//! Materialización real de tránsitos entre ABIs. Los tres caminos:
//!
//! 1. **BMO → MS x64** (cuando código BMO llama a DLL Windows): reservar
//!    shadow space, asegurar 16B stack align, transferir args RCX/RDX/R8/R9
//!    ← RDI/RSI/RDX/R10/R8/R9, llamar, restaurar.
//!
//! 2. **BMO → SysV AMD64** (cuando código BMO llama a .so Linux): 16B stack
//!    align, transferir args RDI/RSI/RDX/RCX/R8/R9
//!    ← RDI/RSI/RDX/R10/R8/R9, llamar, restaurar.
//!
//! 3. **MS x64 → BMO** (cuando código Windows llama a función BMO): quitar
//!    shadow space del RSP, transferir args RDI/RSI/RDX/R10/R8/R9
//!    ← RCX/RDX/R8/R9, llamar, restaurar.
//!
//! Cada función thunk toma un puntero al código destino y los args; devuelve
//! el resultado en RAX.
//!
//! ## Seguridad
//!
//! Los thunks manipulan registros y stack directamente. Son `unsafe` por
//! naturaleza. El caller es responsable de:
//!   - Tener stack 16B aligned antes de `call` (SysV) o 32B + 16B (MS).
//!   - No pasar structs mayores a 16 bytes por registro.
//!   - Limpiar el stack después.

use crate::bmo_abi::primitives::bx_usize;

// ─── Constantes de ABI ───────────────────────────────────────────────

/// Tamaño de "shadow space" requerido cuando se llama código MS x64
/// desde código BMO. **Debe reservarse antes del `call`.**
pub const MSX64_SHADOW_SPACE: bx_usize = 32;

/// Stack alignment requerido por SysV antes de `call`.
pub const SYSV_STACK_ALIGNMENT: bx_usize = 16;

/// Stack alignment requerido por MS x64 antes de `call`.
pub const MSX64_STACK_ALIGNMENT: bx_usize = 16;

// ─── Trampolines runtime ──────────────────────────────────────────────

/// Trampolín BMO → MS x64 (llamar DLL Windows desde BMO).
///
/// Args: `target` (función MS), `a0..a3` (4 args GPR según BMO).
///
/// Stack al entrar: caller garantiza 32B align. Trampolín:
///   1. Reserva 32B shadow space (`sub rsp, 32`).
///   2. Mueve 4 args: RCX←RDI, RDX←RSI, R8←RDX, R9←R10.
///   3. Ajusta align a 16B.
///   4. Llama `target`.
///   5. Limpia shadow + align.
///   6. Retorna RAX.
///
/// SAFETY:
/// - `target` debe ser una función MS x64 válida.
/// - Caller garantiza que los 4 args caben en registros (no se aceptan args
///   por stack en este trampolín).
#[unsafe(naked)]
#[unsafe(no_mangle)]
pub extern "C" fn bmo_to_msx64_trampoline(
    target: usize, a0: u64, a1: u64, a2: u64, a3: u64,
) -> u64 {
    core::arch::naked_asm!(
        // target en RCX, a0..a3 en RDX, R8, R9, [stack+0x28] (after ret addr)
        // (R10, R11 son caller-saved, podemos usarlos.)
        "mov rax, rcx",          // rax = target
        "mov rcx, rdx",          // rcx = a0
        "mov rdx, rsi",          // rdx = a1 (RDI was a0, RSI was a1)
        "mov r8,  rdx",          // r8  = a2 (original RDX was a2)
        "mov r9,  r10",          // r9  = a3
        "sub rsp, 48",           // 32 (shadow) + 16 (align padding)
        "and rsp, ~0xF",         // ensure 16B align
        "call rax",
        "lea rsp, [rsp + 48]",   // restore stack
        "ret",
    )
}

/// Trampolín BMO → SysV AMD64 (llamar .so Linux desde BMO).
///
/// Args: `target`, `a0..a3`.
/// Trampolín:
///   1. Ajusta stack a 16B align.
///   2. Mueve args: RDI←RDI, RSI←RSI, RDX←RDX, RCX←R10.
///   3. Llama `target`.
///   4. Retorna RAX.
#[unsafe(naked)]
#[unsafe(no_mangle)]
pub extern "C" fn bmo_to_sysv_trampoline(
    target: usize, a0: u64, a1: u64, a2: u64, a3: u64,
) -> u64 {
    core::arch::naked_asm!(
        "mov rax, rcx",          // rax = target
        // Args are already in RDI/RSI/RDX; just need R10→RCX for 4th arg.
        "mov rcx, r10",          // rcx = a3
        "sub rsp, 8",            // 16B align (caller likely had 8B ret addr)
        "and rsp, ~0xF",
        "call rax",
        "lea rsp, [rsp + 8]",
        "ret",
    )
}

/// Trampolín MS x64 → BMO (función BMO llamada desde DLL Windows).
///
/// Args llegan en: RCX, RDX, R8, R9, [stack] según MS x64.
/// BMO espera: RDI, RSI, RDX, R10, R8, R9.
///
/// Trampolín:
///   1. Pop 32B de shadow del stack del caller.
///   2. Mueve args: RDI←RCX, RSI←RDX, RDX←R8, R10←R9.
///   3. Llama `target`.
///   4. Retorna RAX.
#[unsafe(naked)]
#[unsafe(no_mangle)]
pub extern "C" fn msx64_to_bmo_trampoline(
    target: usize, _a0: u64, _a1: u64, _a2: u64, _a3: u64,
) -> u64 {
    core::arch::naked_asm!(
        "mov rax, rcx",          // rax = target
        "mov rdi, rdx",          // rdi = a0 (came in rcx)
        "mov rsi, r8",           // rsi = a1 (came in rdx)
        "mov rdx, r9",           // rdx = a2 (came in r8)
        "mov r10, [rsp + 0x28]", // r10 = a3 (came in r9, but pushed by call)
        "add rsp, 32",           // skip MS x64 shadow space
        "jmp rax",               // tail-call (saves a ret)
    )
}

/// Trampolín SysV → BMO (función BMO llamada desde .so Linux).
///
/// Args llegan en: RDI, RSI, RDX, RCX, R8, R9 (ya coinciden con BMO).
/// Solo se necesita garantizar 16B stack align.
#[unsafe(naked)]
#[unsafe(no_mangle)]
pub extern "C" fn sysv_to_bmo_trampoline(
    target: usize, _a0: u64, _a1: u64, _a2: u64, _a3: u64,
) -> u64 {
    core::arch::naked_asm!(
        "mov rax, rcx",          // rax = target
        // RDI/RSI/RDX/RCX/R8/R9 ya coinciden.
        "jmp rax",
    )
}

// ─── Wrappers de alto nivel (safe) ────────────────────────────────────

/// Llama a una función MS x64 (`extern "C"` con shadow space) desde BMO.
/// Toma 4 args u64, retorna u64.
#[inline(always)]
pub unsafe fn call_msx64_4(target: usize, a0: u64, a1: u64, a2: u64, a3: u64) -> u64 {
    bmo_to_msx64_trampoline(target, a0, a1, a2, a3)
}

/// Llama a una función SysV desde BMO.
#[inline(always)]
pub unsafe fn call_sysv_4(target: usize, a0: u64, a1: u64, a2: u64, a3: u64) -> u64 {
    bmo_to_sysv_trampoline(target, a0, a1, a2, a3)
}

// ─── Marcadores para tipado en compilación ───────────────────────────

/// Marca conceptual de "esta función debe llamarse con MS x64 ABI".
#[allow(non_camel_case_types)]
pub struct MsX64Marker;

/// Marca conceptual de "esta función debe llamarse con SysV AMD64 ABI".
#[allow(non_camel_case_types)]
pub struct SysVMarker;
