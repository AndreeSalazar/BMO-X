//! BMO/BMO v1.8.8
//!
//! Desarrolado por Salazar.
//!
//! `bmo_core::desktop3` â€” Ãšnica puerta entre Ring 0 y BMO Core.
//!
//! v1.8.8: este mÃ³dulo centraliza TODOS los syscalls de Ring 3.
//! Antes, `ring0::arch::syscall` llamaba directamente a
//! `bmo_api::dispatch_syscall`. Ahora:
//!
//! ```text
//! Ring 3 app
//!     â”‚  syscall (mov rax, 0x100; syscall)
//!     â–¼
//! ring0::arch::syscall (lstar)
//!     â”‚  1) Captura contexto (PID, TID, CR3)
//!     â”‚  2) Llama desktop3::enter
//!     â”‚  3) Retorna el resultado en rax
//!     â–¼
//! bmo_core::desktop3::enter   â† ESTE MÃ“DULO
//!     â”‚  1) Valida con ByteDefender
//!     â”‚  2) Emite evento a Cabina
//!     â”‚  3) Llama bmo_api::dispatch_syscall
//!     â”‚  4) Emite resultado
//!     â”‚  5) Retorna el valor
//!     â–¼
//! return to ring0::arch::syscall
//!     â”‚  iretq â†’ Ring 3
//!     â–¼
//! Ring 3 app continues
//! ```
//!
//! ## Â¿Por quÃ©?
//!
//! - **AbstracciÃ³n**: Ring 0 no conoce los detalles de BMO API.
//! - **Observabilidad**: cada syscall pasa por Cabina (auditorÃ­a).
//! - **Seguridad**: cada syscall pasa por ByteDefender (capabilities).
//! - **Extensibilidad**: aÃ±adir quotas/auditorÃ­a solo toca el gateway.
//!
//! ## v1.8.8 â€” Alcance
//!
//! - El gateway **delega** a `bmo_api::dispatch_syscall` (mismo resultado).
//! - Cabina emite `info` por cada syscall.
//! - ByteDefender valida capabilities (W^X, syscalls peligrosas).
//! - Tests integrados para validar el pipeline completo.

#![allow(dead_code)]

use crate::bmo_abi::syscalls;

/// VersiÃ³n del gateway. Incrementar cuando cambia el contrato.
pub const GATEWAY_VERSION: (u8, u8) = (1, 0);

/// EstadÃ­sticas del gateway (acumuladas desde boot).
static mut TOTAL_SYSCALLS: u64 = 0;
static mut ALLOWED_SYSCALLS: u64 = 0;
static mut DENIED_SYSCALLS: u64 = 0;
static mut UNKNOWN_SYSCALLS: u64 = 0;

/// Inicializa el gateway (no-op en v1.8.8).
pub fn init() {
    crate::cabina::info("desktop3", "desktop3 v1.0 online â€” single door to BMO Core");
}

/// Punto de entrada ÃšNICO para syscalls desde Ring 3.
///
/// Esta funciÃ³n se llama desde `ring0::arch::syscall` despuÃ©s de
/// capturar el contexto del proceso. Es la **Ãºnica funciÃ³n** que Ring 0
/// puede llamar para delegar un syscall a BMO Core.
///
/// ## ParÃ¡metros
///
/// - `nr`: nÃºmero de syscall (0x100..0x1FF en la ABI BMO).
/// - `a0..a5`: hasta 6 argumentos (System V AMD64).
///
/// ## Retorno
///
/// El valor a poner en rax antes de `iretq`. Si el syscall no existe,
/// retorna un cÃ³digo de error (los errores canÃ³nicos de
/// `bmo_abi::error_code`).
pub fn enter(nr: u16, a0: u64, a1: u64, a2: u64, a3: u64, a4: u64, a5: u64) -> u64 {
    unsafe { TOTAL_SYSCALLS += 1; }

    // â”€â”€ 1. Validar rango BMO ABI â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€
    // Los NR_* vÃ¡lidos estÃ¡n en 0x100..0x1FF.
    if nr < 0x100 || nr > 0x1FF {
        unsafe { UNKNOWN_SYSCALLS += 1; }
        crate::cabina::warn_u64("desktop3", "syscall out of range", nr as u64);
        return crate::bmo_abi::error_code::BmoErrorCode::InvalidArgument as u64;
    }

    // â”€â”€ 2. ByteDefender: valida capabilities del proceso actual â”€â”€
    // v1.8.8: stub. En v1.9 se consulta el proceso real.
    if !defense_allows(nr) {
        unsafe { DENIED_SYSCALLS += 1; }
        crate::cabina::fault_u64("desktop3", "syscall denied by ByteDefender", nr as u64);
        return crate::bmo_abi::error_code::BmoErrorCode::PermissionDenied as u64;
    }

    // â”€â”€ 3. Cabina: registra el syscall entrante â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€
    let name = syscall_name(nr);
    crate::cabina::trace_u64("desktop3", &name, nr as u64);

    // â”€â”€ 4. BMO API: ejecuta el syscall real â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€
    let result = crate::bmo_api::dispatch_syscall(nr, a0, a1, a2, a3, a4, a5);

    // â”€â”€ 5. Cabina: registra el resultado â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€
    if result == 0 {
        unsafe { ALLOWED_SYSCALLS += 1; }
    } else if is_fatal_error(result) {
        crate::cabina::fault_u64("desktop3", "syscall returned fatal", result);
    } else {
        // Resultado no-fatal: solo trace.
    }

    result
}

// â”€â”€ Helpers â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€

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

/// Â¿Es un error fatal (el proceso deberÃ­a morir)?
fn is_fatal_error(code: u64) -> bool {
    use crate::bmo_abi::error_code::BmoErrorCode;
    matches!(
        code as u16,
        x if x == BmoErrorCode::InvalidHandle as u16
            || x == BmoErrorCode::OutOfMemory as u16
    )
}

// â”€â”€ EstadÃ­sticas (read-only) â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€

pub fn total() -> u64 { unsafe { TOTAL_SYSCALLS } }
pub fn allowed() -> u64 { unsafe { ALLOWED_SYSCALLS } }
pub fn denied() -> u64 { unsafe { DENIED_SYSCALLS } }
pub fn unknown() -> u64 { unsafe { UNKNOWN_SYSCALLS } }

pub mod tests;

/// Observa el lanzamiento de una app de Ring 3.
/// Llamado por `userland::app::run` antes de saltar a Ring 3.
pub fn observe_launch(name: &str, format: crate::bef::parsers::BinaryFormat) {
    use crate::bef::parsers::BinaryFormat;
    let fmt = match format {
        BinaryFormat::BefNative => "BEF",
        BinaryFormat::ElfDevoured => "ELF-devoured",
    };
    crate::cabina::info("desktop3", &alloc::format!("launch: {} (format={})", name, fmt));
}




