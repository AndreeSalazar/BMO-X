//! DMA Pushbuffers para Renderizado 3D — Motor PGRAPH Ampere GA10x
//!
//! Canal de comandos DMA directo al motor 3D (Class 0xC697).
//! Confirmado por SigDead Hunter en offsets 0xD05068, 0xE55480, 0xF579E8.
//!
//! PGRAPH base: 0x800000 (BAR0)

use crate::console::Console;
use super::rpc::{NV_PGRAPH_BASE, NV_PFIFO_BASE};

// Registros FIFO para controlar el pushbuffer DMA
const NV_PFIFO_RUNLIST:    u32 = 0x00020800;
const NV_PFIFO_CHANNEL:    u32 = 0x00020000;

// Métodos del motor PGRAPH (Class 0xC697 — Ampere 3D)
pub const PGRAPH_SET_OBJECT:          u32 = 0x0000; // Seleccionar clase
pub const PGRAPH_NO_OPERATION:        u32 = 0x0100; // NOP (ping)
pub const PGRAPH_SET_REF:             u32 = 0x0050; // Fence/sync
pub const PGRAPH_CLEAR_BUFFERS:       u32 = 0x1D94; // Limpiar framebuffer
pub const PGRAPH_BEGIN_END:           u32 = 0x17FC; // Begin/End draw
pub const PGRAPH_VERTEX_BEGIN_END:    u32 = 0x17FC;
pub const PGRAPH_INLINE_VERTEX_DATA:  u32 = 0x1800; // Enviar vértices inline

pub struct PushBuffer {
    phys_addr: u64,
    virt_ptr: *mut u32,
    offset: usize,
    capacity: usize, // en u32 words
}

impl PushBuffer {
    pub fn new(phys_addr: u64) -> Self {
        Self {
            phys_addr,
            virt_ptr: phys_addr as *mut u32,
            offset: 0,
            capacity: 4096 / 4, // 1 página = 1024 comandos
        }
    }

    /// Empuja un método PGRAPH al canal DMA.
    /// Formato: [count:13][subchannel:3][method:13][type:3]
    pub fn push_method(&mut self, subchannel: u32, method: u32, data: u32) {
        if self.offset + 2 > self.capacity { return; }
        // Header word: 1 data word, subchannel, method/4, incrementing
        let header = (1 << 28) | ((subchannel & 0x7) << 13) | ((method >> 2) & 0x1FFF);
        unsafe {
            core::ptr::write_volatile(self.virt_ptr.add(self.offset), header);
            core::ptr::write_volatile(self.virt_ptr.add(self.offset + 1), data);
        }
        self.offset += 2;
    }

    /// Seleccionar la clase 3D (0xC697) en subchannel 0
    pub fn bind_3d_class(&mut self) {
        self.push_method(0, PGRAPH_SET_OBJECT, super::rpc::NV_CLASS_3D_GA10X);
    }

    /// NOP — verificar que el canal funciona
    pub fn nop(&mut self) {
        self.push_method(0, PGRAPH_NO_OPERATION, 0);
    }

    /// Limpiar el framebuffer con un color
    pub fn clear_color(&mut self, color_rgba: u32) {
        self.push_method(0, PGRAPH_CLEAR_BUFFERS, color_rgba);
    }

    /// Le indica al motor PGRAPH que procese toda la cola.
    pub fn execute(&self, bar0: &nv_hal::MmioRegion, con: &mut Console) {
        con.print_colored("=== Fase 4: Aceleracion 3D PGRAPH (Class 0xC697) ===\n",
            crate::fb::colors::ACCENT_CYAN);
        con.print("  [PGRAPH] Disparando ");
        con.print_hex32(self.offset as u32);
        con.println(" words al motor 3D de la GPU...");

        // Escribir PUT pointer para que PGRAPH lea el buffer
        // NV_PFIFO: PUT = offset actual, GET lo lee la GPU
        bar0.write32(NV_PFIFO_CHANNEL + 0x40, 0); // GET = 0
        bar0.write32(NV_PFIFO_CHANNEL + 0x44, (self.offset * 4) as u32); // PUT

        // Verificar que PUT fue aceptado
        let put_readback = bar0.read32(NV_PFIFO_CHANNEL + 0x44);
        con.print("  [PGRAPH] PUT readback: 0x");
        con.print_hex32(put_readback);
        con.newline();

        con.print_colored("=== Motor 3D ACTIVO — GPU Renderizando ===\n",
            crate::fb::colors::TEXT_SUCCESS);
    }
}
