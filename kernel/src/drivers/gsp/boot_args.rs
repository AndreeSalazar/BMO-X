//! GSP Boot Arguments — GSP_ARGUMENTS_CACHED
//!
//! Estructura que el booter espera encontrar en RAM para chain-loadear GSP-RM.
//! Basada en el header de NVIDIA r535 y nouveau.
//!
//! La dirección física de esta estructura se pasa al Falcon via MAILBOX
//! ANTES de arrancar el booter.

// ═══════════════════════════════════════════════════════════════
// MESSAGE_QUEUE_INIT_ARGUMENTS
// Defines el shared memory y las offsets de las colas cmd/status
// ═══════════════════════════════════════════════════════════════
#[repr(C)]
pub struct MessageQueueInitArgs {
    pub shared_mem_phys_addr: u64,    // Dirección física de la shared memory
    pub page_table_entry_count: u32,  // Número de PTEs (páginas)
    pub cmd_queue_offset: u32,        // Offset de la Command Queue dentro de shared mem
    pub stat_queue_offset: u32,       // Offset de la Status Queue
    pub lockless_cmd_queue_offset: u32, // Cola sin lock (opcional, 0)
    pub lockless_stat_queue_offset: u32, // Cola sin lock (opcional, 0)
}

// ═══════════════════════════════════════════════════════════════
// GSP_SR_INIT_ARGUMENTS (Suspend/Resume)
// ═══════════════════════════════════════════════════════════════
#[repr(C)]
pub struct GspSrInitArgs {
    pub old_level: u32,
    pub flags: u32,
    pub in_pm_transition: u32, // NvBool = u32
}

// ═══════════════════════════════════════════════════════════════
// GSP_ARGUMENTS_CACHED — La estructura principal
// Esta se pone en RAM y su dirección va en MAILBOX
// ═══════════════════════════════════════════════════════════════
#[repr(C, align(4096))] // Alineada a página (GSP_PAGE_SIZE)
pub struct GspArgumentsCached {
    pub mq_init: MessageQueueInitArgs,   // Colas de mensajes
    pub sr_init: GspSrInitArgs,          // Suspend/Resume args
    pub gpu_instance: u32,               // 0 para GPU principal
    pub profiler_pa: u64,                // 0 (no profiler)
    pub profiler_size: u64,              // 0
    // Padding hasta 4096 bytes (alineación de página)
    pub _pad: [u8; 4096 - 68],           // 68 = sum of fields above
}

impl GspArgumentsCached {
    /// Crea los boot args con las colas de mensajes configuradas
    pub fn new(shared_mem_phys: u64, shared_mem_pages: u32) -> Self {
        Self {
            mq_init: MessageQueueInitArgs {
                shared_mem_phys_addr: shared_mem_phys,
                page_table_entry_count: shared_mem_pages,
                cmd_queue_offset: 0x1000,       // CmdQ empieza en página 1
                stat_queue_offset: 0x2000,      // StatQ empieza en página 2
                lockless_cmd_queue_offset: 0,   // No usado
                lockless_stat_queue_offset: 0,  // No usado
            },
            sr_init: GspSrInitArgs {
                old_level: 0,
                flags: 0,
                in_pm_transition: 0,
            },
            gpu_instance: 0,
            profiler_pa: 0,
            profiler_size: 0,
            _pad: [0u8; 4096 - 68],
        }
    }
}
