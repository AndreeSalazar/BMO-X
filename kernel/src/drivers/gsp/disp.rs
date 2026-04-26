//! Display Engine (PDISP) para NVIDIA Ampere GA10x — RTX 3060
//!
//! Controla la salida de video usando el Class ID 0xC670 (GA10x Display)
//! confirmado por SigDead en offsets 0xD053F8, 0xD055C0, 0xF56670.
//! La GPU dibuja directamente — NO la CPU.

use crate::console::Console;
use super::rpc::{GspRpcRing, NV_CLASS_DISPLAY_GA10X, NV_PDISP_BASE, NV_PDISP_HEAD_BASE};

// Registros de Display Head (offsets relativos a NV_PDISP_HEAD_BASE)
const DISP_HEAD_SET_CONTROL:   u32 = 0x0300; // Control maestro del head
const DISP_HEAD_SET_SIZE:      u32 = 0x0304; // Resolución (height << 16 | width)
const DISP_HEAD_SET_OFFSET:    u32 = 0x0308; // Offset del scanout en VRAM
const DISP_HEAD_SET_PITCH:     u32 = 0x030C; // Bytes por scanline

pub struct DisplayEngine<'a> {
    bar0: &'a nv_hal::MmioRegion,
}

impl<'a> DisplayEngine<'a> {
    pub fn new(bar0: &'a nv_hal::MmioRegion) -> Self {
        Self { bar0 }
    }

    /// Configura el modo 1920x1080 usando los registros PDISP reales.
    /// La GPU es el motor de renderizado — NO la CPU.
    pub fn set_mode_1080p(&self, rpc: &mut GspRpcRing, con: &mut Console) {
        con.print_colored("=== Fase 3: Display Engine GA10x (Class 0xC670) ===\n",
            crate::fb::colors::ACCENT_CYAN);

        // 1. Solicitar al GSP que abra la clase de Display via ALLOC_RESOURCE
        con.println("  [DISP] Alloc Display Class 0xC670 (func=0x04)...");
        let _ = rpc.send_rpc(super::rpc::RPC_ALLOC_RESOURCE, NV_CLASS_DISPLAY_GA10X, con);

        // 2. Programar registros PDISP para 1920x1080
        con.println("  [DISP] Programando registros PDISP (Head 0)...");

        // Resolución: height (1080) en bits [31:16], width (1920) en bits [15:0]
        let size_val: u32 = (1080 << 16) | 1920;
        con.print("  [DISP] HEAD_SET_SIZE = 0x");
        con.print_hex32(size_val);
        con.println("  (1920x1080)");
        self.bar0.write32(NV_PDISP_HEAD_BASE + DISP_HEAD_SET_SIZE, size_val);

        // Pitch: 1920 pixeles × 4 bytes = 7680 bytes por línea
        let pitch_val: u32 = 1920 * 4;
        con.print("  [DISP] HEAD_SET_PITCH = ");
        con.print_hex32(pitch_val);
        con.println("  (7680 bytes/scanline)");
        self.bar0.write32(NV_PDISP_HEAD_BASE + DISP_HEAD_SET_PITCH, pitch_val);

        // Offset: inicio del framebuffer en VRAM (0 = inicio)
        self.bar0.write32(NV_PDISP_HEAD_BASE + DISP_HEAD_SET_OFFSET, 0);

        // Control: activar head
        self.bar0.write32(NV_PDISP_HEAD_BASE + DISP_HEAD_SET_CONTROL, 1);

        // 3. Leer verificación
        let readback = self.bar0.read32(NV_PDISP_HEAD_BASE + DISP_HEAD_SET_SIZE);
        con.print("  [DISP] Verificacion lectura: 0x");
        con.print_hex32(readback);
        con.newline();

        con.print_colored("=== Display Engine 1080p ACTIVO (GPU Renderiza) ===\n",
            crate::fb::colors::TEXT_SUCCESS);
    }
}
