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
pub mod i8042;
pub mod keyboard;
pub mod mouse;
pub mod apic_mmio;

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

/// Default timer handler — ticks the scheduler, timer wheel, pets the watchdog.
fn default_timer() {
    crate::ring0::proc::timer_tick();
    crate::ring0::dev::timer_wheel::tick();
    crate::ring0::dev::watchdog::pet();
    crate::ring0::dev::watchdog::check();
}

/// Initialize the IRQ subsystem: register built-in handlers.
pub fn init() {
    register(crate::ring0::irq::lapic::TIMER_VECTOR, default_timer);
}
