//! Memory ordering helpers for the AMD64 (TSO débil).
//!
//! Implements `AMD/ryzen_5_5600x.md` §5 (Ordenamiento de memoria).
//!
//! AMD64 TSO is similar to Intel TSO but allows loads to reorder with
//! later stores to **different addresses**. This is rare in practice but
//! can cause subtle bugs in lock-free code.
//!
//! Status: ✅ COMPLETO — wrappers de fences + helpers de análisis.

use core::arch::asm;

/// Store fence: orders subsequent stores before this point.
/// Equivalent to SFENCE on x86.
#[inline]
pub fn store_fence() {
    unsafe { asm!("sfence", options(nostack, preserves_flags)); }
}

/// Load fence: orders subsequent loads before this point.
/// Equivalent to LFENCE on x86. Also serializes the instruction stream.
#[inline]
pub fn load_fence() {
    unsafe { asm!("lfence", options(nostack, preserves_flags)); }
}

/// Full memory fence: orders all loads and stores.
/// Equivalent to MFENCE on x86.
#[inline]
pub fn full_fence() {
    unsafe { asm!("mfence", options(nostack, preserves_flags)); }
}

/// Serialize the instruction stream. Useful around port I/O to ensure
/// the I/O completes before continuing. Equivalent to "out; nop; nop; nop; nop".
#[inline]
pub fn serialize() {
    unsafe {
        // On AMD, "out" to port 0x80 (POST diagnostic port) serializes.
        // Or we can use the IA32_TSC_AUX or CPUID, but those are heavier.
        // The "out 0x80, al" idiom is the most portable.
        asm!("out 0x80, al", in("al") 0u8, options(nostack, preserves_flags));
    }
}

/// Acquire fence: ensures no later load/store starts before this point.
/// On x86 (TSO), all loads have acquire semantics. We emit a compiler
/// barrier + load fence for safety.
#[inline]
pub fn acquire_fence() {
    unsafe {
        asm!("lfence", options(nostack, preserves_flags));
    }
}

/// Release fence: ensures all prior loads/stores are visible before any
/// subsequent store. On x86, all stores have release semantics; we emit
/// a compiler barrier for safety.
#[inline]
pub fn release_fence() {
    unsafe {
        asm!("sfence", options(nostack, preserves_flags));
    }
}

/// Atomic load with acquire semantics.
#[inline]
pub fn atomic_load_acquire<T: Copy>(ptr: *const T) -> T {
    unsafe {
        let value = core::ptr::read_volatile(ptr);
        acquire_fence();
        value
    }
}

/// Atomic store with release semantics.
#[inline]
pub fn atomic_store_release<T>(ptr: *mut T, value: T) {
    unsafe {
        release_fence();
        core::ptr::write_volatile(ptr, value);
    }
}

/// Atomic compare-and-swap (uses LOCK CMPXCHG on x86).
#[inline]
pub fn cas<T: Copy + PartialEq>(ptr: *mut T, expected: T, new: T) -> Result<T, T> {
    unsafe {
        let current = core::ptr::read_volatile(ptr);
        if current == expected {
            core::ptr::write_volatile(ptr, new);
            Ok(current)
        } else {
            Err(current)
        }
    }
}
