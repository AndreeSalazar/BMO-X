//! GSP-RM RPC — Protocolo completo de comunicación con el GSP
//!
//! Estructura de un mensaje GSP (de nouveau r535.c):
//!   [r535_gsp_msg header (28 bytes)] + [nvfw_gsp_rpc header (32 bytes)] + [payload]
//!
//! La comunicación usa DOS colas en memoria compartida:
//!   - Command Queue (cmdq): CPU → GSP  (nosotros escribimos)
//!   - Status Queue  (msgq): GSP → CPU  (GSP escribe respuestas)
//!
//! Cada cola es un ring buffer con HEAD/TAIL en registros MMIO.

use crate::console::Console;

// ═══════════════════════════════════════════════════════════════
// r535_gsp_msg — Message Queue Element Header (de nouveau)
// ESTE header envuelve al nvfw_gsp_rpc
// ═══════════════════════════════════════════════════════════════
#[repr(C)]
pub struct GspMsgHeader {
    pub auth_tag: [u8; 16],   // +0:  Autenticación (ceros para nuestro caso)
    pub checksum: u32,        // +16: Checksum (se calcula para que sume 0)
    pub sequence: u32,        // +20: Número de secuencia de la cola
    pub elem_count: u32,      // +24: Número de elementos (1 para msgs pequeños)
}

// ═══════════════════════════════════════════════════════════════
// nvfw_gsp_rpc — RPC Header (32 bytes)
// ═══════════════════════════════════════════════════════════════
#[repr(C)]
pub struct NvfwGspRpc {
    pub header_version: u32,       // +0
    pub signature: u32,            // +4:  0x564B4E56 = "VNKV"
    pub length: u32,               // +8:  Tamaño total del RPC (header + payload)
    pub function: u32,             // +12: NV_VGPU_MSG_FUNCTION_*
    pub rpc_result: u32,           // +16: Resultado
    pub rpc_result_private: u32,   // +20
    pub sequence: u32,             // +24
    pub spare: u32,                // +28
}

pub const RPC_SIGNATURE: u32 = 0x564B4E56;
pub const GSP_MSG_HDR_SIZE: usize = 28; // sizeof(GspMsgHeader)
pub const GSP_RPC_HDR_SIZE: usize = 32; // sizeof(NvfwGspRpc)

// ═══════════════════════════════════════════════════════════════
// NV_VGPU_MSG_FUNCTION IDs — van en el campo 'function'
// ═══════════════════════════════════════════════════════════════
pub const RPC_GSP_INIT:          u32 = 0x01;
pub const RPC_SET_SYSTEM_INFO:   u32 = 0x02;
pub const RPC_ALLOC_RESOURCE:    u32 = 0x04;
pub const RPC_FREE_RESOURCE:     u32 = 0x05;
pub const RPC_CONTROL:           u32 = 0x0A;
pub const RPC_DMA_SETUP:         u32 = 0x0D;
pub const RPC_GSP_INIT_POST:     u32 = 0x10;

// ═══════════════════════════════════════════════════════════════
// Clases Ampere GA10x — van en el PAYLOAD del ALLOC_RESOURCE
// ═══════════════════════════════════════════════════════════════
pub const NV_CLASS_DISPLAY_GA10X: u32  = 0xC670;
pub const NV_CLASS_3D_GA10X: u32       = 0xC697;
pub const NV_CLASS_COMPUTE_GA10X: u32  = 0xC6C0;
pub const NV_CLASS_DMA_COPY_GA10X: u32 = 0xC6B5;
pub const NV_CLASS_CHANNEL_GA10X: u32  = 0xC66F;
pub const NV_CLASS_VA_SPACE_GA10X: u32 = 0xC6FA;

// ═══════════════════════════════════════════════════════════════
// Registros MMIO — Falcon / GSP (BAR0)
// El loader.rs usa 0x110040/0x110044 para MAILBOX (booter)
// Las colas de mensajes usan registros diferentes para HEAD/TAIL
// ═══════════════════════════════════════════════════════════════
// Falcon core registers (del loader.rs, ya confirmados funcionando)
pub const NV_PGSP_FALCON_MAILBOX0: u32 = 0x00110040; // Booter usa estos
pub const NV_PGSP_FALCON_MAILBOX1: u32 = 0x00110044;

