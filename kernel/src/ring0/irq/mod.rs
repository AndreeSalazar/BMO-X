//! IRQ dispatch — handler registry + unified dispatcher.
//!
//! ## Architecture
//!
//! ```text
//! IDT handler (asm stub)
//!   → irq::dispatch(vector)
//!     → HANDLERS[vector]()
//!       → driver callback
//!         → eoi()
//! ```

pub mod lapic;
pub mod ioapic;
pub mod msi;

/// Maximum IRQ vectors supported.
const MAX_HANDLERS: usize = 256;

type IrqHandler = fn();

static mut HANDLERS: [Option<IrqHandler>; MAX_HANDLERS] = [None; MAX_HANDLERS];

/// Register a handler for a given interrupt vector.
/// Returns `true` on success, `false` if the slot is already taken.
pub fn register(vector: u8, handler: IrqHandler) -> bool {
    let idx = vector as usize;
    if idx >= MAX_HANDLERS { return false; }
    unsafe {
        if HANDLERS[idx].is_some() { return false; }
        HANDLERS[idx] = Some(handler);
        true
    }
}

/// Dispatch an interrupt by vector. The handler is responsible for
/// calling `irq::lapic::eoi()` at the appropriate time.
pub fn dispatch(vector: u8) {
    let idx = vector as usize;
    if idx >= MAX_HANDLERS { return; }
    if let Some(handler) = unsafe { HANDLERS[idx] } {
        handler();
    }
}

/// Default timer handler — ticks the scheduler, pets the watchdog.
fn default_timer() {
    crate::proc::timer_tick();
    crate::dev::watchdog::pet();
    crate::dev::watchdog::check();
}

/// Initialize the IRQ subsystem: register built-in handlers.
pub fn init() {
    register(crate::irq::lapic::TIMER_VECTOR, default_timer);
}
