//! v2.0 — Ring 3 coordinator.
//!
//! Coordina la inicialización del subsistema de userland. La wnd_proc
//! Ring 3 se ejecuta vía el scheduler del kernel:
//!
//! 1. El kernel postea mensajes a la cola del proceso owner_tid.
//! 2. El scheduler (round-robin preemptive) ejecuta el proceso Ring 3.
//! 3. El proceso llama a GetMessage (syscall 0x120) → el wnd_proc se ejecuta.
//! 4. El wnd_proc llama a DISPATCH_RETURN (syscall 0x198) → resultado vuelve al kernel.
//!
//! Para ventanas kernel-side (owner_tid == 0), el kernel ejecuta
//! default_wnd_proc directamente sin transición a Ring 3.

#![allow(dead_code)]

use bmo_core::bmo_api::message::BmoMsgKind;

/// Inicializa el subsistema Ring 3.
pub fn init() {
    // Los procesos se crean bajo demanda via allocate_user_process().
    // No hay loader dinámico todavía — las apps son 64 bytes de
    // x86-64 machine code hardcodeado en user_init.rs.
}

/// Llama al wnd_proc de una ventana Ring 3 de forma sincrónica.
///
/// Retorna el resultado de la wnd_proc, o None si no se pudo ejecutar.
/// Nota: En la arquitectura actual, el wnd_proc se ejecuta cuando el
/// scheduler ejecuta el proceso. Esta función es un placeholder para
/// cuando se implemente la llamada síncrona kernel→Ring3.
pub fn enter_wnd_proc(hwnd: u32, msg: u16, wparam: u64, lparam: u64) -> Option<u64> {
    let kind = BmoMsgKind::from_u16(msg);
    let _ = (hwnd, kind, wparam, lparam);
    None
}

/// Verifica si un wnd_proc es kernel-side (0) o Ring 3 (!= 0).
pub fn is_ring3_wnd_proc(wnd_proc: u64) -> bool {
    wnd_proc != 0
}
