//! CPU Idle States (C-states) (Ring 0 HAL).
//!
//! Manages CPU idle states for power saving. When a core has no work,
//! it enters a C-state (C1 halt, C2 stop-clock, C3 sleep) to reduce
//! power consumption and heat generation.
//!
//! C-states on AMD Zen3:
//!   - C0: Active (executing instructions)
//!   - C1: HLT — core clock stopped, wakeup on interrupt
//!   - C2: MWAIT — deeper sleep, faster wakeup
//!   - C3: Sleep — deepest idle, L2 cache flushed
//!
//! The MWAIT instruction is the preferred way to enter C-states.
//! It's more efficient than HLT and allows the CPU to specify
//! the target C-state via the EAX register.

use core::arch::asm;

/// C-state types (matches ACPI _CST return values).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u32)]
pub enum CStateType {
    C0 = 0,  // Active
    C1 = 1,  // HLT
    C2 = 2,  // Stop-Clock
    C3 = 3,  // Sleep
}

/// MWAIT hints for each C-state.
const MWAIT_C1: u32 = 0x00; // C1: clock stopped
const MWAIT_C2: u32 = 0x10; // C2: stop-clock
const MWAIT_C3: u32 = 0x20; // C3: sleep

/// C-state configuration per core.
#[derive(Debug, Clone, Copy)]
pub struct CStateConfig {
    pub target: CStateType,
    pub latency_us: u32,     // Wakeup latency in microseconds
    pub power_mw: u32,       // Power consumption in milliwatts
}

/// Default C-state table for AMD Ryzen 5 5600X.
static DEFAULT_CSTATES: [CStateConfig; 3] = [
    CStateConfig { target: CStateType::C1, latency_us: 1, power_mw: 50 },
    CStateConfig { target: CStateType::C2, latency_us: 5, power_mw: 20 },
    CStateConfig { target: CStateType::C3, latency_us: 50, power_mw: 5 },
];

static mut CURRENT_CSTATE: CStateType = CStateType::C0;
static mut CSTATES_ENABLED: bool = false;

/// Initialize C-state support.
pub fn init() {
    crate::dev::console::serial_write("[cstates] initializing\n");

    // Check if MWAIT is supported (CPUID.01H:ECX[3])
    let (_, _, ecx, _) = crate::cpu::cpuid(1, 0);
    if ecx & (1 << 3) == 0 {
        crate::dev::console::serial_write("[cstates] MWAIT not supported\n");
        return;
    }

    // Enable MONITOR/MWAIT (CR4.ENUMMonitorMWait can be set if needed)
    // MWAIT is available by default on x86-64

    unsafe { CSTATES_ENABLED = true; }
    crate::dev::console::serial_write("[cstates] MWAIT supported, C-states enabled\n");
}

/// Enter the specified C-state using MWAIT.
///
/// # Safety
/// Must be called with interrupts disabled and only when the core
/// is truly idle. The core will wake up on the next interrupt.
pub unsafe fn enter_cstate(target: CStateType) {
    if !CSTATES_ENABLED { return; }

    let hint = match target {
        CStateType::C0 => return,
        CStateType::C1 => MWAIT_C1,
        CStateType::C2 => MWAIT_C2,
        CStateType::C3 => MWAIT_C3,
    };

    // MWAIT hint: EAX = C-state hint, ECX = interrupt-only wakeup
    // ECX[0] = 0: wake on any interrupt
    // ECX[0] = 1: wake only on unmasked interrupts
    asm!(
        "monitor",  // MONITOR: set up monitoring address
        "mwait",    // MWAIT: enter idle state
        in("eax") 0u64,   // monitoring address (any valid address)
        in("ecx") 0u64,   // hint: wake on any interrupt
        in("edx") hint,
        options(nostack),
    );

    unsafe { CURRENT_CSTATE = CStateType::C0; }
}

/// Enter the lightest C-state (C1/HALT).
/// Used by the idle loop when no deeper sleep is needed.
pub fn halt_idle() {
    unsafe {
        CURRENT_CSTATE = CStateType::C1;
        asm!("sti; hlt", options(nostack));
        CURRENT_CSTATE = CStateType::C0;
    }
}

/// Get the current C-state.
pub fn current() -> CStateType {
    unsafe { CURRENT_CSTATE }
}

/// Get C-state configuration table.
pub fn cstate_table() -> &'static [CStateConfig] {
    &DEFAULT_CSTATES
}
