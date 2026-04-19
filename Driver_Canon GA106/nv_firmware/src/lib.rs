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

// ---------------------------------------------------------------------------
// GSP-RM Container Format
// ---------------------------------------------------------------------------
// gsp_ga10x.bin is a RISC-V ELF containing the GPU System Processor firmware.
// SigDead-BIB analysis confirmed: ~72MB, contains embedded ELF(s) with
// RISC-V (EM_RISCV = 243) machine type for the on-chip GSP processor.
//
// The GSP runs NVIDIA's Resource Manager (RM) on the GPU itself.
// Host driver communicates with GSP via shared-memory RPC.

/// GSP-RM firmware container — parsed from gsp_ga10x.bin.
#[derive(Debug, Clone, Copy)]
pub struct GspContainer {
    /// True if the blob starts with ELF magic (\x7FELF).
    pub is_elf: bool,
    /// ELF class: 32 or 64.
    pub elf_class: u8,
    /// ELF machine type (243 = RISC-V for GSP).
    pub elf_machine: u16,
    /// Entry point address from ELF header.
    pub entry_point: u64,
    /// Number of program headers (loadable segments).
    pub phdr_count: u16,
    /// Number of section headers.
    pub shdr_count: u16,
    /// Total firmware size.
    pub total_size: u32,
}

/// ELF magic bytes.
const ELF_MAGIC: [u8; 4] = [0x7F, b'E', b'L', b'F'];
/// RISC-V machine type.
const EM_RISCV: u16 = 243;

/// Parse the GSP-RM firmware container header from a raw blob.
/// Returns `None` if the blob is too small or not a valid container.
pub fn parse_gsp_container(blob: &[u8]) -> Option<GspContainer> {
    if blob.len() < 64 {
        return None;
    }

    // Check for ELF magic
    if blob[0] == ELF_MAGIC[0] && blob[1] == ELF_MAGIC[1]
        && blob[2] == ELF_MAGIC[2] && blob[3] == ELF_MAGIC[3]
    {
        let class = blob[4]; // 1=32-bit, 2=64-bit
        let machine = u16::from_le_bytes([blob[18], blob[19]]);

        let (entry_point, phdr_count, shdr_count) = if class == 2 {
            // ELF64
            let entry = u64::from_le_bytes([
                blob[24], blob[25], blob[26], blob[27],
                blob[28], blob[29], blob[30], blob[31],
            ]);
            let ph = u16::from_le_bytes([blob[56], blob[57]]);
            let sh = u16::from_le_bytes([blob[60], blob[61]]);
            (entry, ph, sh)
        } else {
            // ELF32
            let entry = u32::from_le_bytes([
                blob[24], blob[25], blob[26], blob[27],
            ]) as u64;
            let ph = u16::from_le_bytes([blob[44], blob[45]]);
            let sh = u16::from_le_bytes([blob[48], blob[49]]);
            (entry, ph, sh)
        };

        return Some(GspContainer {
            is_elf: true,
            elf_class: if class == 2 { 64 } else { 32 },
            elf_machine: machine,
            entry_point,
            phdr_count,
            shdr_count,
            total_size: blob.len() as u32,
        });
    }

    // Not an ELF — could be a proprietary container or compressed blob
    None
}

/// Check if a GSP container targets RISC-V (Ampere+ GSP processor).
pub fn is_gsp_riscv(container: &GspContainer) -> bool {
    container.is_elf && container.elf_machine == EM_RISCV
}

// ---------------------------------------------------------------------------
// GSP-RM ELF Section Layout (from SigDead-BIB firmware analysis)
// ---------------------------------------------------------------------------
// SigDead-BIB `--firmware gsp_ga10x.bin` revealed the actual ELF64 structure:
//
//   Type      : ELF64 ET_REL (relocatable), RISC-V
//   Size      : 72,845,296 bytes (69.47 MB)
//   Sections  : 17 total
//   FALCON hdr: 103 embedded FALCON microcode headers
//   Signatures: 2 × RSA PKCS#1 v1.5 (256-byte)
//   Strings   : 10,806 firmware strings extracted
//
// Section names from the ELF:
//   .fwimage                — Main firmware image (RISC-V code + data)
//   .note.gnu.build-id      — Build identification
//   .fwversion              — Firmware version string
//   .fwsignature_ga10x      — RSA signature for GA10x (Ampere)
//   .fwsignature_gh100      — RSA signature for GH100 (Hopper)
//   .fwsignature_gb10x      — RSA signature for GB10x (Blackwell)
//   .fwsignature_gb10y      — RSA signature for GB10y
//   .fwsignature_gb20x      — RSA signature for GB20x
//   .fwsignature_gb20y      — RSA signature for GB20y
//   .fwsignature_ad10x      — RSA signature for AD10x (Ada Lovelace)
//   .fwsignature_cc_gh100   — CC signature for GH100
//   .fwsignature_cc_gb10x   — CC signature for GB10x
//   .fwsignature_cc_gb20x   — CC signature for GB20x
//   .symtab                 — Symbol table
//   .strtab                 — String table
//   .shstrtab               — Section header string table

