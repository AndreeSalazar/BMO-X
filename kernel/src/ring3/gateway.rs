//! Ring 3 Gateway — puente entre syscalls del kernel y bmo_core.
//!
//! El kernel atrapa syscalls 0x00-0xFF con su propia jump table.
//! Los syscalls 0x100-0x1FF (BMO ABI) se delegan aquí.
//! bmo_core registra su gateway handler vía `bmo_register_gateway`.
//!
//! # Uso
//!
//! 1. bmo_core::init() → llama `bmo_register_gateway(gateway::enter)`
//! 2. kernel::arch::syscall → si nr ≥ 0x100, llama a `dispatch()`

use core::sync::atomic::{AtomicPtr, Ordering};

/// Tipo del handler de gateway: (nr, a0..a5) → resultado
type GatewayFn = unsafe extern "C" fn(u16, u64, u64, u64, u64, u64, u64) -> u64;

/// Puntero al gateway de bmo_core (null hasta que se registre)
static GATEWAY: AtomicPtr<()> = AtomicPtr::new(core::ptr::null_mut());

/// Registra el handler de gateway (llamado por bmo_core durante init).
#[no_mangle]
pub extern "C" fn bmo_register_gateway(f: unsafe extern "C" fn(u16, u64, u64, u64, u64, u64, u64) -> u64) {
    GATEWAY.store(f as *mut (), Ordering::Release);
}

/// Despacha un syscall BMO ABI (0x100-0x1FF) al gateway registrado.
/// Retorna u64::MAX si no hay gateway registrado.
pub fn dispatch(nr: u16, a0: u64, a1: u64, a2: u64, a3: u64, a4: u64, a5: u64) -> u64 {
    let f = GATEWAY.load(Ordering::Acquire);
    if f.is_null() {
        u64::MAX
    } else {
        let handler: GatewayFn = unsafe { core::mem::transmute(f) };
        unsafe { handler(nr, a0, a1, a2, a3, a4, a5) }
    }
}

/// True si bmo_core ya registró su gateway.
pub fn is_ready() -> bool {
    !GATEWAY.load(Ordering::Acquire).is_null()
}
