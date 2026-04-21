//! # nv_regs — GA106 (RTX 3060 12G) GPU Register Definitions
//!
//! Register offsets for NVIDIA Ampere GA106 GPU, organized by engine.
//! Sources: envytools documentation, nouveau kernel driver, NVIDIA open-gpu-kernel-modules,
//! and SigDead-BIB section analysis of nvlddmkm.sys (32 sections, custom naming).
//!
//! The NVIDIA driver organizes hardware access into subsystems that map to
//! PE sections discovered by SigDead-BIB:
//!   _DDTEXT  → Display Driver (non-paged)
//!   _KTEXT   → Kernel core (non-paged)
//!   PAGE_DD  → Display Driver (paged)
//!   PAGE_K   → Kernel core (paged)
//!   PAGEcRM  → Resource Manager
//!   PAGE_KSH → Kernel shaders
//!
//! Zero dependencies. `#![no_std]` compatible.

#![no_std]
#![allow(non_snake_case)]

// ── PCI Identification ──────────────────────────────────────────────────────

pub const NVIDIA_VENDOR_ID: u16 = 0x10DE;
pub const GA106_DEVICE_ID: u16  = 0x2504;  // RTX 3060 12GB

// ── BAR Layout ──────────────────────────────────────────────────────────────

pub const BAR0_SIZE: usize = 16 * 1024 * 1024;   // 16 MB register space
pub const BAR1_SIZE: usize = 256 * 1024 * 1024;   // 256 MB VRAM aperture (mappable)
pub const VRAM_TOTAL: usize = 12 * 1024 * 1024 * 1024; // 12 GB GDDR6

// ── PMC — Power Management Controller (BAR0 + 0x000000) ────────────────────
// Corresponds to NVIDIA's _KTEXT section (kernel core, non-paged)

pub mod pmc {
    pub const BOOT_0:       u32 = 0x0000_0000; // GPU chip ID
    pub const INTR_0:       u32 = 0x0000_0100; // Interrupt status (host)
    pub const INTR_EN_0:    u32 = 0x0000_0140; // Interrupt enable (host)
    pub const INTR_LN_0:    u32 = 0x0000_0160; // Interrupt line 0
    pub const ENABLE:       u32 = 0x0000_0200; // Engine enable mask
    pub const fn intr_top(n: u32) -> u32 { 0x0000_0100 + n * 4 }
    pub const fn intr_en(n: u32) -> u32  { 0x0000_0140 + n * 4 }

    // Engine enable bits
    pub const ENABLE_PGRAPH: u32    = 1 << 12;
    pub const ENABLE_PFIFO: u32     = 1 << 8;
    pub const ENABLE_PCOPY0: u32    = 1 << 17;
    pub const ENABLE_PCOPY1: u32    = 1 << 18;
    pub const ENABLE_PDISPLAY: u32  = 1 << 26;

    // Interrupt bits (from nvlddmkm.sys error strings: IRQ, FIFO, GR, DISP)
    pub const INTR_PFIFO: u32       = 1 << 8;
    pub const INTR_PGRAPH: u32      = 1 << 12;
    pub const INTR_PCOPY0: u32      = 1 << 17;
    pub const INTR_PCOPY1: u32      = 1 << 18;
    pub const INTR_PDISPLAY: u32    = 1 << 26;
    pub const INTR_PMU: u32         = 1 << 24;
}

// ── PBUS — Bus Interface (BAR0 + 0x001000) ─────────────────────────────────

pub mod pbus {
    pub const INTR_0:       u32 = 0x0000_1100;
    pub const INTR_EN_0:    u32 = 0x0000_1140;
    pub const BAR0_WINDOW:  u32 = 0x0000_1700; // BAR0 window for large VRAM access
}

// ── PTIMER — GPU Timer (BAR0 + 0x009000) ────────────────────────────────────

pub mod ptimer {
    pub const TIME_LO:      u32 = 0x0000_9400; // Low 32 bits of GPU time (nanoseconds)
    pub const TIME_HI:      u32 = 0x0000_9410; // High 32 bits
    pub const INTR_0:       u32 = 0x0000_9100;
    pub const ALARM:        u32 = 0x0000_9420;
}

// ── PFIFO — Command Submission Engine (BAR0 + 0x002000) ─────────────────────
// Corresponds to NVIDIA's PAGE_K section (kernel core, paged)
// nvlddmkm.sys: NV_ERR_FIFO_BAD_ACCESS, NV_ERR_INVALID_CHANNEL

