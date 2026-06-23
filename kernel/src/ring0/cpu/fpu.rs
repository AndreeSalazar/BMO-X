//! FPU/SSE/AVX setup + lazy FPU context switching.
//!
//! v1.8.7: este módulo se simplificó drásticamente. Antes exponía la
//! API completa de XSAVE/XRSTOR/FXSAVE/FXRSTOR para save/restore de
//! contexto por task. Esa API no tenía consumidores en el kernel actual
//! (el scheduler RR no hace ctx switch con FPU state — solo lo activa
//! "lazy" vía CR0.TS).
//!
//! Lo que se conserva:
//!   - `init_fpu`: configura x87 + MXCSR al boot (lo llama `cpu::init`).
//!   - `enable_lazy_fpu`: pone CR0.TS=1 (lo llama `cpu::init`).
//!   - `clear_lazy_fpu`: pone CR0.TS=0 (lo llama el ISR de #NM en idt.rs
//!     cuando un thread usa FPU por primera vez).
//!
//! Si en el futuro se quiere per-thread FPU state (vía save_context/
//! restore_context), restaurar la API completa desde git.

/// Initialize FPU/SSE/AVX for the boot CPU.
///
/// Must be called after CR0/CR4/XCR0 are configured.
/// Sets up the initial FPU state with:
/// - x87 FPU: round to nearest, double precision
/// - MXCSR: default value (exceptions masked)
pub fn init_fpu() {
    unsafe {
        // Initialize x87 FPU state
        core::arch::asm!(
            "fninit",
            options(nostack),
        );

        // Set MXCSR to default: all exceptions masked, round to nearest
        let mxcsr: u32 = 0x1F80; // Default MXCSR value
        core::arch::asm!(
            "ldmxcsr [{addr}]",
            addr = in(reg) &mxcsr as *const u32,
            options(nostack),
        );

        crate::dev::console::serial_write("[FPU] x87 FPU + MXCSR initialized\n");
    }
}
