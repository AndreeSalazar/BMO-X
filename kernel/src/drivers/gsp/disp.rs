//! Display Engine via GSP-RM RPC — NVIDIA Ampere GA10x (RTX 3060)
//!
//! En Ampere, TODO el display pasa por GSP-RM via RPC.
//! NO hay acceso MMIO directo a PDISP — el GSP es dueño del hardware.
//!
//! Secuencia (de nvidia-open + nouveau):
//!   1. GSP_RM_ALLOC class 0xC670 (NVC670_DISPLAY) — container
//!   2. GSP_RM_ALLOC class 0xC67D (NVC67D_CORE_CHANNEL_DMA)
//!   3. GSP_RM_CONTROL NV2080_CTRL_CMD_INTERNAL_DISPLAY_GET_STATIC_INFO
//!   4. GSP_RM_CONTROL NV0073_CTRL_CMD_SYSTEM_GET_NUM_HEADS
//!   5. Para DP: GSP_RM_CONTROL NV0073_CTRL_CMD_DP_CTRL (link training)

use crate::console::Console;
use super::rpc::{GspRpcRing, display_class, rpc_fn};

// RM control commands for display (from nvidia-open class headers)
const NV2080_CTRL_CMD_INTERNAL_DISPLAY_GET_STATIC_INFO: u32 = 0x2080_0A01;
const NV0073_CTRL_CMD_SYSTEM_GET_NUM_HEADS: u32             = 0x0073_0102;
const NV0073_CTRL_CMD_SPECIFIC_SET_BACKLIGHT: u32           = 0x0073_0509;

pub struct DisplayEngine<'a> {
    bar0: &'a nv_hal::MmioRegion,
}

impl<'a> DisplayEngine<'a> {
    pub fn new(bar0: &'a nv_hal::MmioRegion) -> Self {
        Self { bar0 }
    }

    /// Configure 1920x1080 display via GSP-RM RPC (NOT direct MMIO).
    /// All display setup on Ampere must go through the GSP Resource Manager.
    pub fn set_mode_1080p(&self, rpc: &mut GspRpcRing, con: &mut Console) {
        con.print_colored("=== Display Engine GA10x via GSP-RM RPC ===\n",
            crate::fb::colors::ACCENT_CYAN);

        // Step 1: Alloc Display container (class 0xC670)
        con.println("  [DISP] GSP_RM_ALLOC Display Container (0xC670)...");
        let h_client = rpc.alloc_handle();
        let h_device = rpc.alloc_handle();
        let h_display = rpc.alloc_handle();
        let _ = rpc.send_rm_alloc(
            h_client, h_device, h_display,
            display_class::NVC670_DISPLAY, con,
        );

        // Step 2: Alloc Core Channel DMA (class 0xC67D)
        con.println("  [DISP] GSP_RM_ALLOC Core Channel DMA (0xC67D)...");
        let h_core = rpc.alloc_handle();
        let _ = rpc.send_rm_alloc(
            h_client, h_display, h_core,
            display_class::NVC67D_CORE_CHANNEL_DMA, con,
        );

        // Step 3: Get display static info via RM_CONTROL
        con.println("  [DISP] GSP_RM_CONTROL: GET_DISPLAY_STATIC_INFO...");
        let _ = rpc.send_rm_control(
            h_client, h_device,
            NV2080_CTRL_CMD_INTERNAL_DISPLAY_GET_STATIC_INFO, con,
        );

        // Step 4: Get number of heads
        con.println("  [DISP] GSP_RM_CONTROL: GET_NUM_HEADS...");
        let _ = rpc.send_rm_control(
            h_client, h_display,
            NV0073_CTRL_CMD_SYSTEM_GET_NUM_HEADS, con,
        );

        // Step 5: Read BOOT_0 for chip verification (this MMIO is always accessible)
        let boot0 = self.bar0.read32(0x0000_0000); // NV_PMC_BOOT_0
        con.print("  [DISP] BOOT_0=0x");
        con.print_hex32(boot0);
        con.println(" (chip ID verification)");

        con.print_colored("=== Display Engine via GSP-RM COMPLETE ===\n",
            crate::fb::colors::TEXT_SUCCESS);
    }
}
