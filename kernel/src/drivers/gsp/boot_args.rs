//! GSP Boot Arguments — LibOS Init Arguments
//!
//! The GSP RISC-V firmware (libos-v3.1.0) expects boot arguments passed
//! via FALCON MAILBOX0/MAILBOX1 as a physical address pointing to this structure.
//!
//! Fuente: nvidia-open kernel_gsp.c, nouveau tu102.c
//!
//! LibOS expects 4 memory regions (LOGINIT, LOGINTR, LOGRM, RMARGS) plus
//! the message queue init arguments and suspend/resume state.
//!
//! The address of GspArgumentsCached goes in MAILBOX0 (lo32) / MAILBOX1 (hi32)
//! BEFORE the booter is launched.

// ═══════════════════════════════════════════════════════════════
// MESSAGE_QUEUE_INIT_ARGUMENTS
// Fuente: nvidia-open message_queue_priv.h
// ═══════════════════════════════════════════════════════════════
#[repr(C)]
pub struct MessageQueueInitArgs {
    pub shared_mem_phys_addr: u64,        // Physical address of shared memory
    pub page_table_entry_count: u32,      // Number of PTEs (pages)
    pub cmd_queue_offset: u32,            // Offset of Command Queue in shared mem
    pub stat_queue_offset: u32,           // Offset of Status Queue
    pub lockless_cmd_queue_offset: u32,   // Lockless queue (optional, 0)
    pub lockless_stat_queue_offset: u32,  // Lockless queue (optional, 0)
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
// LibOS Memory Region Descriptor
// Fuente: nouveau tu102.c — gsp_libos_memory_region
// GSP expects exactly 4 regions: LOGINIT, LOGINTR, LOGRM, RMARGS
// ═══════════════════════════════════════════════════════════════
#[repr(C)]
pub struct LibosMemoryRegion {
    pub pa: u64,       // Physical address (sysmem)
    pub size: u64,     // Size in bytes
    pub kind: u32,     // 0=LOGINIT, 1=LOGINTR, 2=LOGRM, 3=RMARGS
    pub _pad: u32,
}

// Region kinds (from nouveau)
pub const LIBOS_REGION_LOGINIT: u32 = 0; // Init log buffer (GSP writes boot log here)
pub const LIBOS_REGION_LOGINTR: u32 = 1; // Interrupt log buffer
pub const LIBOS_REGION_LOGRM:   u32 = 2; // RM log buffer (main debug log)
pub const LIBOS_REGION_RMARGS:  u32 = 3; // RM arguments (config data)

// Region sizes (from nouveau defaults)
pub const LOGINIT_SIZE: usize = 0x1_0000;  // 64KB
pub const LOGINTR_SIZE: usize = 0x1_0000;  // 64KB
pub const LOGRM_SIZE:   usize = 0x10_0000; // 1MB
pub const RMARGS_SIZE:  usize = 0x1000;    // 4KB

// ═══════════════════════════════════════════════════════════════
// GSP_ARGUMENTS_CACHED — Main boot arguments structure
// Passed to GSP via MAILBOX0/MAILBOX1 before boot
// ═══════════════════════════════════════════════════════════════
#[repr(C, align(4096))]
pub struct GspArgumentsCached {
    pub mq_init: MessageQueueInitArgs,         // Message queue setup
    pub sr_init: GspSrInitArgs,                // Suspend/Resume args
    pub gpu_instance: u32,                     // 0 for primary GPU
    pub profiler_pa: u64,                      // 0 (no profiler)
    pub profiler_size: u64,                    // 0
    // LibOS init regions (4 regions * 24 bytes = 96 bytes)
    pub libos_regions: [LibosMemoryRegion; 4], // LOGINIT, LOGINTR, LOGRM, RMARGS
    pub num_regions: u32,                      // = 4
    pub _pad: [u8; 4096 - 164],               // Pad to page boundary
}

impl GspArgumentsCached {
    /// Create boot args with message queues and LibOS memory regions configured.
    ///
    /// # Arguments
    /// * `shared_mem_phys` - Physical address of shared memory (cmdq+msgq)
    /// * `shared_mem_pages` - Number of pages in shared memory
    /// * `loginit_phys` - Physical address of LOGINIT buffer (64KB)
    /// * `logintr_phys` - Physical address of LOGINTR buffer (64KB)
    /// * `logrm_phys` - Physical address of LOGRM buffer (1MB)
    /// * `rmargs_phys` - Physical address of RMARGS buffer (4KB)
    pub fn new(
        shared_mem_phys: u64,
        shared_mem_pages: u32,
        loginit_phys: u64,
        logintr_phys: u64,
        logrm_phys: u64,
        rmargs_phys: u64,
    ) -> Self {
        Self {
            mq_init: MessageQueueInitArgs {
                shared_mem_phys_addr: shared_mem_phys,
                page_table_entry_count: shared_mem_pages,
                cmd_queue_offset: 0x0000,                // CmdQ at start
                stat_queue_offset: super::rpc::CMDQ_SIZE as u32, // MsgQ after CmdQ
                lockless_cmd_queue_offset: 0,
                lockless_stat_queue_offset: 0,
            },
            sr_init: GspSrInitArgs {
                old_level: 0,
                flags: 0,
                in_pm_transition: 0,
            },
            gpu_instance: 0,
            profiler_pa: 0,
            profiler_size: 0,
            libos_regions: [
                LibosMemoryRegion {
                    pa: loginit_phys,
                    size: LOGINIT_SIZE as u64,
                    kind: LIBOS_REGION_LOGINIT,
                    _pad: 0,
                },
                LibosMemoryRegion {
                    pa: logintr_phys,
                    size: LOGINTR_SIZE as u64,
                    kind: LIBOS_REGION_LOGINTR,
                    _pad: 0,
                },
                LibosMemoryRegion {
                    pa: logrm_phys,
                    size: LOGRM_SIZE as u64,
                    kind: LIBOS_REGION_LOGRM,
                    _pad: 0,
                },
                LibosMemoryRegion {
                    pa: rmargs_phys,
                    size: RMARGS_SIZE as u64,
                    kind: LIBOS_REGION_RMARGS,
                    _pad: 0,
                },
            ],
            num_regions: 4,
            _pad: [0u8; 4096 - 164],
        }
    }

    /// Simplified constructor for when LibOS regions are not yet allocated.
    /// Uses shared_mem_phys + offsets for the log regions.
    pub fn new_simple(shared_mem_phys: u64, shared_mem_pages: u32) -> Self {
        // Allocate regions after the message queues in the shared memory block
        let base = shared_mem_phys + (shared_mem_pages as u64 * 4096);
        Self::new(
            shared_mem_phys,
            shared_mem_pages,
            base,                                           // LOGINIT
            base + LOGINIT_SIZE as u64,                     // LOGINTR
            base + LOGINIT_SIZE as u64 + LOGINTR_SIZE as u64, // LOGRM
            base + LOGINIT_SIZE as u64 + LOGINTR_SIZE as u64 + LOGRM_SIZE as u64, // RMARGS
        )
    }
}