/// Known ELF section names inside gsp_ga10x.bin.
pub mod gsp_sections {
    pub const FWIMAGE: &str          = ".fwimage";
    pub const BUILD_ID: &str         = ".note.gnu.build-id";
    pub const FWVERSION: &str        = ".fwversion";
    pub const SIG_GA10X: &str        = ".fwsignature_ga10x";
    pub const SIG_GH100: &str        = ".fwsignature_gh100";
    pub const SIG_AD10X: &str        = ".fwsignature_ad10x";
    pub const SIG_GB10X: &str        = ".fwsignature_gb10x";
    pub const SIG_GB10Y: &str        = ".fwsignature_gb10y";
    pub const SIG_GB20X: &str        = ".fwsignature_gb20x";
    pub const SIG_GB20Y: &str        = ".fwsignature_gb20y";
    pub const SECTION_COUNT: usize   = 17;
}

// ---------------------------------------------------------------------------
// GSP Internal OS: libos-v3.1.0 (from SigDead-BIB string extraction)
// ---------------------------------------------------------------------------
// The GSP runs an internal microkernel called "libos" (v3.1.0).
// SigDead-BIB extracted 210 source file paths from the firmware:
//
//   /gpu_drv/uproc/os/libos-v3.1.0/kernel/full/mm/memorypool.c
//   /gpu_drv/uproc/os/libos-v3.1.0/kernel/full/mm/objectpool.c
//   /gpu_drv/uproc/os/libos-v3.1.0/kernel/full/mm/pagestate.c
//   /gpu_drv/uproc/os/libos-v3.1.0/kernel/full/mm/identity.c
//   /gpu_drv/uproc/os/libos-v3.1.0/kernel/full/mm/pagetable.c
//   /gpu_drv/uproc/os/libos-v3.1.0/kernel/full/mm/address_space.c
//   /gpu_drv/uproc/os/libos-v3.1.0/kernel/full/ipi.c
//   /gpu_drv/uproc/os/libos-v3.1.0/kernel/full/sched/port.c
//   /gpu_drv/uproc/os/libos-v3.1.0/kernel/full/sched/heartbeat.c
//   /gpu_drv/uproc/os/libos-v3.1.0/kernel/full/partition.c
//   /gpu_drv/uproc/os/libos-v3.1.0/kernel/full/server/server.c
//   /gpu_drv/uproc/os/libos-v3.1.0/kernel/full/loader.c
//   /gpu_drv/uproc/os/libos-v3.1.0/kernel/drivers/dma.c
//   /gpu_drv/uproc/os/libos-v3.1.0/kernel/drivers/extintr-v2.c
//   /gpu_drv/uproc/os/libos-v3.1.0/kernel/drivers/gdma.c
//
// Embedded kernel ELF names found in firmware strings:
//   kernel_ga10x.elf    — Ampere GA10x (our target)
//   kernel_gh100.elf    — Hopper GH100
//   kernel_gb10x.elf    — Blackwell GB10x
//   kernel_gb10y.elf    — Blackwell GB10y
//   kernel_gb20x.elf    — Blackwell GB20x
//   kernel_gb20y.elf    — Blackwell GB20y

/// GSP internal libos kernel metadata.
pub mod gsp_libos {
    pub const VERSION: &str = "libos-v3.1.0";

    /// Embedded kernel ELF for GA10x (our RTX 3060 target).
    pub const KERNEL_ELF_GA10X: &str = "kernel_ga10x.elf";
    pub const KERNEL_ELF_GH100: &str = "kernel_gh100.elf";
    pub const KERNEL_ELF_GB10X: &str = "kernel_gb10x.elf";

    /// libos subsystems (from source paths in firmware strings).
    pub const SUBSYS_MM: &str          = "mm";          // Memory management
    pub const SUBSYS_SCHED: &str       = "sched";       // Scheduler (ports, heartbeat)
    pub const SUBSYS_LOADER: &str      = "loader";      // Firmware loader
    pub const SUBSYS_SERVER: &str      = "server";      // RPC server
    pub const SUBSYS_PARTITION: &str   = "partition";    // vGPU partitions
    pub const SUBSYS_IPI: &str         = "ipi";         // Inter-processor interrupt
    pub const SUBSYS_DMA: &str         = "dma";         // DMA driver
    pub const SUBSYS_EXTINTR: &str     = "extintr-v2";  // External interrupt v2
    pub const SUBSYS_GDMA: &str        = "gdma";        // GPU DMA

    /// libos IPI message protocol constants (from firmware assertions).
    /// "((NvU64)header->kind) < IpiMessageNull"
    /// "header->size <= (LIBOS_CONFIG_MESSAGE_PAGE_SIZE - readingPageOffset ...)"
    pub const CONFIG_MESSAGE_PAGE_SIZE: usize = 4096;
    pub const CONFIG_ROOT_PARTITION_ID: u32   = 0;
    /// "vaBase <= LIBOS_CONFIG_IDENTITY_MAPS_END"
    pub const CONFIG_IDENTITY_MAPS_END: u64   = 0xFFFF_FFFF;
}

