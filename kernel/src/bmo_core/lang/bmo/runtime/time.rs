//! ÑEXO Runtime — Reloj y tiempo.
//!
//! Wraps kernel clock/timer services.

#![allow(dead_code)]

/// Get current time in nanoseconds since boot.
pub fn now_ns() -> u64 {
    let mut ns: u64;
    unsafe {
        core::arch::asm!(
            "mov rax, 0x50",
            "syscall",
            out("rax") ns,
            options(nomem, nostack)
        );
    }
    ns
}

/// Get current time in milliseconds since boot.
pub fn now_ms() -> u64 {
    now_ns() / 1_000_000
}

/// Get current time in seconds since boot.
pub fn now_secs() -> u64 {
    now_ns() / 1_000_000_000
}

/// Sleep for given nanoseconds (blocking).
pub fn sleep_ns(ns: u64) {
    unsafe {
        core::arch::asm!(
            "mov rax, 0x51",
            "syscall",
            in("rdi") ns,
            options(nomem, nostack)
        );
    }
}

/// Sleep for given milliseconds.
pub fn sleep_ms(ms: u64) {
    sleep_ns(ms * 1_000_000);
}

/// Sleep for given seconds.
pub fn sleep_secs(secs: u64) {
    sleep_ns(secs * 1_000_000_000);
}

/// Timer for measuring elapsed time.
pub struct Timer {
    start_ns: u64,
}

impl Timer {
    /// Create and start a timer.
    pub fn start() -> Self {
        Self { start_ns: now_ns() }
    }

    /// Elapsed nanoseconds since creation.
    pub fn elapsed_ns(&self) -> u64 {
        now_ns().wrapping_sub(self.start_ns)
    }

    /// Elapsed milliseconds since creation.
    pub fn elapsed_ms(&self) -> u64 {
        self.elapsed_ns() / 1_000_000
    }

    /// Elapsed seconds since creation.
    pub fn elapsed_secs(&self) -> u64 {
        self.elapsed_ns() / 1_000_000_000
    }

    /// Reset the timer.
    pub fn reset(&mut self) {
        self.start_ns = now_ns();
    }
}

/// Initialize time subsystem.
pub fn init() {
    crate::bmo_core::diag::info("nexo_time", "Time subsystem initialized");
}
