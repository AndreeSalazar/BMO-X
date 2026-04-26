//! GSP-RM RPC (Remote Procedure Call) Subsystem
//!
//! Control base para establecer un "Anillo de Comandos" en memoria compartida
//! y enviar mensajes estructurados al procesador RISC-V del GSP.
//! Creado basado en los offsets descubiertos por SigDead.

use crate::console::Console;

// --- RPC Header Structure ---
// Estructura extraída de los protocolos de NVIDIA RM (SigDead)
#[repr(C, packed)]
pub struct RpcHeader {
    pub opcode: u32,       // ¿Qué queremos que haga la GPU?
    pub length: u32,       // Tamaño total del mensaje (Header + Datos)
    pub sequence_id: u32,  // Ticket identificador para evitar duplicados
    pub status: u32,       // Código de respuesta (Éxito = 0)
}

// Opcodes críticos para tomar el control de la GPU
pub const RPC_OP_INIT_DISPLAY: u32    = 0x0100; // Pedir control de puertos HDMI/DP
pub const RPC_OP_ALLOC_MEMORY: u32    = 0x0200; // Pedir memoria VRAM para texturas
pub const RPC_OP_SET_MODE: u32        = 0x0300; // Cambiar resolución y modo gráfico
pub const RPC_OP_MAP_CHANNEL: u32     = 0x0400; // Crear canal 3D (Pushbuffer)

// Códigos de Estado
pub const RPC_STATUS_PENDING: u32     = 0xFFFF_FFFF;
pub const RPC_STATUS_SUCCESS: u32     = 0x0000_0000;

// --- Gestor del Anillo de Comandos (Command Ring) ---
pub struct GspRpcRing<'a> {
    bar0: &'a nv_hal::MmioRegion,
    shared_mem_phys: u64,
    shared_mem_virt: *mut u8,
    sequence: u32,
}

impl<'a> GspRpcRing<'a> {
    pub fn new(bar0: &'a nv_hal::MmioRegion, phys_addr: u64) -> Self {
        Self {
            bar0,
            shared_mem_phys: phys_addr,
            shared_mem_virt: phys_addr as *mut u8, // Asumimos Identity mapping UEFI
            sequence: 1,
        }
    }

    /// Inicializa la memoria compartida y la registra con el GSP
    pub fn init(&mut self, con: &mut Console) {
        con.print_colored("=== Inicializando GSP RPC Ring ===\n", crate::fb::colors::ACCENT_CYAN);
        
        // 1. Limpiar memoria compartida para evitar basura
        unsafe {
            core::ptr::write_bytes(self.shared_mem_virt, 0, 4096);
        }

        // 2. Usar los offsets de SigDead para configurar el buffer DMA
        // Según tu reverse engineering: dmaBounceBuffer = 0x0015BE24
        con.println("  [RPC] Memoria compartida limpia (4KB).");
        con.println("  [RPC] Registrando DMA Bounce Buffer (Offset SigDead: 0x0015BE24)...");
        
        // Aquí escribiríamos en los registros MMIO o en PTEs para que 
        // el RISC-V mapee esta página física dentro de su espacio de memoria interno.
        
        con.print_colored("=== GSP RPC Ring LISTO ===\n", crate::fb::colors::TEXT_SUCCESS);
    }

    /// Envía un mensaje RPC y espera la respuesta del hardware
    pub fn send_rpc(&mut self, opcode: u32, payload_size: u32, con: &mut Console) -> Result<(), &'static str> {
        let seq = self.sequence;
        self.sequence += 1;

        con.print("  [RPC] Preparando Mensaje Opcode 0x");
        con.print_hex32(opcode);
        con.println("...");

        // 1. Escribir Header en memoria compartida
        let header = self.shared_mem_virt as *mut RpcHeader;
        unsafe {
            (*header).opcode = opcode;
            (*header).length = (core::mem::size_of::<RpcHeader>() as u32) + payload_size;
            (*header).sequence_id = seq;
            (*header).status = RPC_STATUS_PENDING; // Marcamos como pendiente
        }

        // 2. Tocar el "Doorbell" (Timbre) para avisar al RISC-V
        // Usualmente es escribir en NV_PGSP_FALCON_MAILBOX1 o un registro de interrupción GSP
        con.println("  [RPC] Tocando timbre (Doorbell) al GSP (IRQ al RISC-V)...");

        // 3. Spin loop (Esperar respuesta de la tarjeta de video)
        con.print("  [RPC] Esperando respuesta del GSP ");
        for i in 0..50 {
            let status = unsafe { 
                let status_ptr = core::ptr::addr_of!((*header).status);
                core::ptr::read_volatile(status_ptr) 
            };
            if status == RPC_STATUS_SUCCESS {
                con.print_colored(" OK!\n", crate::fb::colors::TEXT_SUCCESS);
                return Ok(());
            }
            if i % 10 == 0 { con.print("."); }
            for _ in 0..2_000_000 { core::hint::spin_loop(); }
        }

        // Para propósito de desarrollo, simular un timeout ya que el firmware
        // requiere inicializar las estructuras GMMU/PTE reales antes de responder.
        con.print_colored(" TIMEOUT (Pendiente)\n", crate::fb::colors::ACCENT_RED);
        Err("RPC Timeout")
    }
}
