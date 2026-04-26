//! NVIDIA Resource Manager (NV_RM) via GSP RPC
//!
//! Usa RPC_ALLOC_RESOURCE (0x04) con Class IDs en el payload.
//! Los Class IDs NO son function IDs — van dentro del mensaje.

use crate::console::Console;
use super::rpc::*;

pub struct NvResourceManager<'a, 'b> {
    rpc: &'a mut GspRpcRing<'b>,
}

impl<'a, 'b> NvResourceManager<'a, 'b> {
    pub fn new(rpc: &'a mut GspRpcRing<'b>) -> Self {
        Self { rpc }
    }

    /// Paso 1: Inicializar GSP-RM
    pub fn gsp_init(&mut self, con: &mut Console) -> Result<(), &'static str> {
        con.print_colored("=== NV_RM: GSP Init (func=0x01) ===\n", crate::fb::colors::ACCENT_CYAN);
        self.rpc.send_rpc(RPC_GSP_INIT, 0, con)
    }

    /// Paso 2: Enviar info del sistema
    pub fn set_system_info(&mut self, con: &mut Console) -> Result<(), &'static str> {
        con.println("  [NV_RM] Set System Info (func=0x02)...");
        self.rpc.send_rpc(RPC_SET_SYSTEM_INFO, 0, con)
    }

    /// Paso 3: Asignar VA Space (Class 0xC6FA)
    pub fn alloc_va_space(&mut self, con: &mut Console) -> Result<(), &'static str> {
        con.println("  [NV_RM] Alloc VA Space (func=0x04, class=0xC6FA)...");
        self.rpc.send_rpc(RPC_ALLOC_RESOURCE, NV_CLASS_VA_SPACE_GA10X, con)
    }

    /// Paso 4: Crear canal DMA (Class 0xC66F)
    pub fn alloc_channel(&mut self, con: &mut Console) -> Result<(), &'static str> {
        con.println("  [NV_RM] Alloc Channel DMA (func=0x04, class=0xC66F)...");
        self.rpc.send_rpc(RPC_ALLOC_RESOURCE, NV_CLASS_CHANNEL_GA10X, con)
    }

    /// Paso 5: Asignar Display Engine (Class 0xC670)
    pub fn alloc_display(&mut self, con: &mut Console) -> Result<(), &'static str> {
        con.println("  [NV_RM] Alloc Display (func=0x04, class=0xC670)...");
        self.rpc.send_rpc(RPC_ALLOC_RESOURCE, NV_CLASS_DISPLAY_GA10X, con)
    }

    /// Paso 6: Asignar motor 3D (Class 0xC697)
    pub fn alloc_3d(&mut self, con: &mut Console) -> Result<(), &'static str> {
        con.println("  [NV_RM] Alloc 3D/Graphics (func=0x04, class=0xC697)...");
        self.rpc.send_rpc(RPC_ALLOC_RESOURCE, NV_CLASS_3D_GA10X, con)
    }

    /// Paso 7: Asignar Compute (Class 0xC6C0)
    pub fn alloc_compute(&mut self, con: &mut Console) -> Result<(), &'static str> {
        con.println("  [NV_RM] Alloc Compute (func=0x04, class=0xC6C0)...");
        self.rpc.send_rpc(RPC_ALLOC_RESOURCE, NV_CLASS_COMPUTE_GA10X, con)
    }

    /// Paso 8: Asignar DMA Copy (Class 0xC6B5)
    pub fn alloc_dma_copy(&mut self, con: &mut Console) -> Result<(), &'static str> {
        con.println("  [NV_RM] Alloc DMA Copy (func=0x04, class=0xC6B5)...");
        self.rpc.send_rpc(RPC_ALLOC_RESOURCE, NV_CLASS_DMA_COPY_GA10X, con)
    }

    /// Paso 9: Post-Init
    pub fn gsp_init_post(&mut self, con: &mut Console) -> Result<(), &'static str> {
        con.println("  [NV_RM] GSP Init Post (func=0x10)...");
        self.rpc.send_rpc(RPC_GSP_INIT_POST, 0, con)
    }
}
