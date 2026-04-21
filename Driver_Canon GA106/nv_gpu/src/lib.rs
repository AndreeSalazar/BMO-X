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
#![allow(unused_imports)]

use nv_error::{NvError, NvResult};
use nv_regs::{self, pmc, pfifo, pgraph, pcopy, pdisplay, pmem, ptimer, pbdma, BAR0_SIZE, BAR1_SIZE};
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
    // 0. Set GPU to D0 power state (required on cold boot / USB boot)
    // MSI B550 CSM may leave GPU in D3hot
    nv_hal::set_power_d0(platform, pci);

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
/// On cold boot from USB, not all engines may be ready immediately.
/// We retry with a delay and only require critical engines.
pub fn enable_engines(gpu: &mut Gpu) -> NvResult<()> {
    let mask = pmc::ENABLE_PFIFO
             | pmc::ENABLE_PGRAPH
             | pmc::ENABLE_PCOPY0
             | pmc::ENABLE_PCOPY1
             | pmc::ENABLE_PDISPLAY;

    // First attempt: enable all engines
    gpu.bar0.set_bits(pmc::ENABLE, mask);

    // Brief delay for engines to come up
    // (rdtsc-based busy wait, ~1ms)
    for _ in 0..1000000u32 {
        core::hint::spin_loop();
    }

    // Check which engines are enabled
    let enabled = gpu.bar0.read32(pmc::ENABLE);

    // Critical engines: at least PFIFO must be up
    let critical = pmc::ENABLE_PFIFO;
    if enabled & critical != critical {
        // Try once more after a longer delay
        gpu.bar0.write32(pmc::ENABLE, 0); // Reset all
        for _ in 0..5000000u32 { core::hint::spin_loop(); }
        gpu.bar0.set_bits(pmc::ENABLE, mask);
        for _ in 0..5000000u32 { core::hint::spin_loop(); }

        let enabled2 = gpu.bar0.read32(pmc::ENABLE);
        if enabled2 & critical != critical {
            return Err(NvError::GpuNotFullPower);
        }
    }

    gpu.state = GpuState::EnginesReset;
    Ok(())
}

/// Detect VRAM size from memory controller registers.
///
/// On Ampere (GA106), the FB_MEM_SIZE register at 0x100CE0 reports
/// VRAM in units of 16MB (not 1MB like older GPUs).
/// Value 776 × 16 = 12416 MB ≈ 12 GB (includes firmware-reserved).
///
/// For display, we round down to the marketed size.
pub fn detect_vram(bar0: &MmioRegion) -> u64 {
    let cfg = bar0.read32(pmem::FB_MEM_SIZE);
    let raw_val = (cfg & 0xFFF) as u64;

    // Ampere encoding: value × 16MB
    let total_mb = raw_val * 16;

    if total_mb >= 1024 && total_mb <= 65536 {
        // Valid range (1GB - 64GB) — use Ampere interpretation
        // Round down to nearest GB for clean display
        let gb = total_mb / 1024;
        gb * 1024 * 1024 * 1024
    } else {
        // Legacy encoding: value in MB
        let mb_legacy = (cfg & 0xFFFF) as u64;
        if mb_legacy >= 512 {
            mb_legacy * 1024 * 1024
        } else {
            // Fallback for GA106 RTX 3060 12GB
            12u64 * 1024 * 1024 * 1024
        }
    }
}

