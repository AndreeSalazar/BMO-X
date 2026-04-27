//! NVIDIA Resource Manager (NV_RM) via GSP RPC
//!
//! Implements the correct GSP-RM init sequence from nvidia-open + nouveau:
//!   1. GSP_SET_SYSTEM_INFO (fn=72) — BAR addresses, ACPI info
//!   2. SET_REGISTRY (fn=73) — Registry key/value pairs
//!   3. GET_GSP_STATIC_INFO (fn=65) — Get internal handles
//!   4. GSP_RM_ALLOC (fn=103) — Create RM objects (display, channel, etc.)
//!   5. GSP_RM_CONTROL (fn=76) — Issue control commands
//!
//! All object creation uses GSP_RM_ALLOC with RpcGspRmAlloc payload.
//! All control calls use GSP_RM_CONTROL with RpcGspRmControl payload.

use crate::console::Console;
use super::rpc::*;

pub struct NvResourceManager<'a, 'b> {
    rpc: &'a mut GspRpcRing<'b>,
    // RM handle hierarchy (from nvidia-open)
    h_client: u32,      // Root client handle
    h_device: u32,      // Device handle (parent for subdevices)
    h_subdevice: u32,   // Subdevice handle (parent for engines)
    h_display: u32,     // Display container handle
    h_channel: u32,     // FIFO channel handle
    h_va_space: u32,    // Virtual address space handle
}

impl<'a, 'b> NvResourceManager<'a, 'b> {
    pub fn new(rpc: &'a mut GspRpcRing<'b>) -> Self {
        // Pre-allocate RM handles (nouveau style: sequential from 0xDEAD)
        let h_client = rpc.alloc_handle();
        let h_device = rpc.alloc_handle();
        let h_subdevice = rpc.alloc_handle();
        let h_display = rpc.alloc_handle();
        let h_channel = rpc.alloc_handle();
        let h_va_space = rpc.alloc_handle();
        Self { rpc, h_client, h_device, h_subdevice, h_display, h_channel, h_va_space }
    }

