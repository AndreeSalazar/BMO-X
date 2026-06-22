//! `vendor/amd/gpu/rdna4/regs.rs` — RDNA4 MMIO register offsets.
//!
//! v1.8.8: skeleton. Defines the offsets of key MMIO registers in the
//! RDNA4 GPU. The full register map is defined in AMD's NDA BKDG for
//! Navi 4x; here we only define the registers we use from Ring 0.

#![allow(dead_code)]

/// BIF (Bus Interface) registers — PCIe config space mirrors.
/// Offset from MMIO base (BIF_BX0).
pub const BIF_BX0_OFFSET: u32 = 0x0000_0000;

/// VGA registers (legacy compatibility).
pub const VGA_OFFSET: u32 = 0x0000_0000;

/// D1F0: device config / BAR registers.
pub const D1F0_OFFSET: u32 = 0x0000_0000;

/// D1F2: PCI Express capability.
pub const D1F2_OFFSET: u32 = 0x0000_4000;

/// GRBM (Graphics Register Bus Manager) — controls engine state.
pub const GRBM_OFFSET: u32 = 0x0000_8000;

/// SRBM (System Register Bus Manager) — controls system-level state.
pub const SRBM_OFFSET: u32 = 0x0000_8400;

/// SDMA (System DMA) registers.
pub const SDMA0_OFFSET: u32 = 0x0000_A000;

/// GFX (Graphics Compute) ring registers.
pub const GFX_RING0_OFFSET: u32 = 0x0000_C000;

/// MMHUB (Memory Management Hub) registers.
pub const MMHUB_OFFSET: u32 = 0x0001_A000;

/// VCN (Video Core Next) — for video encode/decode.
pub const VCN0_OFFSET: u32 = 0x0001_2000;

/// RLC (Run Length Coding) — power management.
pub const RLC_OFFSET: u32 = 0x0001_C000;

//
// Within the GRBM block:
//
/// GRBM_GFX_CNTL — Graphics engine control.
pub const REG_GRBM_GFX_CNTL: u32 = GRBM_OFFSET + 0x0000;
/// GRBM_STATUS — current graphics engine status.
pub const REG_GRBM_STATUS: u32 = GRBM_OFFSET + 0x8010;

//
// Within the GFX ring block:
//
/// GFX_RING_BASE — ring buffer base address (LO).
pub const REG_GFX_RING_BASE_LO: u32 = GFX_RING0_OFFSET + 0x002C;
/// GFX_RING_BASE — ring buffer base address (HI).
pub const REG_GFX_RING_BASE_HI: u32 = GFX_RING0_OFFSET + 0x0030;
/// GFX_RING_SIZE — ring buffer size in dwords.
pub const REG_GFX_RING_SIZE: u32 = GFX_RING0_OFFSET + 0x0034;
/// GFX_RING_CNTL — ring buffer control.
pub const REG_GFX_RING_CNTL: u32 = GFX_RING0_OFFSET + 0x0038;
/// GFX_RING_WPT — write pointer.
pub const REG_GFX_RING_WPT: u32 = GFX_RING0_OFFSET + 0x003C;
/// GFX_RING_RPT — read pointer.
pub const REG_GFX_RING_RPT: u32 = GFX_RING0_OFFSET + 0x0040;
/// GFX_RING_DOORBELL — doorbell index for the ring.
pub const REG_GFX_RING_DOORBELL: u32 = GFX_RING0_OFFSET + 0x0208;

//
// Fence registers:
//
/// GFX_FENCE — graphics fence (64-bit address + value).
pub const REG_GFX_FENCE_GFX: u32 = GFX_RING0_OFFSET + 0x0900;