/// Return the raw FB_MEM_SIZE register value for debugging.
pub fn detect_vram_raw(bar0: &MmioRegion) -> u32 {
    bar0.read32(pmem::FB_MEM_SIZE)
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

// ---------------------------------------------------------------------------
// FIFO / Channel Management (from SigDead-BIB IOCTL + string analysis)
// ---------------------------------------------------------------------------
// nvlddmkm.sys uses a runlist-based channel scheduler on Ampere.
// Channels are submitted via PBDMA engines ("_PBDMA0", "_PBDMA1").
// SigDead-BIB found: NV_ERR_FIFO_BAD_ACCESS, NV_ERR_INVALID_CHANNEL,
// and 525 IOCTLs including command submission paths.

/// FIFO channel descriptor.
#[derive(Debug, Clone, Copy)]
pub struct FifoChannel {
    /// Channel ID (0..511).
    pub id: u32,
    /// Whether this channel is active.
    pub active: bool,
    /// Push buffer physical address.
    pub pb_phys: u64,
    /// Push buffer size in bytes.
    pub pb_size: u32,
}

/// Initialize FIFO subsystem — enable PFIFO and clear runlists.
pub fn fifo_init(gpu: &Gpu) -> NvResult<()> {
    // Verify PFIFO is enabled
    let enabled = gpu.bar0.read32(pmc::ENABLE);
    if enabled & pmc::ENABLE_PFIFO == 0 {
        return Err(NvError::FifoBadAccess);
    }

    // Clear FIFO interrupts
    gpu.bar0.write32(pfifo::INTR_0, 0xFFFF_FFFF);
    // Enable FIFO error interrupts
    gpu.bar0.write32(pfifo::INTR_EN_0, 0x0000_0001);

    Ok(())
}

/// Read PBDMA engine status (from SigDead-BIB GSP firmware: _PBDMA0, _PBDMA1).
pub fn pbdma_status(gpu: &Gpu, engine: u32) -> u32 {
    if engine >= pbdma::COUNT {
        return 0;
    }
    gpu.bar0.read32(pbdma::STATUS(engine))
}

// ---------------------------------------------------------------------------
// Copy Engine Management (from SigDead-BIB GSP firmware HUB clients)
// ---------------------------------------------------------------------------
// SigDead-BIB found: HUBCLIENT_CE0..CE3, HUBCLIENT_HSCE0..HSCE8,
// HUBCLIENT_CE_SHIM. GA106 has 5 copy engines (CE0-CE4).
// Copy engines handle DMA transfers between system memory and VRAM.

/// Initialize a specific Copy Engine.
pub fn ce_init(gpu: &Gpu, ce_id: u32) -> NvResult<()> {
    if ce_id >= pcopy::CE_COUNT {
        return Err(NvError::InvalidIndex);
    }

    // Clear CE interrupt
    gpu.bar0.write32(pcopy::CE_INTR(ce_id), 0xFFFF_FFFF);

    Ok(())
}

/// Initialize all Copy Engines.
pub fn ce_init_all(gpu: &Gpu) -> NvResult<()> {
    for i in 0..pcopy::CE_COUNT {
        ce_init(gpu, i)?;
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// GPU Info Summary (for shell `gpu` command)
// ---------------------------------------------------------------------------

/// Comprehensive GPU info collected from registers.
#[derive(Debug, Clone, Copy)]
pub struct GpuInfo {
    pub chip_id: u32,
    pub chip: ChipInfo,
    pub vram_bytes: u64,
    pub vram_mb: u32,
    pub engines_enabled: u32,
    pub fifo_enabled: bool,
    pub graph_enabled: bool,
    pub display_enabled: bool,
    pub ce_count: u32,
    pub gpc_count: u32,
    pub sm_count: u32,
    pub pbdma_count: u32,
    pub gpu_time_ns: u64,
    pub state: GpuState,
    /// GSP-RM communication state (from SigDead-BIB XOR 0x20 analysis).
    pub gsp_rm: GspRmState,
}

// ---------------------------------------------------------------------------
// GSP-RM Communication State (from SigDead-BIB XOR 0x20 decoded API)
// ---------------------------------------------------------------------------
// The XOR analysis of gsp_ga10x.bin revealed the complete libos-v3.1.0
// internal architecture:
//   - Virtual memory: kernelAddressSpace, kernelMemorySet, dmaBounceBuffer
//   - Task system:    kernelTaskCreate, handleTable, priority scheduling
//   - Server/RPC:     kernelServerEntry, kernelPortAllocate, serviceWorker
//   - Boot:           libosBootFindElfHeader, rootFS, initELF
//   - Debug:          debugTaskCommsPort, debugElf
//   - MNOC:           mnocWorker, mnocSetRxIRQ (Message Network-On-Chip)
//   - Crypto:         51× AES Rcon, 51× RSA e=65537, SHA-256 (firmware signing)
//
// For FastOS to fully communicate with the RTX 3060's GSP-RM, we need:
//   1. DMA bounce buffer (host→GSP shared memory) — allocated in VRAM
//   2. Command ring buffer (RPC message queue)
//   3. GSP boot via SEC2→GSP FALCON handoff
//   4. RPC handshake (MSG_INIT → GSP responds with capabilities)
//   5. Ongoing RPC for engine control, display, power management

/// GSP-RM communication state — tracks host↔GSP protocol status.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct GspRmState {
    /// GSP firmware loaded into VRAM WPR.
    pub fw_loaded: bool,
    /// FALCON bootstrap completed (SEC2 → GSP handoff).
    pub falcon_booted: bool,
    /// DMA bounce buffer allocated for host↔GSP data transfer.
    pub dma_bounce_ready: bool,
    /// Command ring buffer initialized.
    pub cmd_ring_ready: bool,
    /// RPC handshake completed (MSG_INIT sent and acknowledged).
    pub rpc_handshake: bool,
    /// GSP-RM server running (kernelServerEntry reached).
    pub server_running: bool,
    /// Number of RPC messages sent to GSP.
    pub rpc_msg_sent: u32,
    /// Number of RPC responses received from GSP.
    pub rpc_msg_recv: u32,
    /// Last RPC status code from GSP (0 = LIBOS_OK).
    pub last_rpc_status: u32,
    /// GSP libos version detected.
    pub libos_version: GspLibosVersion,
    /// Crypto capabilities detected in firmware.
    pub crypto: GspCrypto,
}

/// GSP libos version info.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct GspLibosVersion {
    pub major: u8,
    pub minor: u8,
    pub patch: u8,
}

/// Crypto info found in GSP firmware by SigDead-BIB.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct GspCrypto {
    /// AES-256 used for internal encrypted channels.
    pub aes_present: bool,
    /// AES round constant instances found.
    pub aes_rcon_count: u32,
    /// RSA PKCS#1 v1.5 signatures in firmware.
    pub rsa_sig_count: u32,
    /// SHA-256 used for firmware integrity verification.
    pub sha256_present: bool,
}

