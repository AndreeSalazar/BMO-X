//! RTX 3060 12G (GA106) — register offsets from SigDead-BIB nv_regs.

pub const NVIDIA_VENDOR_ID: u16 = 0x10DE;
pub const GA106_DEVICE_ID: u16  = 0x2504;
pub const BAR0_SIZE: usize = 16 * 1024 * 1024;

pub mod regs {
    pub const BOOT_0: u32        = 0x0000_0000;
    pub const PMC_ENABLE: u32    = 0x0000_0200;
    pub const PMC_INTR_0: u32    = 0x0000_0100;
    pub const PTIMER_LO: u32     = 0x0000_9400;
    pub const PTIMER_HI: u32     = 0x0000_9410;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GpuState {
    Uninitialized,
    Detected,
    BarsMapped,
    Ready,
    Error,
}

pub struct Rtx3060 {
    pub bar0_phys: u64,
    pub state: GpuState,
    pub chip_id: u32,
}

impl Rtx3060 {
    pub fn new(bar0_phys: u64) -> Self {
        Self { bar0_phys, state: GpuState::Detected, chip_id: 0 }
    }
}