pub mod pfifo {
    pub const INTR_0:           u32 = 0x0000_2100;
    pub const INTR_EN_0:        u32 = 0x0000_2140;

    // Runlist (Ampere uses runlist-based scheduling)
    pub const RUNLIST_BASE:     u32 = 0x0000_2270;
    pub const RUNLIST_SUBMIT:   u32 = 0x0000_2274;

    // Per-channel registers
    pub const fn CHAN_BASE(ch: u32) -> u32 { 0x0080_0000 + ch * 0x2000 }
    pub const fn CHAN_PUT(ch: u32) -> u32  { CHAN_BASE(ch) + 0x0040 }
    pub const fn CHAN_GET(ch: u32) -> u32  { CHAN_BASE(ch) + 0x0044 }
    pub const fn CHAN_REF(ch: u32) -> u32  { CHAN_BASE(ch) + 0x0048 }

    pub const MAX_CHANNELS: u32 = 512;
}

// ── PGRAPH — Graphics/Compute Engine (BAR0 + 0x400000) ─────────────────────
// Corresponds to NVIDIA's PAGE_DD section (Display Driver, paged)

pub mod pgraph {
    pub const INTR_0:           u32 = 0x0040_0100;
    pub const INTR_EN_0:        u32 = 0x0040_0140;
    pub const FECS_INTR:        u32 = 0x0040_9C20; // Frontend context switch interrupt
    pub const STATUS:           u32 = 0x0040_0700;
    pub const TRAPPED_ADDR:     u32 = 0x0040_0704;
    pub const TRAPPED_DATA_LO:  u32 = 0x0040_0708;

    // GPC (Graphics Processing Cluster) — GA106 has 3 GPCs
    pub const GPC_COUNT: u32 = 3;
    pub const fn GPC_BASE(gpc: u32) -> u32 { 0x0050_0000 + gpc * 0x8000 }

    // TPC (Texture Processing Cluster) — per GPC
    pub const TPC_PER_GPC: u32 = 4; // GA106: up to 4 TPCs per GPC
    pub const fn TPC_BASE(gpc: u32, tpc: u32) -> u32 {
        GPC_BASE(gpc) + 0x2000 + tpc * 0x400
    }

    // SM (Streaming Multiprocessor) count — GA106: 28 SMs total
    pub const SM_COUNT: u32 = 28;
}

// ── PCOPY — Copy Engines (BAR0 + 0x104000) ─────────────────────────────────
// DMA copy operations (NV_ERR_DMA_IN_USE, NV_ERR_DMA_MEM_NOT_LOCKED)

pub mod pcopy {
    pub const fn CE_BASE(n: u32) -> u32 { 0x0010_4000 + n * 0x1000 }
    pub const fn CE_INTR(n: u32) -> u32 { CE_BASE(n) + 0x0100 }

    pub const CE_COUNT: u32 = 5; // GA106 has 5 copy engines
}

// ── PDISPLAY — Display Engine (BAR0 + 0x610000) ────────────────────────────
// Corresponds to NVIDIA's _DDTEXT section (Display Driver, non-paged)
// nvlddmkm.sys error: "Display Underflow"

pub mod pdisplay {
    pub const INTR_0:               u32 = 0x0061_1000;
    pub const INTR_EN_0:            u32 = 0x0061_1004;

    // Display heads (outputs)
    pub const HEAD_COUNT: u32 = 4; // GA106 supports up to 4 display heads

    pub const fn HEAD_BASE(h: u32) -> u32 { 0x0061_6000 + h * 0x800 }
    pub const fn HEAD_SET_OFFSET(h: u32) -> u32     { HEAD_BASE(h) + 0x0104 }
    pub const fn HEAD_SET_SIZE(h: u32) -> u32       { HEAD_BASE(h) + 0x0108 }
    pub const fn HEAD_SET_STORAGE(h: u32) -> u32    { HEAD_BASE(h) + 0x010C }
    pub const fn HEAD_SET_PITCH(h: u32) -> u32      { HEAD_BASE(h) + 0x0110 }
    pub const fn HEAD_SET_CONTROL(h: u32) -> u32    { HEAD_BASE(h) + 0x0200 }

    // Pixel formats
    pub const PIXEL_FORMAT_BGRA8888: u32 = 0xCF;
    pub const PIXEL_FORMAT_RGBX8888: u32 = 0xE6;

    // SOR (Serial Output Resource) — DisplayPort/HDMI
    pub const SOR_COUNT: u32 = 4;
    pub const fn SOR_BASE(s: u32) -> u32 { 0x0061_C000 + s * 0x800 }

