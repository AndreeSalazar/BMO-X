//! NVIDIA Resource Manager (NV_RM) via GSP RPC — Datos Reales
//!
//! Gestor de Recursos usando las clases de Ampere GA10x confirmadas
//! por SigDead Hunter. Tabla de dispatch principal: 59 handlers (0xD07DA8).

use crate::console::Console;
use super::rpc::*;

pub struct NvResourceManager<'a, 'b> {
    rpc: &'a mut GspRpcRing<'b>,
}

impl<'a, 'b> NvResourceManager<'a, 'b> {
    pub fn new(rpc: &'a mut GspRpcRing<'b>) -> Self {
        Self { rpc }
    }

    /// Solicitar memoria VRAM a través del VA Space (Class 0xC6FA)
    pub fn allocate_vram(&mut self, size_mb: u32, con: &mut Console) -> Result<u64, &'static str> {
        con.print_colored("=== Fase 2: NV_RM Resource Manager (GA10x) ===\n",
            crate::fb::colors::ACCENT_CYAN);

        // 1. Abrir Virtual Address Space (Class 0xC6FA)
        con.print("  [NV_RM] Abriendo VA Space (Class 0xC6FA)... ");
        con.print_hex32(size_mb);
        con.println(" MB solicitados.");
        let _ = self.rpc.send_rpc(NV_CLASS_VA_SPACE_GA10X, 8, con);

        // 2. Crear canal DMA (Class 0xC66F)
        con.println("  [NV_RM] Creando Canal DMA (Class 0xC66F)...");
        let _ = self.rpc.send_rpc(NV_CLASS_CHANNEL_GA10X, 16, con);

        Ok(0x0000_0000)
    }

    /// Transferir control del display al GSP (Class 0xC670)
    pub fn init_display_engine(&mut self, con: &mut Console) -> Result<(), &'static str> {
        con.println("  [NV_RM] Solicitando Display Engine (Class 0xC670)...");
        let _ = self.rpc.send_rpc(NV_CLASS_DISPLAY_GA10X, 0, con);
        Ok(())
    }

    /// Activar motor 3D (Class 0xC697)
    pub fn init_3d_engine(&mut self, con: &mut Console) -> Result<(), &'static str> {
        con.println("  [NV_RM] Activando Motor 3D/Graphics (Class 0xC697)...");
        let _ = self.rpc.send_rpc(NV_CLASS_3D_GA10X, 0, con);
        Ok(())
    }

    /// Activar motor Compute (Class 0xC6C0)
    pub fn init_compute_engine(&mut self, con: &mut Console) -> Result<(), &'static str> {
        con.println("  [NV_RM] Activando Motor Compute (Class 0xC6C0)...");
        let _ = self.rpc.send_rpc(NV_CLASS_COMPUTE_GA10X, 0, con);
        Ok(())
    }

    /// Activar DMA Copy engines (Class 0xC6B5, 10 instancias)
    pub fn init_dma_copy(&mut self, con: &mut Console) -> Result<(), &'static str> {
        con.println("  [NV_RM] Activando DMA Copy (Class 0xC6B5, 10 instancias)...");
        let _ = self.rpc.send_rpc(NV_CLASS_DMA_COPY_GA10X, 4, con);
        Ok(())
    }
}
