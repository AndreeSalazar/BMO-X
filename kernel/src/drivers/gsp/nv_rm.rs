//! NVIDIA Resource Manager (NV_RM) via GSP RPC
//!
//! Fase 2: Gestor de Recursos. Se encarga de enviar los mensajes RPC
//! a la IA del GSP para solicitar memoria de video (VRAM) y acceso a motores gráficos.

use crate::console::Console;
use super::rpc::{GspRpcRing, RPC_OP_ALLOC_MEMORY, RPC_OP_INIT_DISPLAY};

pub struct NvResourceManager<'a, 'b> {
    rpc: &'a mut GspRpcRing<'b>,
}

impl<'a, 'b> NvResourceManager<'a, 'b> {
    pub fn new(rpc: &'a mut GspRpcRing<'b>) -> Self {
        Self { rpc }
    }

    pub fn allocate_vram(&mut self, size_mb: u32, con: &mut Console) -> Result<u64, &'static str> {
        con.print_colored("=== Fase 2: NV_RM (Resource Manager) ===\n", crate::fb::colors::ACCENT_CYAN);
        con.print("  [NV_RM] Solicitando ");
        con.print_hex32(size_mb);
        con.println(" MB de VRAM al GSP...");
        
        self.rpc.send_rpc(RPC_OP_ALLOC_MEMORY, 8, con)?;
        
        // Simulamos la recepción de un puntero a la memoria de la tarjeta
        Ok(0x0000_0000) 
    }

    pub fn init_display_engine(&mut self, con: &mut Console) -> Result<(), &'static str> {
        con.println("  [NV_RM] Transfiriendo control del HDMI/DP a FastOS...");
        self.rpc.send_rpc(RPC_OP_INIT_DISPLAY, 0, con)?;
        Ok(())
    }
}
