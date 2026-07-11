//! BMO/BMO v1.8.8
//!
//! Desarrolado por Salazar.
//!
//! `bmo_core::gateway` — Unica puerta entre Ring 0 y BMO Core.
//!
//! v1.8.8: este modulo centraliza TODOS los syscalls de Ring 3.
//! Antes, `ring0::arch::syscall` llamaba directamente a
//! `bmo_api::dispatch_syscall`. Ahora:
//!
//! ```text
//! Ring 3 app
//!     |  syscall (mov rax, 0x100; syscall)
//!     v
//! ring0::arch::syscall (lstar)
//!     |  1) Captura contexto (PID, TID, CR3)
//!     |  2) Llama gateway::enter
//!     |  3) Retorna el resultado en rax
//!     v
//! bmo_core::gateway::enter   <- ESTE MODULO
//!     |  1) Valida con ByteDefender
//!     |  2) Emite evento a Cabina
//!     |  3) Llama bmo_api::dispatch_syscall
//!     |  4) Emite resultado
//!     |  5) Retorna el valor
//!     v
//! return to ring0::arch::syscall
//!     |  iretq -> Ring 3
//!     v
//! Ring 3 app continues
//! ```

#![allow(dead_code)]

use crate::bmo_abi::syscalls;
use core::sync::atomic::{AtomicU64, Ordering};

/// Version del gateway. Incrementar cuando cambia el contrato.
pub const GATEWAY_VERSION: (u8, u8) = (1, 0);

/// Estadisticas del gateway (acumuladas desde boot).
static TOTAL_SYSCALLS: AtomicU64 = AtomicU64::new(0);
static ALLOWED_SYSCALLS: AtomicU64 = AtomicU64::new(0);
static DENIED_SYSCALLS: AtomicU64 = AtomicU64::new(0);
static UNKNOWN_SYSCALLS: AtomicU64 = AtomicU64::new(0);

/// Inicializa el gateway: registra el handler en el kernel y notifica a Cabina.
pub fn init() {
    // Registrar este gateway como el dispatch de syscalls 0x100-0x1FF
    unsafe { bmo_register_gateway(enter); }
    crate::cabina::info("gateway", "gateway v1.0 online - single door to BMO Core");
}

extern "C" {
    fn bmo_register_gateway(f: unsafe extern "C" fn(u16, u64, u64, u64, u64, u64, u64) -> u64);
}

/// Punto de entrada UNICO para syscalls desde Ring 3.
///
/// Esta funcion se llama desde `ring0::arch::syscall` despues de
/// capturar el contexto del proceso. Es la **unica funcion** que Ring 0
/// puede llamar para delegar un syscall a BMO Core.
pub extern "C" fn enter(nr: u16, a0: u64, a1: u64, a2: u64, a3: u64, a4: u64, a5: u64) -> u64 {
    TOTAL_SYSCALLS.fetch_add(1, Ordering::Relaxed);

    // 1. Validar rango BMO ABI
    if nr < 0x100 || nr > 0x1FF {
        UNKNOWN_SYSCALLS.fetch_add(1, Ordering::Relaxed);
        crate::cabina::warn_u64("gateway", "syscall out of range", nr as u64);
        return crate::bmo_abi::error_code::BmoErrorCode::InvalidArgument as u64;
    }

    // 2. ByteDefender: valida capabilities del proceso actual
    if !defense_allows(nr) {
        DENIED_SYSCALLS.fetch_add(1, Ordering::Relaxed);
        crate::cabina::fault_u64("gateway", "syscall denied by ByteDefender", nr as u64);
        return crate::bmo_abi::error_code::BmoErrorCode::PermissionDenied as u64;
    }

    // 3. Cabina: registra el syscall entrante
    let name = syscall_name(nr);
    crate::cabina::trace_u64("gateway", &name, nr as u64);

    // 4. BMO API: ejecuta el syscall real
    let result = crate::bmo_api::dispatch_syscall(nr, a0, a1, a2, a3, a4, a5);

    // 5. Cabina: registra el resultado
    if result == 0 {
        ALLOWED_SYSCALLS.fetch_add(1, Ordering::Relaxed);
    } else if is_fatal_error(result) {
        crate::cabina::fault_u64("gateway", "syscall returned fatal", result);
    }

    result
}

// --- Helpers ---

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

/// Es un error fatal (el proceso deberia morir)?
fn is_fatal_error(code: u64) -> bool {
    use crate::bmo_abi::error_code::BmoErrorCode;
    matches!(
        code as u16,
        x if x == BmoErrorCode::InvalidHandle as u16
            || x == BmoErrorCode::OutOfMemory as u16
    )
}

// --- Estadisticas (read-only) ---

pub fn total() -> u64 { TOTAL_SYSCALLS.load(Ordering::Relaxed) }
pub fn allowed() -> u64 { ALLOWED_SYSCALLS.load(Ordering::Relaxed) }
pub fn denied() -> u64 { DENIED_SYSCALLS.load(Ordering::Relaxed) }
pub fn unknown() -> u64 { UNKNOWN_SYSCALLS.load(Ordering::Relaxed) }

pub mod tests;

/// Observa el lanzamiento de una app de Ring 3.
pub fn observe_launch(name: &str, format: crate::bef::parsers::BinaryFormat) {
    use crate::bef::parsers::BinaryFormat;
    let fmt = match format {
        BinaryFormat::BefNative => "BEF",
        BinaryFormat::ElfDevoured => "ELF-devoured",
    };
    crate::cabina::info("gateway", &alloc::format!("launch: {} (format={})", name, fmt));
}