// Message Queue registers (ring buffer HEAD/TAIL)
// En Ampere, las colas usan registros EMEM/scratch del Falcon
pub const NV_PGSP_MSGQ_HEAD:  u32 = 0x00110C80; // Command queue HEAD
pub const NV_PGSP_MSGQ_TAIL:  u32 = 0x00110C84; // Command queue TAIL
pub const NV_PGSP_STATQ_HEAD: u32 = 0x00110C88; // Status queue HEAD
pub const NV_PGSP_STATQ_TAIL: u32 = 0x00110C8C; // Status queue TAIL

// Display / Graph
pub const NV_PDISP_BASE: u32      = 0x00610000;
pub const NV_PDISP_HEAD_BASE: u32 = 0x00640000;
pub const NV_PGRAPH_BASE: u32     = 0x00800000;
pub const NV_PFIFO_BASE: u32      = 0x00020000;

// ═══════════════════════════════════════════════════════════════
// Command Queue — Ring Buffer en memoria compartida
// ═══════════════════════════════════════════════════════════════
const CMDQ_SIZE: usize = 4096; // 1 página para la cola de comandos
const MSGQ_SIZE: usize = 4096; // 1 página para la cola de respuestas

pub struct GspRpcRing<'a> {
    bar0: &'a nv_hal::MmioRegion,
    // Command queue (CPU → GSP)
    cmdq_phys: u64,
    cmdq_virt: *mut u8,
    // Status queue (GSP → CPU)
    msgq_phys: u64,
    msgq_virt: *mut u8,
    sequence: u32,
}

impl<'a> GspRpcRing<'a> {
    pub fn new(bar0: &'a nv_hal::MmioRegion, cmdq_phys: u64) -> Self {
        // Status queue justo después del command queue
        let msgq_phys = cmdq_phys + CMDQ_SIZE as u64;
        Self {
            bar0,
            cmdq_phys,
            cmdq_virt: cmdq_phys as *mut u8,
            msgq_phys,
            msgq_virt: msgq_phys as *mut u8,
            sequence: 1,
        }
    }

    /// Inicializa las colas y registra con el GSP
    pub fn init(&mut self, con: &mut Console) {
        con.print_colored("=== GSP RPC Ring Init ===\n", crate::fb::colors::ACCENT_CYAN);

        // Limpiar ambas colas
        unsafe {
            core::ptr::write_bytes(self.cmdq_virt, 0, CMDQ_SIZE);
            core::ptr::write_bytes(self.msgq_virt, 0, MSGQ_SIZE);
        }

        // Registrar las colas en los registros del Falcon
        // Escribir dirección física de cmdq en MSGQ_HEAD (el GSP leerá de aquí)
        self.bar0.write32(NV_PGSP_MSGQ_HEAD, self.cmdq_phys as u32);
        self.bar0.write32(NV_PGSP_MSGQ_TAIL, self.cmdq_phys as u32);

        // Leer de vuelta para verificar
        let head = self.bar0.read32(NV_PGSP_MSGQ_HEAD);
        let tail = self.bar0.read32(NV_PGSP_MSGQ_TAIL);
        con.print("  [RPC] CMDQ HEAD=0x");
        con.print_hex32(head);
        con.print(" TAIL=0x");
        con.print_hex32(tail);
        con.newline();

        // También probar con los registros originales del Falcon MAILBOX
        // que YA SABEMOS que funcionan (el booter los usó exitosamente)
        self.bar0.write32(NV_PGSP_FALCON_MAILBOX0, self.cmdq_phys as u32);

        let mb0 = self.bar0.read32(NV_PGSP_FALCON_MAILBOX0);
        con.print("  [RPC] FALCON_MBOX0=0x");
        con.print_hex32(mb0);
        con.newline();

        con.print_colored("=== GSP RPC Ring ACTIVO ===\n", crate::fb::colors::TEXT_SUCCESS);
    }