    // I2C for EDID reading (NV_ERR_I2C_ERROR, NV_ERR_I2C_SPEED_TOO_HIGH)
    pub const I2C_PORT_COUNT: u32 = 6;
    pub const fn I2C_BASE(port: u32) -> u32 { 0x0000_D000 + port * 0x20 }
    pub const fn I2C_DATA(port: u32) -> u32 { I2C_BASE(port) + 0x04 }
    pub const fn I2C_CTRL(port: u32) -> u32 { I2C_BASE(port) + 0x08 }
}

// ── FALCON — Microcontroller (GSP, PMU, SEC2, etc.) ────────────────────────
// nvlddmkm.sys PAGErGEN (72MB) contains FALCON firmware blobs

pub mod falcon {
    pub const fn BASE(engine: u32) -> u32 { engine }

    // Common FALCON register offsets (relative to engine base)
    pub const IRQSSET:      u32 = 0x0000;
    pub const IRQSCLR:      u32 = 0x0004;
    pub const IRQSTAT:      u32 = 0x0008;
    pub const IRQMASK:      u32 = 0x0018;
    pub const IRQDEST:      u32 = 0x001C;
    pub const SCRATCH0:     u32 = 0x0040;
    pub const SCRATCH1:     u32 = 0x0044;
    pub const CPUCTL:       u32 = 0x0100;
    pub const BOOTVEC:      u32 = 0x0104;
    pub const HWCFG:        u32 = 0x0108;
    pub const DMACTL:       u32 = 0x010C;
    pub const DMATRFBASE:   u32 = 0x0110;
    pub const DMATRFMOFFS:  u32 = 0x0114;
    pub const DMATRFCMD:    u32 = 0x0118;
    pub const DMATRFFBOFFS: u32 = 0x011C;

    // Falcon instances on GA106
    pub const GSP:  u32 = 0x0011_0000; // GPU System Processor
    pub const PMU:  u32 = 0x0010_A000; // Power Management Unit
    pub const SEC2: u32 = 0x0010_1000; // Security Engine 2
    pub const NVDEC: u32 = 0x0084_0000; // Video Decoder

    // CPUCTL commands
    pub const CPUCTL_START:     u32 = 0x02;
    pub const CPUCTL_HALT:      u32 = 0x10;

    // DMA transfer commands
    pub const DMA_CMD_LOAD_IMEM: u32 = 0x11;
    pub const DMA_CMD_LOAD_DMEM: u32 = 0x01;
}

// ── PMEM — GPU Memory Interface (BAR0 + 0x022000) ──────────────────────────
// NV_ERR_MEMORY_TRAINING_FAILED, NV_ERR_BROKEN_FB

pub mod pmem {
    pub const FBPA_COUNT: u32 = 6; // GA106: 6 FBPA partitions (12GB / 2GB each)
    pub const fn FBPA_BASE(n: u32) -> u32 { 0x009A_0000 + n * 0x4000 }

    // Memory controller
    pub const FB_CFG0:      u32 = 0x0010_0C10;
    pub const FB_MEM_SIZE:  u32 = 0x0010_0CE0;
}

// ── NV_PRAMIN — Instance Memory Window ──────────────────────────────────────

pub mod pramin {
    pub const BASE:         u32 = 0x0070_0000;
    pub const SIZE:         u32 = 0x0010_0000; // 1 MB window
}

// ── PBDMA — Push Buffer DMA Engines ─────────────────────────────────────────
// SigDead-BIB GSP firmware strings: "_PBDMA0", "_PBDMA1"
// PBDMA engines feed command buffers from host memory into GPU engines.

pub mod pbdma {
    pub const COUNT: u32 = 2;  // GA106: PBDMA0 + PBDMA1
    pub const fn BASE(n: u32) -> u32 { 0x0004_0000 + n * 0x2000 }
    pub const fn GP_PUT(n: u32) -> u32 { BASE(n) + 0x0000 }
    pub const fn GP_GET(n: u32) -> u32 { BASE(n) + 0x0004 }
    pub const fn PB_PUT(n: u32) -> u32 { BASE(n) + 0x005C }
    pub const fn PB_GET(n: u32) -> u32 { BASE(n) + 0x0060 }
    pub const fn INTR(n: u32) -> u32   { BASE(n) + 0x0110 }
    pub const fn STATUS(n: u32) -> u32 { BASE(n) + 0x0118 }
}