/// Firmware memory layout info from GSP string analysis.
/// "FB offset %llx fwWprStart %llx" — firmware lives in WPR (Write Protected Region).
pub mod gsp_memory {
    /// The GSP firmware is loaded into a Write-Protected Region in VRAM.
    /// The WPR prevents host CPU from reading/writing the GSP firmware once locked.
    pub const WPR_ALIGNMENT: u64 = 0x20000; // 128 KB alignment (typical)
}

// ---------------------------------------------------------------------------
// GSP-RM RPC Protocol
// ---------------------------------------------------------------------------
// The host driver communicates with the running GSP firmware via
// shared-memory RPC (Ring Producer/Consumer). Messages follow a
// header + payload format.

/// GSP RPC message header — sent via shared memory to/from GSP.
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct GspRpcHeader {
    /// Message type / command ID.
    pub msg_type: u32,
    /// Payload size in bytes (follows this header).
    pub payload_size: u32,
    /// Sequence number for request/response matching.
    pub sequence: u32,
    /// Status / return code (set by GSP in responses).
    pub status: u32,
}

/// Known GSP RPC message types (from open-gpu-kernel-modules).
pub mod gsp_rpc {
    /// Initialize the Resource Manager on GSP.
    pub const MSG_INIT: u32 = 0x0001;
    /// GPU engine information query.
    pub const MSG_GPU_INFO: u32 = 0x0002;
    /// Allocate a GPU object (channel, memory, etc.).
    pub const MSG_ALLOC: u32 = 0x0003;
    /// Free a GPU object.
    pub const MSG_FREE: u32 = 0x0004;
    /// Control call (IOCTL-like) to a GPU object.
    pub const MSG_CONTROL: u32 = 0x0005;
    /// Display mode set.
    pub const MSG_DISPLAY: u32 = 0x0010;
    /// Power state change.
    pub const MSG_POWER: u32 = 0x0020;
    /// Event notification from GSP to host.
    pub const MSG_EVENT: u32 = 0x0100;
    /// Heartbeat / keepalive.
    pub const MSG_HEARTBEAT: u32 = 0xFFFF;
}

/// GSP shared memory layout for command ring.
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct GspCmdRing {
    /// Physical address of the ring buffer.
    pub ring_phys: u64,
    /// Size of ring buffer in bytes.
    pub ring_size: u32,
    /// Producer index (written by host).
    pub put: u32,
    /// Consumer index (written by GSP).
    pub get: u32,
    /// Reserved / alignment.
    pub _pad: u32,
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

// ---------------------------------------------------------------------------
// GSP Boot Sequence (Ampere GA106)
// ---------------------------------------------------------------------------
// Full GSP boot requires:
// 1. Load GSP-RM ELF into VRAM via DMA
// 2. Configure GSP FALCON bootstrap (SEC2 → GSP handoff)
// 3. Set up shared memory regions (command ring, status page)
// 4. Trigger GSP boot via FALCON CPUCTL
// 5. Wait for GSP handshake (SCRATCH register or shared memory flag)
// 6. Send MSG_INIT RPC to initialize Resource Manager
// 7. GSP responds with GPU capabilities and engine info

/// Boot parameters passed to GSP firmware via shared memory.
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct GspBootParams {
    /// Magic value for validation ("GSPB" = 0x42505347).
    pub magic: u32,
    /// Boot param structure version.
    pub version: u32,
    /// Physical address of GSP-RM ELF image in VRAM.
    pub fw_phys: u64,
    /// Size of GSP-RM ELF image.
    pub fw_size: u32,
    /// Physical address of command ring buffer.
    pub cmd_ring_phys: u64,
    /// Physical address of status/response ring.
    pub status_ring_phys: u64,
    /// Physical address of shared scratch region.
    pub scratch_phys: u64,
    /// GPU chip ID (0x2504 for GA106).
    pub chip_id: u32,
    /// VRAM size in bytes.
    pub vram_size: u64,
}

impl GspBootParams {
    /// Magic value: "GSPB" in little-endian.
    pub const MAGIC: u32 = 0x4250_5347;
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

    #[test]
    fn gsp_container_elf_detect() {
        // Minimal ELF64 RISC-V header
        let mut blob = [0u8; 64];
        blob[0] = 0x7F; blob[1] = b'E'; blob[2] = b'L'; blob[3] = b'F';
        blob[4] = 2; // ELF64
        blob[5] = 1; // Little-endian
        blob[18] = 243; blob[19] = 0; // EM_RISCV
        blob[24] = 0x00; blob[25] = 0x10; // entry = 0x1000

        let c = parse_gsp_container(&blob).unwrap();
        assert!(c.is_elf);
        assert_eq!(c.elf_class, 64);
        assert_eq!(c.elf_machine, 243);
        assert!(is_gsp_riscv(&c));
    }

    #[test]
    fn gsp_boot_params_magic() {
        assert_eq!(GspBootParams::MAGIC, 0x4250_5347);
    }
}