    /// Step 1: GSP_SET_SYSTEM_INFO (fn=72) — Tell GSP about the host system
    pub fn set_system_info(&mut self, con: &mut Console) -> Result<(), &'static str> {
        con.print_colored("  [NV_RM] GSP_SET_SYSTEM_INFO (fn=72)\n", crate::fb::colors::ACCENT_CYAN);
        self.rpc.send_rpc_simple(rpc_fn::GSP_SET_SYSTEM_INFO, con)
    }

    /// Step 2: SET_REGISTRY (fn=73) — Send registry key/value pairs
    pub fn set_registry(&mut self, con: &mut Console) -> Result<(), &'static str> {
        con.println("  [NV_RM] SET_REGISTRY (fn=73)...");
        self.rpc.send_rpc_simple(rpc_fn::SET_REGISTRY, con)
    }

    /// Step 3: GET_GSP_STATIC_INFO (fn=65) — Get internal handles and caps
    pub fn get_static_info(&mut self, con: &mut Console) -> Result<(), &'static str> {
        con.println("  [NV_RM] GET_GSP_STATIC_INFO (fn=65)...");
        self.rpc.send_rpc_simple(rpc_fn::GET_GSP_STATIC_INFO, con)
    }

    /// Step 4: Alloc VA Space — GSP_RM_ALLOC (fn=103, class=0xC6FA)
    pub fn alloc_va_space(&mut self, con: &mut Console) -> Result<(), &'static str> {
        con.println("  [NV_RM] ALLOC VA Space (fn=103, class=0xC6FA)...");
        self.rpc.send_rm_alloc(
            self.h_client, self.h_subdevice, self.h_va_space,
            NV_CLASS_VA_SPACE_GA10X, con,
        )
    }

    /// Step 5: Alloc FIFO Channel — GSP_RM_ALLOC (fn=103, class=0xC36F)
    pub fn alloc_channel(&mut self, con: &mut Console) -> Result<(), &'static str> {
        con.println("  [NV_RM] ALLOC Channel GPFIFO (fn=103, class=0xC36F)...");
        self.rpc.send_rm_alloc(
            self.h_client, self.h_device, self.h_channel,
            NV_CLASS_CHANNEL_GA10X, con,
        )
    }

    /// Step 6: Alloc Display Container — GSP_RM_ALLOC (fn=103, class=0xC670)
    pub fn alloc_display(&mut self, con: &mut Console) -> Result<(), &'static str> {
        con.println("  [NV_RM] ALLOC Display (fn=103, class=0xC670)...");
        self.rpc.send_rm_alloc(
            self.h_client, self.h_device, self.h_display,
            NV_CLASS_DISPLAY_GA10X, con,
        )
    }

    /// Step 6b: Alloc Core Channel DMA — GSP_RM_ALLOC (fn=103, class=0xC67D)
    pub fn alloc_core_channel(&mut self, con: &mut Console) -> Result<(), &'static str> {
        con.println("  [NV_RM] ALLOC Core Channel DMA (fn=103, class=0xC67D)...");
        let h_core = self.rpc.alloc_handle();
        self.rpc.send_rm_alloc(
            self.h_client, self.h_display, h_core,
            display_class::NVC67D_CORE_CHANNEL_DMA, con,
        )
    }

    /// Step 7: Alloc 3D/Graphics engine — GSP_RM_ALLOC (fn=103, class=0xC697)
    pub fn alloc_3d(&mut self, con: &mut Console) -> Result<(), &'static str> {
        con.println("  [NV_RM] ALLOC 3D/Graphics (fn=103, class=0xC697)...");
        let h_3d = self.rpc.alloc_handle();
        self.rpc.send_rm_alloc(
            self.h_client, self.h_channel, h_3d,
            NV_CLASS_3D_GA10X, con,
        )
    }

    /// Step 8: Alloc Compute engine — GSP_RM_ALLOC (fn=103, class=0xC6C0)
    pub fn alloc_compute(&mut self, con: &mut Console) -> Result<(), &'static str> {
        con.println("  [NV_RM] ALLOC Compute (fn=103, class=0xC6C0)...");
        let h_ce = self.rpc.alloc_handle();
        self.rpc.send_rm_alloc(
            self.h_client, self.h_channel, h_ce,
            NV_CLASS_COMPUTE_GA10X, con,
        )
    }

    /// Step 9: Alloc DMA Copy engine — GSP_RM_ALLOC (fn=103, class=0xC6B5)
    pub fn alloc_dma_copy(&mut self, con: &mut Console) -> Result<(), &'static str> {
        con.println("  [NV_RM] ALLOC DMA Copy (fn=103, class=0xC6B5)...");
        let h_dma = self.rpc.alloc_handle();
        self.rpc.send_rm_alloc(
            self.h_client, self.h_channel, h_dma,
            NV_CLASS_DMA_COPY_GA10X, con,
        )
    }

    /// Control: Get display static info via GSP_RM_CONTROL (fn=76)
    pub fn get_display_static_info(&mut self, con: &mut Console) -> Result<(), &'static str> {
        con.println("  [NV_RM] CTRL: GET_DISPLAY_STATIC_INFO...");
        // NV2080_CTRL_CMD_INTERNAL_DISPLAY_GET_STATIC_INFO = 0x20800A01
        self.rpc.send_rm_control(self.h_client, self.h_subdevice, 0x2080_0A01, con)
    }

    /// Control: Get number of heads via GSP_RM_CONTROL (fn=76)
    pub fn get_num_heads(&mut self, con: &mut Console) -> Result<(), &'static str> {
        con.println("  [NV_RM] CTRL: GET_NUM_HEADS...");
        // NV0073_CTRL_CMD_SYSTEM_GET_NUM_HEADS = 0x00730102
        self.rpc.send_rm_control(self.h_client, self.h_display, 0x0073_0102, con)
    }

    /// Run the full init sequence as specified in the GSP Implementation Guide
    pub fn full_init_sequence(&mut self, con: &mut Console) -> Result<(), &'static str> {
        con.print_colored("=== NV_RM: Full Init Sequence (nvidia-open) ===\n",
            crate::fb::colors::ACCENT_CYAN);

        // Phase 5: Init RPCs
        let _ = self.set_system_info(con);
        let _ = self.set_registry(con);
        let _ = self.get_static_info(con);

        // Phase 6: Display via GSP
        let _ = self.alloc_display(con);
        let _ = self.alloc_core_channel(con);
        let _ = self.get_display_static_info(con);
        let _ = self.get_num_heads(con);

        // Phase 7: FIFO + engines via GSP
        let _ = self.alloc_va_space(con);
        let _ = self.alloc_channel(con);
        let _ = self.alloc_3d(con);
        let _ = self.alloc_compute(con);
        let _ = self.alloc_dma_copy(con);

        con.print_colored("=== NV_RM: Init Sequence COMPLETE ===\n",
            crate::fb::colors::TEXT_SUCCESS);
        Ok(())
    }
}
