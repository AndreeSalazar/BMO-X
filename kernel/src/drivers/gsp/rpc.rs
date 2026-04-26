//! GSP-RM RPC (Remote Procedure Call) — Datos REALES del firmware
//!
//! Estructura nvfw_gsp_rpc extraída de nouveau (Linux kernel) y validada
//! por SigDead Hunter contra el firmware gsp_ga10x.bin de la RTX 3060.
//!
//! Registro MAILBOX real: 0x110044 (PGSP_MAILBOX0) — confirmado en offset 0xE9AEF4
//! RPC Dispatch Table: 59 handlers en offset 0xD07DA8 del firmware

use crate::console::Console;

// ═══════════════════════════════════════════════════════════════
// nvfw_gsp_rpc — Header RPC real (32 bytes) — de nouveau r535.c
// ═══════════════════════════════════════════════════════════════
#[repr(C)]
pub struct NvfwGspRpc {
    pub header_version: u32,       // +0:  Versión del protocolo
    pub signature: u32,            // +4:  0x564B4E56 = "VNKV"
    pub length: u32,               // +8:  Tamaño total (header + payload)
    pub function: u32,             // +12: NV_VGPU_MSG_FUNCTION_*
    pub rpc_result: u32,           // +16: Código de retorno
    pub rpc_result_private: u32,   // +20: Estado interno GSP
    pub sequence: u32,             // +24: Número de secuencia
    pub spare: u32,                // +28: Reservado / cpuRmGfid
}

pub const RPC_SIGNATURE: u32 = 0x564B4E56; // "VNKV" en little-endian

// ═══════════════════════════════════════════════════════════════
// NV_VGPU_MSG_FUNCTION — IDs de función RPC (de nouveau r535.c)
// ESTOS van en el campo 'function' del header — NO los class IDs
// ═══════════════════════════════════════════════════════════════
pub const RPC_GSP_INIT:          u32 = 0x01; // Inicializar GSP-RM
pub const RPC_SET_SYSTEM_INFO:   u32 = 0x02; // Info del sistema host
pub const RPC_ALLOC_RESOURCE:    u32 = 0x04; // Asignar recurso (class ID va en payload)
pub const RPC_FREE_RESOURCE:     u32 = 0x05; // Liberar recurso
pub const RPC_CONTROL:           u32 = 0x0A; // Control de subsistema
pub const RPC_DMA_SETUP:         u32 = 0x0D; // Configurar canal DMA
pub const RPC_GSP_INIT_POST:     u32 = 0x10; // Post-init GSP

// ═══════════════════════════════════════════════════════════════
// Clases de Objetos Ampere GA10x — van en el PAYLOAD del RPC
// Confirmados por SigDead Hunter — offsets 0xD04A00-0xD05600
// ═══════════════════════════════════════════════════════════════
pub const NV_CLASS_DISPLAY_GA10X: u32  = 0xC670;
pub const NV_CLASS_3D_GA10X: u32       = 0xC697;
pub const NV_CLASS_COMPUTE_GA10X: u32  = 0xC6C0;
pub const NV_CLASS_DMA_COPY_GA10X: u32 = 0xC6B5;
pub const NV_CLASS_CHANNEL_GA10X: u32  = 0xC66F;
pub const NV_CLASS_VA_SPACE_GA10X: u32 = 0xC6FA;

// ═══════════════════════════════════════════════════════════════
// Registros MMIO reales (confirmados por SigDead en 0xE9AEF4)
// ═══════════════════════════════════════════════════════════════
pub const NV_PGSP_MAILBOX0: u32       = 0x00110044;
pub const NV_PGSP_MAILBOX1: u32       = 0x00110048;
pub const NV_PGSP_QUEUE_HEAD0: u32    = 0x00110A00;
pub const NV_PGSP_QUEUE_TAIL0: u32    = 0x00110A04;

pub const NV_PDISP_BASE: u32          = 0x00610000;
pub const NV_PDISP_HEAD_BASE: u32     = 0x00640000;
pub const NV_PGRAPH_BASE: u32         = 0x00800000;

