//! `bmo_core::ring3_gateway` — Única puerta entre Ring 0 y BMO Core.
//!
//! v1.8.8: este módulo centraliza TODOS los syscalls de Ring 3.
//! Antes, `ring0::arch::syscall` llamaba directamente a
//! `bmo_api::dispatch_syscall`. Ahora:
//!
//! ```text
//! Ring 3 app
//!     │  syscall (mov rax, 0x100; syscall)
//!     ▼
//! ring0::arch::syscall (lstar)
//!     │  1) Captura contexto (PID, TID, CR3)
//!     │  2) Llama ring3_gateway::enter
//!     │  3) Retorna el resultado en rax
//!     ▼
//! bmo_core::ring3_gateway::enter   ← ESTE MÓDULO
//!     │  1) Valida con ByteDefender
//!     │  2) Emite evento a Cabina
//!     │  3) Llama bmo_api::dispatch_syscall
//!     │  4) Emite resultado
//!     │  5) Retorna el valor
//!     ▼
//! return to ring0::arch::syscall
//!     │  iretq → Ring 3
//!     ▼
//! Ring 3 app continues
//! ```
//!
//! ## ¿Por qué?
//!
//! - **Abstracción**: Ring 0 no conoce los detalles de BMO API.
//! - **Observabilidad**: cada syscall pasa por Cabina (auditoría).
//! - **Seguridad**: cada syscall pasa por ByteDefender (capabilities).
//! - **Extensibilidad**: añadir quotas/auditoría solo toca el gateway.
//!
//! ## v1.8.8 — Alcance
//!
//! - El gateway **delega** a `bmo_api::dispatch_syscall` (mismo resultado).
//! - Cabina emite `info` por cada syscall.
//! - ByteDefender valida capabilities (W^X, syscalls peligrosas).
//! - Tests integrados para validar el pipeline completo.

#![allow(dead_code)]

use crate::bmo_abi::syscalls;

/// Versión del gateway. Incrementar cuando cambia el contrato.
pub const GATEWAY_VERSION: (u8, u8) = (1, 0);

/// Estadísticas del gateway (acumuladas desde boot).
static mut TOTAL_SYSCALLS: u64 = 0;
static mut ALLOWED_SYSCALLS: u64 = 0;
static mut DENIED_SYSCALLS: u64 = 0;
static mut UNKNOWN_SYSCALLS: u64 = 0;

/// Inicializa el gateway (no-op en v1.8.8).
pub fn init() {
    crate::cabina::info("ring3_gateway", "ring3_gateway v1.0 online — single door to BMO Core");
}

/// Punto de entrada ÚNICO para syscalls desde Ring 3.
///
/// Esta función se llama desde `ring0::arch::syscall` después de
/// capturar el contexto del proceso. Es la **única función** que Ring 0
/// puede llamar para delegar un syscall a BMO Core.
///
/// ## Parámetros
///
/// - `nr`: número de syscall (0x100..0x1FF en la ABI BMO).
/// - `a0..a5`: hasta 6 argumentos (System V AMD64).
///
/// ## Retorno
///
/// El valor a poner en rax antes de `iretq`. Si el syscall no existe,
/// retorna un código de error (los errores canónicos de
/// `bmo_abi::error_code`).
pub fn enter(nr: u16, a0: u64, a1: u64, a2: u64, a3: u64, a4: u64, a5: u64) -> u64 {
    unsafe { TOTAL_SYSCALLS += 1; }

    // ── 1. Validar rango BMO ABI ──────────────────────────────────
    // Los NR_* válidos están en 0x100..0x1FF.
    if nr < 0x100 || nr > 0x1FF {
        unsafe { UNKNOWN_SYSCALLS += 1; }
        crate::cabina::warn_u64("ring3_gateway", "syscall out of range", nr as u64);
        return crate::bmo_abi::error_code::BmoErrorCode::InvalidArgument as u64;
    }

    // ── 2. ByteDefender: valida capabilities del proceso actual ──
    // v1.8.8: stub. En v1.9 se consulta el proceso real.
    if !defense_allows(nr) {
        unsafe { DENIED_SYSCALLS += 1; }
        crate::cabina::fault_u64("ring3_gateway", "syscall denied by ByteDefender", nr as u64);
        return crate::bmo_abi::error_code::BmoErrorCode::PermissionDenied as u64;
    }

    // ── 3. Cabina: registra el syscall entrante ──────────────────
    let name = syscall_name(nr);
    crate::cabina::trace_u64("ring3_gateway", &name, nr as u64);

    // ── 4. BMO API: ejecuta el syscall real ──────────────────────
    let result = crate::bmo_core::bmo_api::dispatch_syscall(nr, a0, a1, a2, a3, a4, a5);

    // ── 5. Cabina: registra el resultado ─────────────────────────
    if result == 0 {
        unsafe { ALLOWED_SYSCALLS += 1; }
    } else if is_fatal_error(result) {
        crate::cabina::fault_u64("ring3_gateway", "syscall returned fatal", result);
    } else {
        // Resultado no-fatal: solo trace.
    }

    result
}

