//! `vendor/amd/gpu/rdna4/` — RDNA4 (Navi 4x) GPU driver skeleton.
//!
//! v1.8.8: minimal scaffold. Will be expanded with the actual driver:
//! - ring buffer management
//! - command submission
//! - fence synchronization
//! - VRAM aperture setup
//! - power management
//!
//! Submodules (planned):
//! - `pci.rs`        ← vendor/device ID, BARs (✅ done)
//! - `regs.rs`       ← MMIO register offsets (✅ done)
//! - `mmio.rs`       ← read/write helpers (✅ done)
//! - `vram.rs`       ← VRAM aperture (TODO)
//! - `rings.rs`      ← GFX/compute/SDMA rings (TODO)
//! - `fences.rs`     ← fence sync (TODO)
//! - `irq.rs`        ← GPU interrupts (TODO)
//! - `dma.rs`        ← DMA buffers (TODO)
//! - `power.rs`      ← power/clocks (TODO)
//! - `device.rs`     ← Rdna4Device struct (✅ done)

#![allow(dead_code)]

pub mod pci;
pub mod regs;
pub mod mmio;
pub mod device;

// Subsystem that re-exports the public types of this driver.
pub use pci::{is_rDNA4, device_name, RDNA4_DEVICE_IDS, PCI_VENDOR_ID_AMD};
pub use device::{Rdna4Device, first_active, MAX_RDNA4_DEVICES};
