//! # nv_firmware — FALCON/GSP Firmware Loader
//!
//! Handles loading GPU firmware into FALCON microcontrollers.
//! In NVIDIA's driver, the 72MB `PAGErGEN` section (entropy 7.99) contains
//! compressed firmware blobs for GSP, PMU, SEC2, NVDEC, etc.
//!
//! Modern NVIDIA drivers (Ampere+) use a GSP (GPU System Processor) that
//! runs its own firmware and manages most GPU operations. The host driver
//! sends commands to GSP via shared memory / ring buffers.
//!
//! SigDead-BIB discovered: PAGErGEN = 72MB, PAGEdBIN = 64KB (additional blobs).
//!
//! `#![no_std]` compatible.

#![no_std]

use nv_error::{NvError, NvResult};
use nv_regs::falcon;
use nv_hal::{MmioRegion, DmaBuffer, Platform};

/// Firmware image header (simplified — real format is proprietary).
/// The actual NVIDIA firmware uses a container format with signatures,
/// version info, and multiple sub-images.
#[derive(Debug, Clone, Copy)]
pub struct FirmwareHeader {
    pub magic: u32,
    pub version: u32,
    pub imem_offset: u32,  // Instruction memory offset in blob
    pub imem_size: u32,
    pub dmem_offset: u32,  // Data memory offset in blob
    pub dmem_size: u32,
    pub boot_vector: u32,
}

/// Which FALCON engine to load firmware into.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FalconEngine {
    Gsp,    // GPU System Processor — main firmware on Ampere+
    Pmu,    // Power Management Unit
    Sec2,   // Security Engine 2
    Nvdec,  // Video Decoder
}

impl FalconEngine {
    /// BAR0 base address for this FALCON instance.
    pub const fn mmio_base(self) -> u32 {
        match self {
            Self::Gsp   => falcon::GSP,
            Self::Pmu   => falcon::PMU,
            Self::Sec2  => falcon::SEC2,
            Self::Nvdec => falcon::NVDEC,
        }
    }
}

/// Reset a FALCON engine and wait for it to be ready.
pub fn falcon_reset(bar0: &MmioRegion, engine: FalconEngine, platform: &dyn Platform) -> NvResult<()> {
    let base = engine.mmio_base();

    // Halt the CPU
    bar0.write32(base + falcon::CPUCTL, falcon::CPUCTL_HALT);
    platform.stall_us(10);

    // Clear all interrupts
    bar0.write32(base + falcon::IRQSCLR, 0xFFFF_FFFF);

    // Verify halted
    let cpuctl = bar0.read32(base + falcon::CPUCTL);
    if cpuctl & falcon::CPUCTL_HALT == 0 {
        return Err(NvError::GpuInFullchipReset);
    }

    Ok(())
}

/// Load firmware into a FALCON engine via DMA.
///
/// Steps (mirrors NVIDIA's nvlddmkm.sys initialization):
/// 1. Halt FALCON
/// 2. DMA transfer IMEM (instruction memory)
/// 3. DMA transfer DMEM (data memory)
/// 4. Set boot vector
/// 5. Start FALCON
pub fn falcon_load(
    bar0: &MmioRegion,
    engine: FalconEngine,
    firmware: &[u8],
    dma_buf: &DmaBuffer,
    platform: &dyn Platform,
) -> NvResult<()> {
    let base = engine.mmio_base();

    // 1. Reset and halt
    falcon_reset(bar0, engine, platform)?;

    // 2. Copy firmware into DMA buffer
    if firmware.len() > dma_buf.size {
        return Err(NvError::BufferTooSmall);
    }
    dma_buf.write(0, firmware);

    // 3. Set DMA transfer base (physical address of DMA buffer, 256-byte aligned)
    let phys_base = (dma_buf.phys >> 8) as u32;
    bar0.write32(base + falcon::DMATRFBASE, phys_base);

    // 4. Transfer IMEM (instruction memory)
    // Transfer in 256-byte blocks
    let imem_blocks = (firmware.len() + 255) / 256;
    for block in 0..imem_blocks {
        let offset = (block * 256) as u32;
        bar0.write32(base + falcon::DMATRFMOFFS, offset);
        bar0.write32(base + falcon::DMATRFFBOFFS, offset);
        bar0.write32(base + falcon::DMATRFCMD, falcon::DMA_CMD_LOAD_IMEM);

        // Wait for transfer complete
        platform.stall_us(10);
    }

    // 5. Set boot vector to 0 (start of IMEM)
    bar0.write32(base + falcon::BOOTVEC, 0);

    // 6. Start the FALCON CPU
    bar0.write32(base + falcon::CPUCTL, falcon::CPUCTL_START);

    // 7. Verify it started — check scratch register for handshake
    platform.stall_us(1000); // Wait 1ms for boot
    let scratch = bar0.read32(base + falcon::SCRATCH0);
    if scratch == 0 {
        // Firmware didn't write handshake — may still be booting or failed
        return Err(NvError::ModuleLoadFailed);
    }

    Ok(())
}

/// Check if a FALCON engine is running.
pub fn falcon_is_running(bar0: &MmioRegion, engine: FalconEngine) -> bool {
    let base = engine.mmio_base();
    let cpuctl = bar0.read32(base + falcon::CPUCTL);
    // If HALT bit is clear, the FALCON is running
    cpuctl & falcon::CPUCTL_HALT == 0
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn falcon_engine_bases() {
        assert_eq!(FalconEngine::Gsp.mmio_base(), falcon::GSP);
        assert_eq!(FalconEngine::Pmu.mmio_base(), falcon::PMU);
        assert_eq!(FalconEngine::Sec2.mmio_base(), falcon::SEC2);
    }
}