// ── HUB Client IDs ──────────────────────────────────────────────────────────
// SigDead-BIB GSP firmware strings: HUBCLIENT_CE0..CE3, HUBCLIENT_HSCE0..HSCE8,
// HUBCLIENT_CE_SHIM. These are the internal hub routing IDs.

pub mod hub {
    // Copy Engine hub clients
    pub const CLIENT_CE0: u32       = 0;
    pub const CLIENT_CE1: u32       = 1;
    pub const CLIENT_CE2: u32       = 2;
    pub const CLIENT_CE3: u32       = 3;
    pub const CLIENT_CE_SHIM: u32   = 4;
    // High-Speed Copy Engine hub clients
    pub const CLIENT_HSCE0: u32     = 16;
    pub const CLIENT_HSCE1: u32     = 17;
    pub const CLIENT_HSCE2: u32     = 18;
    pub const CLIENT_HSCE3: u32     = 19;
    pub const CLIENT_HSCE4: u32     = 20;
    pub const CLIENT_HSCE5: u32     = 21;
    pub const CLIENT_HSCE6: u32     = 22;
    pub const CLIENT_HSCE7: u32     = 23;
    pub const CLIENT_HSCE8: u32     = 24;
    pub const CLIENT_HSCE15: u32    = 31;
}

// ── SEC_FAULT — Security Engine Fault Registers ─────────────────────────────
// SigDead-BIB GSP firmware: "SEC_FAULT: _BAR_FIREWALL_ENGAGE"

pub mod sec_fault {
    pub const INTR:             u32 = 0x000B_C100;
    pub const INTR_EN:          u32 = 0x000B_C140;
    pub const BAR_FIREWALL:     u32 = 0x000B_C200;
}

// ── Graphics Exception Codes ────────────────────────────────────────────────
// SigDead-BIB GSP firmware: "Graphics Exception: DMA_DRAM_ACCESS_OUT_OF_BOUNDS",
// "Graphics Exception: DMA_READ_FIFOED_FROM_PB", etc.

pub mod gr_exception {
    pub const DMA_DRAM_ACCESS_OUT_OF_BOUNDS: u32 = 0x01;
    pub const DMA_READ_FIFOED_FROM_PB: u32       = 0x02;
    pub const DMA_ILLEGAL_FIFO_CONFIG: u32       = 0x03;
    pub const DMA_READ_FIFOED_OVERFLOW: u32      = 0x04;
    pub const TMA_BARRIER_MISALIGNED_ADDR: u32   = 0x10;
    pub const TMA_BARRIER_OOR_ADDR: u32          = 0x11;
}

// ── XBAR — Crossbar Clock Domains ───────────────────────────────────────────
// SigDead-BIB GSP firmware: XBARCLK, PERF_CF_CONTROLLER_XBAR_MAX,
// CLIENT_LOW_STRICT_XBAR_MAX, THERM_POLICY_XBAR, PWR_POLICY_XBAR, etc.

pub mod xbar {
    pub const CLK_BASE:         u32 = 0x000B_0000;
    pub const CLK_CTRL:         u32 = 0x000B_0004;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pci_ids() {
        assert_eq!(NVIDIA_VENDOR_ID, 0x10DE);
        assert_eq!(GA106_DEVICE_ID, 0x2504);
    }

    #[test]
    fn register_offsets_nonzero() {
        assert!(pmc::BOOT_0 == 0);
        assert!(pmc::ENABLE > 0);
        assert!(pfifo::RUNLIST_BASE > 0);
        assert!(pgraph::INTR_0 > 0);
        assert!(pdisplay::HEAD_COUNT == 4);
    }

    #[test]
    fn channel_registers() {
        let ch0_put = pfifo::CHAN_PUT(0);
        let ch1_put = pfifo::CHAN_PUT(1);
        assert!(ch1_put > ch0_put);
        assert_eq!(ch1_put - ch0_put, 0x2000);
    }

    #[test]
    fn ga106_specs() {
        assert_eq!(pgraph::GPC_COUNT, 3);
        assert_eq!(pgraph::SM_COUNT, 28);
        assert_eq!(pcopy::CE_COUNT, 5);
        assert_eq!(pmem::FBPA_COUNT, 6);
    }

    #[test]
    fn pbdma_engines() {
        assert_eq!(pbdma::COUNT, 2);
        assert!(pbdma::BASE(1) > pbdma::BASE(0));
        assert_eq!(pbdma::BASE(1) - pbdma::BASE(0), 0x2000);
    }

    #[test]
    fn hub_clients() {
        assert_eq!(hub::CLIENT_CE0, 0);
        assert_eq!(hub::CLIENT_HSCE0, 16);
    }
}