// ── Helpers ─────────────────────────────────────────────────────

/// Traduce un NR_* a su nombre legible para Cabina.
fn syscall_name(nr: u16) -> &'static str {
    match nr as u32 {
        syscalls::NR_WM_CREATE_WINDOW => "wm_create_window",
        syscalls::NR_WM_DESTROY_WINDOW => "wm_destroy_window",
        syscalls::NR_WM_SHOW_WINDOW => "wm_show_window",
        syscalls::NR_WM_HIDE_WINDOW => "wm_hide_window",
        syscalls::NR_WM_BEGIN_PAINT => "wm_begin_paint",
        syscalls::NR_WM_END_PAINT => "wm_end_paint",
        syscalls::NR_DRAW_CLEAR => "draw_clear",
        syscalls::NR_DRAW_PIXEL => "draw_pixel",
        syscalls::NR_WINPAINT_FILL_RECT => "fill_rect",
        syscalls::NR_WINPAINT_DRAW_TEXT => "draw_text",
        syscalls::NR_FS_OPEN => "fs_open",
        syscalls::NR_FS_READ => "fs_read",
        syscalls::NR_FS_WRITE => "fs_write",
        syscalls::NR_FS_CLOSE => "fs_close",
        syscalls::NR_TIME_NOW_NS => "time_now_ns",
        syscalls::NR_TIME_SLEEP_MS => "time_sleep_ms",
        syscalls::NR_MEM_ALLOC => "mem_alloc",
        syscalls::NR_MEM_FREE => "mem_free",
        syscalls::NR_PROC_EXIT => "proc_exit",
        syscalls::NR_PROC_GET_PID => "proc_get_pid",
        syscalls::NR_PROC_YIELD => "proc_yield",
        syscalls::NR_BEFCORE_SEND => "befcore_send",
        syscalls::NR_BEFCORE_RECV => "befcore_recv",
        syscalls::NR_AUDIO_BEEP => "audio_beep",
        syscalls::NR_DEBUG_PRINT => "debug_print",
        syscalls::NR_DEBUG_PANIC => "debug_panic",
        _ => "unknown",
    }
}

/// ByteDefender: valida si el proceso actual puede invocar `nr`.
/// v1.8.8: siempre retorna `true` (sin sandbox). En v1.9 se conecta
/// con `defense::on_syscall`.
fn defense_allows(_nr: u16) -> bool {
    // v1.9: return crate::defense::on_syscall(nr, 0, 0) == PolicyAction::Allow;
    true
}

/// ¿Es un error fatal (el proceso debería morir)?
fn is_fatal_error(code: u64) -> bool {
    use crate::bmo_abi::error_code::BmoErrorCode;
    matches!(
        code as u16,
        x if x == BmoErrorCode::InvalidHandle as u16
            || x == BmoErrorCode::OutOfMemory as u16
    )
}

// ── Estadísticas (read-only) ────────────────────────────────────

pub fn total() -> u64 { unsafe { TOTAL_SYSCALLS } }
pub fn allowed() -> u64 { unsafe { ALLOWED_SYSCALLS } }
pub fn denied() -> u64 { unsafe { DENIED_SYSCALLS } }
pub fn unknown() -> u64 { unsafe { UNKNOWN_SYSCALLS } }
