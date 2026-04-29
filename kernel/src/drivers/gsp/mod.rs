//! GSP (Graphics System Processor) driver for GA106.
//!
//! Handles GSP FALCON bootstrap, scratch register access,
//! and GSP-RM communication.
//!
//! ## Init Sequence (unified)
//!
//! ```text
//! gsp_init(bar0, fw_blob, console)
//!   +-- [1] PRIV Ring init (priv_ring::PrivRingInit::init)
//!   |      +-- start ring -> wait ready -> clear IRQ -> PMC enable -> Falcon reset -> verify
//!   +-- [2] Page allocator: alloc contiguous DMA buffer (arch::page_alloc)
//!   +-- [3] Copy firmware ELF to DMA buffer
//!   +-- [4] DMA transfer -> Falcon DMEM (256-byte blocks)
//!   +-- [5] Set boot vector + start Falcon CPU
//!   +-- [6] Wait GSP-RM handshake (MAILBOX0/1)
//! ```

pub mod scratch;
pub mod loader;
pub mod priv_ring;
pub mod boot_args;
pub mod elf_parser;
pub mod rpc;
pub mod gmmu;
pub mod nv_rm;
pub mod disp;
pub mod pushbuffer;
pub mod radix3;

use crate::console::Console;
pub use loader::{GspLoader, GspLoadError};
pub use priv_ring::{PrivRingInit, PrivRingError};

/// GSP firmware blobs needed for GA10x boot.
///
/// For Ampere (GA10x), the boot requires 3 separate firmware files:
/// - `gsp_rm`: The full GSP-RM ELF (~69MB) loaded via Radix3 into VRAM
/// - `bootloader`: RISC-V bootloader (~20KB) with nvfw_bin_hdr magic=0x10de
/// - `booter_load`: Falcon HS booter (~60KB) that sets up WPR2/ACR
pub struct GspFirmwareBlobs<'a> {
    pub gsp_rm: &'a [u8],
    pub bootloader: &'a [u8],
    pub booter_load: &'a [u8],
    /// Optional VBIOS / FWSEC blob for pre-boot WPR2 setup via SEC2
    pub vbios_rom: Option<&'a [u8]>,
}

/// One-call GSP initialization: PRIV Ring + firmware load + boot.
///
/// This is the top-level entry point that kernel `main.rs` should call.
/// It creates a `GspLoader` and runs the full boot sequence.
///
/// # Arguments
/// * `bar0` - Mapped BAR0 MMIO region for the GPU.
/// * `fw_blob` - Raw bytes of `gsp_ga10x.bin` (RISC-V ELF, ~70 MB).
/// * `con` - Console for diagnostic output.
///
/// # Returns
/// `Ok(())` on success, or `GspLoadError` on failure.
pub fn gsp_init(
    bar0: &nv_hal::MmioRegion,
    fw_blob: &[u8],
    con: &mut Console,
) -> Result<(), GspLoadError> {
    let loader = GspLoader::new(bar0);
    loader.load(fw_blob, con)
}

/// Full GSP initialization with all 3 firmware blobs (proper GA10x boot).
pub fn gsp_init_full(
    bar0: &nv_hal::MmioRegion,
    blobs: &GspFirmwareBlobs,
    con: &mut Console,
) -> Result<(), GspLoadError> {
    let loader = GspLoader::new(bar0);
    loader.load_full(blobs, con)
}
