//! RTX 3060 12G (GA106) — Full driver integration via Driver_Canon GA106 crates.
//!
//! This module bridges the nv_kernel driver stack into FastOS.
//! All operations are Ring 0 — no userspace, no syscalls.

pub use nv_error::{NvError, NvResult};
pub use nv_regs;
pub use nv_gpu::GpuState;
pub use nv_kernel::{DriverState, DriverInfo};

use crate::platform::FastOsPlatform;

static PLATFORM: FastOsPlatform = FastOsPlatform::new();

pub fn init_gpu_driver(gpu_pci_bar0: u64) -> NvResult<DriverState> {
    // [!] FASE 4: Inyectamos voltajes directamente al BAR0 del Hardware (MMIO SEC2)
    // El escáner SigDead descubrió esto: "Offset: 0x00A899B1 -> Register: 0x0010A43C (SEC2 (Secure Boot))"
    {
        let sec2_mmio_base = gpu_pci_bar0 + 0x10A43C;
        // Sólo preparamos el log. NO escribimos de verdad porque hacerlo
        // desde un entorno no controlado podría colapsar el bus PCIe en el host (Pantallazo Azul).
        crate::drivers::serial::serial_write("[SEC2] RTX 3060 Hardware MMIO Base located at: 0x");
        // Convert to hex manually (simple log)
        crate::drivers::serial::serial_write("xxxxxx\n"); // Placeholder until proper hex formatter
    }

    nv_kernel::driver_init(&PLATFORM)
}

pub fn gpu_info(state: &DriverState) -> DriverInfo {
    nv_kernel::driver_info(state)
}

pub fn handle_gpu_irq(state: &mut DriverState) -> u32 {
    nv_kernel::driver_handle_irq(state)
}

pub fn setup_display(
    state: &mut DriverState,
    head: u32,
    width: u32,
    height: u32,
    fb_phys: u64,
) -> NvResult<()> {
    nv_kernel::driver_setup_display(state, head, width, height, fb_phys, &PLATFORM)
}

pub fn teardown(state: &mut DriverState) {
    nv_kernel::driver_teardown(state, &PLATFORM)
}
