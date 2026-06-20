//! v1.7.4 — Ring 3 coordinator.
//!
//! Coordina la inicialización del subsistema de userland. En esta
//! versión (v1.7.4) Ring 3 está en preparación estructural — la
//! wnd_proc real a Ring 3 vía iretq (syscall 0x198 BMO_DISPATCH_RETURN)
//! está documentada en `docs/BMO_API_V2_SPEC.md` §6.2 pero su
//! implementación completa (per-thread kernel stack + trampoline +
//! reentrancy) queda para v2.1.
//!
//! Estado actual:
//!   * 256 syscalls 0x100..0x1FF definidos en bmo_core::bmo_api
//!   * sys_send_message y sys_dispatch_message caen en default_wnd_proc
//!     cuando wnd_proc=0 (sin wnd_proc real todavía)
//!   * El BMO API spec completo está en docs/BMO_API_V2_SPEC.md
//!
//! Punto de entrada: todavía no se llama desde ring_0 porque no hay
//! un loader dinámico de apps. Cuando esté listo, `ring0::ring_0::main`
//! invocará `ring3::ring_3::enter()` después de `bmo_core::coord::enter`
//! devuelva el control — pero en v1.7.4 BMO Core no retorna.

/// Inicializa el subsistema Ring 3. v1.7.4: no-op.
pub fn init() {
    // v2.0: cargar el ELF loader, registrar los BEFs built-in,
    // validar el per-thread kernel stack size, y registrar la
    // wnd_proc trampoline en el rango 0x100..0x1FF.
}

/// Punto de entrada de una app Ring 3. v1.7.4: stub.
///
/// En v2.0, esta función será llamada por el syscall 0x124
/// `bmo_dispatch_message` con un iretq al wnd_proc del Ring 3.
/// En v1.7.4 sólo se ejecuta el default_wnd_proc.
pub fn enter_wnd_proc(_hwnd: u32, _msg: u16, _wparam: u64, _lparam: u64) -> u64 {
    // v2.0: iretq al wnd_proc Ring 3 + esperar syscall 0x198
    // BMO_DISPATCH_RETURN.
    0
}
