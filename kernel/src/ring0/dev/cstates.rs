//! CPU Idle States (C-states) — stubbed (only init kept).

/// Initialize C-state support.
pub fn init() {
    crate::dev::console::serial_write("[cstates] initializing\n");
    let (_, _, ecx, _) = crate::cpu::cpuid(1, 0);
    if ecx & (1 << 3) == 0 {
        crate::dev::console::serial_write("[cstates] MWAIT not supported\n");
        return;
    }
    crate::dev::console::serial_write("[cstates] MWAIT supported\n");
}