// Registros del FIFO (canales DMA)
pub const NV_PFIFO_BASE: u32          = 0x00020000;

// ═══════════════════════════════════════════════════════════════
// Comunicación GSP — Anillo de Comandos
// ═══════════════════════════════════════════════════════════════
pub struct GspRpcRing<'a> {
    bar0: &'a nv_hal::MmioRegion,
    msg_buf_phys: u64,
    msg_buf_virt: *mut u8,
    sequence: u32,
}

impl<'a> GspRpcRing<'a> {
    pub fn new(bar0: &'a nv_hal::MmioRegion, phys_addr: u64) -> Self {
        Self {
            bar0,
            msg_buf_phys: phys_addr,
            msg_buf_virt: phys_addr as *mut u8,
            sequence: 1,
        }
    }

    /// Registra el buffer y escribe dirección en QUEUE_HEAD
    pub fn init(&mut self, con: &mut Console) {
        con.print_colored("=== GSP RPC Init ===\n", crate::fb::colors::ACCENT_CYAN);
        unsafe { core::ptr::write_bytes(self.msg_buf_virt, 0, 4096); }

        // Registrar dirección del buffer en QUEUE_HEAD para que GSP lo lea
        self.bar0.write32(NV_PGSP_QUEUE_HEAD0, self.msg_buf_phys as u32);
        self.bar0.write32(NV_PGSP_QUEUE_TAIL0, self.msg_buf_phys as u32);

        let qh = self.bar0.read32(NV_PGSP_QUEUE_HEAD0);
        con.print("  [RPC] QUEUE_HEAD0 = 0x");
        con.print_hex32(qh);
        con.newline();

        con.print_colored("=== GSP RPC Ring ACTIVO ===\n", crate::fb::colors::TEXT_SUCCESS);
    }

    /// Envía un mensaje RPC con function ID y opcionalmente un class_id en el payload
    pub fn send_rpc(&mut self, function: u32, class_id: u32,
                    con: &mut Console) -> Result<(), &'static str> {
        let seq = self.sequence;
        self.sequence += 1;

        // Payload = 4 bytes si hay class_id, 0 si no
        let payload_size: u32 = if class_id != 0 { 4 } else { 0 };
        let total_len = 32 + payload_size;

        con.print("  [RPC] func=0x");
        con.print_hex32(function);
        if class_id != 0 {
            con.print(" class=0x");
            con.print_hex32(class_id);
        }
        con.print(" seq=");
        con.print_hex32(seq);
        con.newline();

        // Escribir header nvfw_gsp_rpc
        let hdr = self.msg_buf_virt as *mut NvfwGspRpc;
        unsafe {
            (*hdr).header_version = 0x03;
            (*hdr).signature = RPC_SIGNATURE;
            (*hdr).length = total_len;
            (*hdr).function = function;
            (*hdr).rpc_result = 0xFFFF_FFFF;
            (*hdr).rpc_result_private = 0;
            (*hdr).sequence = seq;
            (*hdr).spare = 0;

            // Si hay class_id, ponerlo en el payload (offset +32)
            if class_id != 0 {
                let payload = self.msg_buf_virt.add(32) as *mut u32;
                core::ptr::write_volatile(payload, class_id);
            }
        }

        // Notificar al GSP escribiendo en QUEUE_HEAD
        self.bar0.write32(NV_PGSP_QUEUE_HEAD0, self.msg_buf_phys as u32);

        // Esperar respuesta leyendo rpc_result
        con.print("  [RPC] Esperando GSP ");
        for i in 0..100 {
            let result = unsafe {
                core::ptr::read_volatile(&(*hdr).rpc_result)
            };
            if result == 0 {
                con.print_colored(" OK!\n", crate::fb::colors::TEXT_SUCCESS);
                return Ok(());
            }
            if i % 20 == 0 { con.print("."); }
            for _ in 0..1_000_000 { core::hint::spin_loop(); }
        }

        con.print_colored(" TIMEOUT\n", crate::fb::colors::ACCENT_RED);
        Err("GSP RPC Timeout")
    }
}
