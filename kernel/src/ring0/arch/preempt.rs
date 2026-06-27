//! Preemption Control (Ring 0 HAL).
//!
//! Controls when the scheduler can preempt the current task.
//! Critical sections disable preemption to prevent race conditions.

use core::arch::asm;

/// Disable preemption on the current core.
pub fn disable() {
    unsafe {
        let ptr: *mut u32;
        asm!("mov {}, gs:[32]", out(reg) ptr, options(nostack)); // preempt_count offset
        *ptr += 1;
    }
}

/// Enable preemption on the current core.
pub fn enable() {
    unsafe {
        let ptr: *mut u32;
        asm!("mov {}, gs:[32]", out(reg) ptr, options(nostack));
        if *ptr == 0 {
            panic!("preemption_enable: counter underflow");
        }
        *ptr -= 1;
    }
}

/// Check if preemption is currently disabled.
pub fn is_disabled() -> bool {
    unsafe {
        let val: u32;
        asm!("mov {}, gs:[32]", out(reg) val, options(nostack, readonly));
        val > 0
    }
}

/// Run a closure with preemption disabled.
pub fn disable_guard<F, R>(f: F) -> R
where
    F: FnOnce() -> R,
{
    disable();
    let result = f();
    enable();
    result
}