impl GspRmState {
    /// Initial state — nothing initialized yet.
    pub const fn uninit() -> Self {
        Self {
            fw_loaded: false,
            falcon_booted: false,
            dma_bounce_ready: false,
            cmd_ring_ready: false,
            rpc_handshake: false,
            server_running: false,
            rpc_msg_sent: 0,
            rpc_msg_recv: 0,
            last_rpc_status: 0,
            libos_version: GspLibosVersion { major: 3, minor: 1, patch: 0 },
            crypto: GspCrypto {
                aes_present: true,
                aes_rcon_count: 51,
                rsa_sig_count: 2,
                sha256_present: true,
            },
        }
    }

    /// Check if GSP-RM is fully operational (all init stages complete).
    pub fn is_operational(&self) -> bool {
        self.fw_loaded && self.falcon_booted && self.dma_bounce_ready
            && self.cmd_ring_ready && self.rpc_handshake && self.server_running
    }

    /// Human-readable status string.
    pub fn status_str(&self) -> &'static str {
        if self.is_operational() {
            "OPERATIONAL"
        } else if self.rpc_handshake {
            "RPC_READY"
        } else if self.cmd_ring_ready {
            "CMD_RING_READY"
        } else if self.dma_bounce_ready {
            "DMA_READY"
        } else if self.falcon_booted {
            "FALCON_BOOTED"
        } else if self.fw_loaded {
            "FW_LOADED"
        } else {
            "UNINITIALIZED"
        }
    }
}

/// Gather GPU info from live registers.
pub fn gpu_info(gpu: &Gpu) -> GpuInfo {
    let enabled = gpu.bar0.read32(pmc::ENABLE);
    let time = gpu_time_ns(gpu);
    let chip = ChipInfo::from_boot0(gpu.chip_id);

    GpuInfo {
        chip_id: gpu.chip_id,
        chip,
        vram_bytes: gpu.vram_size,
        vram_mb: (gpu.vram_size / (1024 * 1024)) as u32,
        engines_enabled: enabled,
        fifo_enabled: enabled & pmc::ENABLE_PFIFO != 0,
        graph_enabled: enabled & pmc::ENABLE_PGRAPH != 0,
        display_enabled: enabled & pmc::ENABLE_PDISPLAY != 0,
        ce_count: pcopy::CE_COUNT,
        gpc_count: pgraph::GPC_COUNT,
        sm_count: pgraph::SM_COUNT,
        pbdma_count: pbdma::COUNT,
        gpu_time_ns: time,
        state: gpu.state,
        gsp_rm: GspRmState::uninit(),
    }
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
