//! Thermal Monitoring — stubbed (init only, check not yet wired).

/// Initialize thermal monitoring.
pub fn init() {
    crate::dev::console::serial_write("[thermal] initializing\n");
    let (eax, _, _, _) = crate::cpu::cpuid(6, 0);
    if eax & (1 << 6) == 0 {
        crate::dev::console::serial_write("[thermal] no thermal sensors detected\n");
    } else {
        crate::dev::console::serial_write("[thermal] sensors present\n");
    }
}