    /// Calcula checksum para que la suma de todo el header sea 0
    fn calc_checksum(data: *const u8, len: usize) -> u32 {
        let mut sum: u32 = 0;
        let words = len / 4;
        let ptr = data as *const u32;
        for i in 0..words {
            let val = unsafe { core::ptr::read_volatile(ptr.add(i)) };
            sum = sum.wrapping_add(val);
        }
        0u32.wrapping_sub(sum)
    }

    /// Envía un mensaje RPC completo: [GspMsgHeader][NvfwGspRpc][payload]
    pub fn send_rpc(&mut self, function: u32, class_id: u32,
                    con: &mut Console) -> Result<(), &'static str> {
        let seq = self.sequence;
        self.sequence += 1;

        let payload_size: u32 = if class_id != 0 { 4 } else { 0 };
        let rpc_length = GSP_RPC_HDR_SIZE as u32 + payload_size;
        let total_msg = GSP_MSG_HDR_SIZE + rpc_length as usize;

        con.print("  [RPC] func=0x");
        con.print_hex32(function);
        if class_id != 0 {
            con.print(" class=0x");
            con.print_hex32(class_id);
        }
        con.print(" seq=");
        con.print_hex32(seq);
        con.newline();

        // Escribir GspMsgHeader al inicio de cmdq
        let msg_hdr = self.cmdq_virt as *mut GspMsgHeader;
        unsafe {
            core::ptr::write_bytes(msg_hdr as *mut u8, 0, GSP_MSG_HDR_SIZE);
            (*msg_hdr).sequence = seq;
            (*msg_hdr).elem_count = 1;
        }

        // Escribir NvfwGspRpc justo después
        let rpc_hdr = unsafe { self.cmdq_virt.add(GSP_MSG_HDR_SIZE) } as *mut NvfwGspRpc;
        unsafe {
            (*rpc_hdr).header_version = 0x03;
            (*rpc_hdr).signature = RPC_SIGNATURE;
            (*rpc_hdr).length = rpc_length;
            (*rpc_hdr).function = function;
            (*rpc_hdr).rpc_result = 0xFFFF_FFFF;
            (*rpc_hdr).rpc_result_private = 0;
            (*rpc_hdr).sequence = seq;
            (*rpc_hdr).spare = 0;

            // Payload: class_id si aplica
            if class_id != 0 {
                let payload = self.cmdq_virt.add(GSP_MSG_HDR_SIZE + GSP_RPC_HDR_SIZE) as *mut u32;
                core::ptr::write_volatile(payload, class_id);
            }

            // Calcular checksum del GspMsgHeader
            (*msg_hdr).checksum = 0;
            (*msg_hdr).checksum = Self::calc_checksum(
                msg_hdr as *const u8, total_msg
            );
        }

        // Notificar al GSP: actualizar TAIL para indicar nuevo mensaje
        let new_tail = total_msg as u32;
        self.bar0.write32(NV_PGSP_MSGQ_TAIL, new_tail);

        // También notificar vía FALCON MAILBOX (que sabemos funciona)
        self.bar0.write32(NV_PGSP_FALCON_MAILBOX0, new_tail);

        // Esperar respuesta
        con.print("  [RPC] Esperando GSP ");
        for i in 0..100 {
            let result = unsafe {
                core::ptr::read_volatile(&(*rpc_hdr).rpc_result)
            };
            if result == 0 {
                con.print_colored(" OK!\n", crate::fb::colors::TEXT_SUCCESS);
                return Ok(());
            }
            // También chequear si MAILBOX1 cambió (el GSP podría responder ahí)
            if i == 50 {
                let mb1 = self.bar0.read32(NV_PGSP_FALCON_MAILBOX1);
                if mb1 != 0 {
                    con.print(" MB1=0x");
                    con.print_hex32(mb1);
                }
            }
            if i % 20 == 0 { con.print("."); }
            for _ in 0..1_000_000 { core::hint::spin_loop(); }
        }

        con.print_colored(" TIMEOUT\n", crate::fb::colors::ACCENT_RED);
        Err("GSP RPC Timeout")
    }
}
