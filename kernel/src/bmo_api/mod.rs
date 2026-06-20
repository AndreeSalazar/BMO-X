//! BMO API v2.0 — windowing API para FastOS/BMO.
//!
//! Único módulo del subsistema. v2.0 reemplaza completamente al v1.x
//! legacy. Inspirado en Win32 USER32, X11, Wayland y Cocoa, pero
//! profundamente modular para que cada subsistema viva aislado.
//!
//! Syscall ABI: 0x100..0x1FF (256 números), convención System V
//! AMD64.  Ver `docs/BMO_API_SPEC.md` para el spec completo.
//!
//! Módulos:
//!   v2::handle         : Handle table con generation counter
//!   v2::window         : Windows + classes + Z-order + parent/child tree
//!   v2::message        : bmo_msg + BMO_MSG_* enum
//!   v2::queue          : SPSC ring per-thread (64 mensajes)
//!   v2::event          : MouseMove dedup, paint-region coalesce
//!   v2::surface        : Offscreen surfaces (1:1 ventana↔surface)
//!   v2::draw           : DC + primitives
//!   v2::class          : Class table + default wnd_proc
//!   v2::wm             : Z-order, focus, drag/resize, snap, modal
//!   v2::timer          : Timer wheel (1 ms)
//!   v2::input          : PS/2 + USB HID → events
//!   v2::cursor         : 16 builtin cursor sprites
//!   v2::paint_compositor : Dirty-region tracking + blit
//!   v2::syscall        : Dispatcher 0x100..0x1FF
//!
//! Re-exports de alto nivel para que el resto del kernel no tenga que
//! importar submódulos.

#![allow(dead_code)]
#![allow(static_mut_refs)]

pub mod v2;

// ── Re-exports principales (lo que el resto del kernel usa) ───────
pub use v2::BmoState;
pub use v2::wm;
#[allow(unused_imports)]
pub use v2::paint_compositor;
#[allow(unused_imports)]
pub use v2::cursor;

/// Estado global del subsistema. Todos los accesos al window manager
/// pasan por este singleton.
#[inline]
pub fn state() -> &'static mut BmoState {
    v2::state()
}

/// Inicializa el subsistema. Llamar desde `boot::phase5` después del
/// scheduler y antes de entrar al desktop.
pub fn init() {
    v2::init()
}

/// Tick periódico llamado desde el scheduler. Procesa timers y el
/// compositor de pintado.
pub fn tick() {
    v2::tick()
}

/// Llamado por `arch::syscall_entry` cuando el nr está en 0x100..0x1FF.
#[inline]
pub fn dispatch_syscall(nr: u16, a0: u64, a1: u64, a2: u64, a3: u64, a4: u64, a5: u64) -> u64 {
    v2::dispatch_syscall(nr, a0, a1, a2, a3, a4, a5)
}
