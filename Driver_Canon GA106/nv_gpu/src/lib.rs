//! # nv_gpu — GPU Core Initialization & Memory Management
//!
//! Central GPU state: init, reset, VRAM detection, engine enable, interrupts.
//! Maps to nvlddmkm.sys sections: _KTEXT (non-paged core), PAGE_K (paged kernel),
//! PAGEcRM (Resource Manager).
//!
//! SigDead-BIB found: 76,231 functions, 32 sections, 336 imports from ntoskrnl/HAL.
//! The driver's INIT section (39KB) is discarded after DriverEntry — we mirror that.
//!
//! `#![no_std]` compatible.

#![no_std]

use nv_error::{NvError, NvResult};
use nv_regs::{self, pmc, pfifo, pgraph, pcopy, pdisplay, pmem, ptimer, BAR0_SIZE, BAR1_SIZE};
use nv_hal::{MmioRegion, PciAddress, Platform, DmaBuffer};
use nv_firmware::{self, FalconEngine};

/// GPU state — the main driver object.
/// Equivalent to NVIDIA's Resource Manager (PAGEcRM section).
pub struct Gpu {
    pub bar0: MmioRegion,
    pub bar1: Option<MmioRegion>,
    pub pci: PciAddress,
    pub chip_id: u32,
    pub vram_size: u64,
    pub state: GpuState,
}

/// GPU lifecycle states (from NVIDIA error strings).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GpuState {
    Uninitialized,
    BarsMapping,
    EnginesReset,
    FirmwareLoaded,
    Ready,
    Lost,           // NV_ERR_GPU_IS_LOST
    InReset,        // NV_ERR_GPU_IN_FULLCHIP_RESET
}

/// GPU chip information read from BOOT_0.
#[derive(Debug, Clone, Copy)]
pub struct ChipInfo {
    pub chip_id: u32,
    pub implementation: u8,     // bits [23:20]
    pub architecture: u8,       // bits [27:24]
    pub revision: u8,           // bits [11:8]
    pub is_ampere: bool,
}

impl ChipInfo {
    pub fn from_boot0(val: u32) -> Self {
        Self {
            chip_id: val,
            implementation: ((val >> 20) & 0xF) as u8,
            architecture: ((val >> 24) & 0xF) as u8,
            revision: ((val >> 8) & 0xF) as u8,
            is_ampere: ((val >> 24) & 0xF) >= 0xA, // Ampere = arch 0x17+
        }
    }
}

/// Initialize GPU — the main entry point (equivalent to DriverEntry/INIT section).
///
/// Sequence:
/// 1. Map BARs
/// 2. Read chip ID
/// 3. Enable engines via PMC
/// 4. Detect VRAM
/// 5. Load firmware (GSP/PMU)
/// 6. Enable interrupts
pub fn gpu_init(platform: &dyn Platform, pci: PciAddress) -> NvResult<Gpu> {
    // 1. Enable bus mastering for DMA
    nv_hal::enable_bus_master(platform, pci);

    // 2. Map BAR0 (register space)
    let bar0_phys = nv_hal::read_bar0(platform, pci);
    let bar0_ptr = platform.map_mmio(bar0_phys, BAR0_SIZE);
    if bar0_ptr.is_null() {
        return Err(NvError::InvalidAddress);
    }
    let bar0 = unsafe { MmioRegion::new(bar0_ptr, BAR0_SIZE) };

    // 3. Read chip ID from PMC.BOOT_0
    let boot0 = bar0.read32(pmc::BOOT_0);
    if boot0 == 0 || boot0 == 0xFFFF_FFFF {
        return Err(NvError::CardNotPresent);
    }
    let chip = ChipInfo::from_boot0(boot0);

    // 4. Detect VRAM size
    let vram_size = detect_vram(&bar0);

    // 5. Map BAR1 (VRAM aperture) — optional for init
    let bar1_phys = nv_hal::read_bar1(platform, pci);
    let bar1 = if bar1_phys != 0 {
        let ptr = platform.map_mmio(bar1_phys, BAR1_SIZE);
        if !ptr.is_null() {
            Some(unsafe { MmioRegion::new(ptr, BAR1_SIZE) })
        } else {
            None
        }
    } else {
        None
    };

    Ok(Gpu {
        bar0,
        bar1,
        pci,
        chip_id: chip.chip_id,
        vram_size,
        state: GpuState::BarsMapping,
    })
}

