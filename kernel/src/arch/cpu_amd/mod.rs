//! AMD-specific CPU initialization module.

pub mod ryzen_5_5600x;

/// Detects CPU features and initializes AMD Ryzen optimizations.
pub fn init() {
    crate::drivers::serial::serial_write("[CPU-AMD] Detectando extensiones del procesador...\n");
    let features = crate::arch::cpu::detect_cpu();
    ryzen_5_5600x::init(&features);
}
