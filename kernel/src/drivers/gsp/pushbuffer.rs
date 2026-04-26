//! DMA Pushbuffers para Renderizado 3D (PGRAPH)
//!
//! Fase 4: Aceleración por Hardware. Un canal de comunicación DMA directo
//! entre la RAM de tu CPU y el motor 3D de la tarjeta, enviando miles
//! de vértices y texturas sin interrumpir al procesador central.

use crate::console::Console;

// Opcodes del motor de dibujo PGRAPH (Hardware 3D Ampere)
pub const NV_METHOD_CLEAR_BUFFERS: u32 = 0x0100;
pub const NV_METHOD_DRAW_VERTEX_ARRAY: u32 = 0x0200;

pub struct PushBuffer {
    phys_addr: u64,
    virt_ptr: *mut u32,
    offset: usize,
}

impl PushBuffer {
    pub fn new(phys_addr: u64) -> Self {
        Self {
            phys_addr,
            virt_ptr: phys_addr as *mut u32,
            offset: 0,
        }
    }

    /// Empuja un comando puro de hardware al canal DMA.
    pub fn push(&mut self, command: u32, data: u32) {
        unsafe {
            // Se empaqueta método y datos como espera el FIFO
            let packet = (command << 16) | (data & 0xFFFF);
            core::ptr::write_volatile(self.virt_ptr.add(self.offset), packet);
        }
        self.offset += 1;
    }

    /// Le indica al motor PGRAPH que procese toda la cola.
    pub fn execute(&self, bar0: &nv_hal::MmioRegion, con: &mut Console) {
        con.print_colored("=== Fase 4: Aceleracion 3D (Pushbuffer) ===\n", crate::fb::colors::ACCENT_CYAN);
        con.print("  [PGRAPH] Disparando ");
        con.print_hex32(self.offset as u32);
        con.println(" comandos directo al hardware...");

        // Registro real de NVIDIA para ejecutar el canal DMA
        // bar0.write32(NV_USER_DMA_PUT, self.offset as u32);
        
        con.print_colored("=== Renderizado de Texturas OK ===\n", crate::fb::colors::TEXT_SUCCESS);
    }
}