/// Enable GPU engines via PMC.ENABLE register.
pub fn enable_engines(gpu: &mut Gpu) -> NvResult<()> {
    let mask = pmc::ENABLE_PFIFO
             | pmc::ENABLE_PGRAPH
             | pmc::ENABLE_PCOPY0
             | pmc::ENABLE_PCOPY1
             | pmc::ENABLE_PDISPLAY;

    gpu.bar0.set_bits(pmc::ENABLE, mask);

    // Verify engines are enabled
    let enabled = gpu.bar0.read32(pmc::ENABLE);
    if enabled & mask != mask {
        return Err(NvError::GpuNotFullPower);
    }

    gpu.state = GpuState::EnginesReset;
    Ok(())
}

/// Detect VRAM size from memory controller registers.
fn detect_vram(bar0: &MmioRegion) -> u64 {
    let cfg = bar0.read32(pmem::FB_MEM_SIZE);
    // FB_MEM_SIZE is in units of MB on Ampere
    let mb = cfg & 0xFFFF;
    (mb as u64) * 1024 * 1024
}

/// Read GPU timer (nanosecond precision).
pub fn gpu_time_ns(gpu: &Gpu) -> u64 {
    let lo = gpu.bar0.read32(ptimer::TIME_LO) as u64;
    let hi = gpu.bar0.read32(ptimer::TIME_HI) as u64;
    (hi << 32) | lo
}

/// Enable GPU interrupts at top level.
pub fn enable_interrupts(gpu: &Gpu) {
    let mask = pmc::INTR_PFIFO
             | pmc::INTR_PGRAPH
             | pmc::INTR_PCOPY0
             | pmc::INTR_PCOPY1
             | pmc::INTR_PDISPLAY;

    gpu.bar0.write32(pmc::INTR_EN_0, mask);
}

/// Disable all GPU interrupts.
pub fn disable_interrupts(gpu: &Gpu) {
    gpu.bar0.write32(pmc::INTR_EN_0, 0);
}

/// Top-half interrupt handler — check which engines fired.
/// Returns bitmask of pending interrupts (0 = not ours).
pub fn handle_interrupt(gpu: &Gpu) -> u32 {
    let pending = gpu.bar0.read32(pmc::INTR_0);
    if pending == 0 {
        return 0;
    }

    // Acknowledge all pending interrupts
    gpu.bar0.write32(pmc::INTR_0, pending);

    pending
}

/// Full GPU reset sequence.
pub fn gpu_reset(gpu: &mut Gpu, platform: &dyn Platform) -> NvResult<()> {
    gpu.state = GpuState::InReset;

    // Disable interrupts first
    disable_interrupts(gpu);

    // Disable all engines
    gpu.bar0.write32(pmc::ENABLE, 0);
    platform.stall_us(100);

    // Re-enable engines
    enable_engines(gpu)?;

    gpu.state = GpuState::EnginesReset;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn chip_info_parsing() {
        // Simulated GA106 BOOT_0 value: arch=0x7 @ bits[27:24], rev=0xA @ bits[11:8]
        let chip = ChipInfo::from_boot0(0x1700_0A01);
        assert_eq!(chip.architecture, 7);
        assert_eq!(chip.revision, 0xA);
    }

    #[test]
    fn gpu_states() {
        assert_ne!(GpuState::Ready, GpuState::Lost);
        assert_eq!(GpuState::Uninitialized, GpuState::Uninitialized);
    }
}
