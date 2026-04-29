//! GSP-RM RPC — Protocolo real de comunicación con el GSP (Ampere GA10x)
//!
//! Basado en nvidia-open-gpu-kernel-modules + nouveau (Linux kernel):
//!   - nvidia-open/inc/kernel/gpu/gsp/message_queue_priv.h → GSP_MSG_QUEUE_ELEMENT
//!   - nvidia-open/generated/g_rpc-message-header.h → rpc_message_header_v03_00
//!   - nvidia-open/inc/kernel/vgpu/rpc_global_enums.h → NV_VGPU_MSG_FUNCTION_*
//!   - nouveau/nvkm/subdev/gsp/rm/r535/rpc.c → send/recv implementation
//!
//! Estructura de un mensaje GSP:
//!   [GSP_MSG_QUEUE_ELEMENT (48 bytes)] + [RpcMessageHeader (32 bytes)] + [payload]
//!
//! La comunicación usa DOS colas en memoria compartida (SYSMEM):
//!   - Command Queue (cmdq): CPU → GSP  (256KB, nosotros escribimos)
//!   - Message Queue  (msgq): GSP → CPU  (256KB, GSP escribe respuestas/eventos)
//!
//! Cada cola es un ring buffer. Doorbell via QUEUE_HEAD MMIO registers.

use crate::console::Console;

// ═══════════════════════════════════════════════════════════════
// GSP_MSG_QUEUE_ELEMENT — Transport wrapper (48 bytes)
// Fuente: nvidia-open/inc/kernel/gpu/gsp/message_queue_priv.h
// ═══════════════════════════════════════════════════════════════
#[repr(C)]
pub struct GspMsgQueueElement {
    pub auth_tag_buffer: [u8; 16],  // +0:  AES-GCM auth tag (zeros for now)
    pub aad_buffer: [u8; 16],       // +16: Additional auth data (zeros)
    pub checksum: u32,              // +32: XOR of all u32 words == 0
    pub sequence: u32,              // +36: Monotonic per queue
    pub elem_count: u32,            // +40: Pages (4KB) this element occupies (1-16)
    pub pad: u32,                   // +44: Padding
    // RpcMessageHeader follows at +48, aligned to 8 bytes
}

// ═══════════════════════════════════════════════════════════════
// RpcMessageHeader — rpc_message_header_v03_00 (32 bytes)
// Fuente: nvidia-open/generated/g_rpc-message-header.h
// NOTA: signature = "VRPC" = 0x43505256, NO "VNKV"
// ═══════════════════════════════════════════════════════════════
#[repr(C)]
pub struct RpcMessageHeader {
    pub header_version: u32,        // +0:  0x03000000
    pub signature: u32,             // +4:  0x43505256 ("VRPC" in LE)
    pub length: u32,                // +8:  sizeof(header) + payload
    pub function: u32,              // +12: NV_VGPU_MSG_FUNCTION_*
    pub rpc_result: u32,            // +16: 0 = success
    pub rpc_result_private: u32,    // +20
    pub sequence: u32,              // +24
    pub spare: u32,                 // +28: cpuRmGfid
}

pub const RPC_SIGNATURE: u32 = 0x43505256;       // "VRPC" in LE
pub const RPC_HEADER_VERSION: u32 = 0x0300_0000;  // v03.00
pub const GSP_MSG_ELEM_SIZE: usize = 48;          // sizeof(GspMsgQueueElement)
pub const RPC_HDR_SIZE: usize = 32;               // sizeof(RpcMessageHeader)
pub const GSP_PAGE_SIZE: usize = 4096;

// Backward compat aliases used by other modules
pub const GSP_MSG_HDR_SIZE: usize = GSP_MSG_ELEM_SIZE;
pub const GSP_RPC_HDR_SIZE: usize = RPC_HDR_SIZE;

// ═══════════════════════════════════════════════════════════════
// NV_VGPU_MSG_FUNCTION IDs — REAL values
// Fuente: nvidia-open/inc/kernel/vgpu/rpc_global_enums.h
// ═══════════════════════════════════════════════════════════════
pub mod rpc_fn {
    pub const FREE: u32                     = 10;
    pub const UNLOADING_GUEST_DRIVER: u32   = 47;
    pub const GET_GSP_STATIC_INFO: u32      = 65;
    pub const CONTINUATION_RECORD: u32      = 71;
    pub const GSP_SET_SYSTEM_INFO: u32      = 72;
    pub const SET_REGISTRY: u32             = 73;
    pub const GSP_RM_CONTROL: u32           = 76;   // Wraps any NVxxxx_CTRL_CMD_*
    pub const GSP_RM_ALLOC: u32             = 103;  // Allocates any RM object

    // Events (GSP → CPU, on message queue)
    pub const EVENT_GSP_INIT_DONE: u32      = 0x1001;
    pub const EVENT_POST_EVENT: u32         = 0x1003;
    pub const EVENT_RC_TRIGGERED: u32       = 0x1004;
    pub const EVENT_MMU_FAULT: u32          = 0x1005;
    pub const EVENT_RUN_CPU_SEQ: u32        = 0x1006;
    pub const EVENT_OS_ERROR_LOG: u32       = 0x1007;
}

// Backward compat aliases (old names → real IDs)
pub const RPC_GSP_INIT:          u32 = rpc_fn::GET_GSP_STATIC_INFO; // was 0x01
pub const RPC_SET_SYSTEM_INFO:   u32 = rpc_fn::GSP_SET_SYSTEM_INFO; // was 0x02
pub const RPC_ALLOC_RESOURCE:    u32 = rpc_fn::GSP_RM_ALLOC;        // was 0x04
pub const RPC_FREE_RESOURCE:     u32 = rpc_fn::FREE;                // was 0x05
pub const RPC_CONTROL:           u32 = rpc_fn::GSP_RM_CONTROL;      // was 0x0A
pub const RPC_SET_REGISTRY:      u32 = rpc_fn::SET_REGISTRY;
pub const RPC_GSP_INIT_POST:     u32 = rpc_fn::GET_GSP_STATIC_INFO; // maps to static info

// ═══════════════════════════════════════════════════════════════
// GSP_RM_ALLOC Payload — Para crear CUALQUIER objeto RM
// Fuente: nvidia-open kernel_gsp.c
// ═══════════════════════════════════════════════════════════════
#[repr(C)]
pub struct RpcGspRmAlloc {
    pub h_client: u32,
    pub h_parent: u32,
    pub h_object: u32,
    pub h_class: u32,           // e.g. 0xC670 for display
    pub params_size: u32,
    // params: [u8] follows (variable)
}

// ═══════════════════════════════════════════════════════════════
// GSP_RM_CONTROL Payload — Para CUALQUIER control call
// ═══════════════════════════════════════════════════════════════
#[repr(C)]
pub struct RpcGspRmControl {
    pub h_client: u32,
    pub h_object: u32,
    pub cmd: u32,               // e.g. NV2080_CTRL_CMD_*
    pub status: u32,
    pub params_size: u32,
    // params: [u8] follows (variable)
}

// ═══════════════════════════════════════════════════════════════
// Clases Ampere GA10x — van en h_class del RpcGspRmAlloc
// ═══════════════════════════════════════════════════════════════
pub const NV_CLASS_DISPLAY_GA10X: u32  = 0xC670;
pub const NV_CLASS_3D_GA10X: u32       = 0xC697;
pub const NV_CLASS_COMPUTE_GA10X: u32  = 0xC6C0;
pub const NV_CLASS_DMA_COPY_GA10X: u32 = 0xC6B5;
pub const NV_CLASS_CHANNEL_GA10X: u32  = 0xC36F; // AMPERE_CHANNEL_GPFIFO (was 0xC66F)
pub const NV_CLASS_VA_SPACE_GA10X: u32 = 0xC6FA;

// Display sub-classes (GA10x Ampere)
pub mod display_class {
    pub const NVC670_DISPLAY: u32              = 0xC670; // Container
    pub const NVC67D_CORE_CHANNEL_DMA: u32     = 0xC67D; // Core channel
    pub const NVC67A_CURSOR_CHANNEL: u32       = 0xC67A; // Per-head cursor
    pub const NVC67B_WINDOW_CHANNEL: u32       = 0xC67B; // Window/overlay
    pub const NVC67E_WINDOW_IMM_CHANNEL: u32   = 0xC67E; // Immediate flip
}

// ═══════════════════════════════════════════════════════════════
// PGSP Registers (BAR0 offsets) — CORRECTED from nvidia-open + nouveau
// ═══════════════════════════════════════════════════════════════

// Falcon core registers (confirmed working via loader.rs)
pub const NV_PGSP_FALCON_MAILBOX0: u32 = 0x0011_0040; // Lo32 of libos args PA
pub const NV_PGSP_FALCON_MAILBOX1: u32 = 0x0011_0044; // Hi32 of libos args PA

// Message Queue doorbell registers (ring buffer HEAD/TAIL)
// Fuente: nvidia-open gsp_fw_wpr_meta.h, nouveau tu102.c
pub const NV_PGSP_QUEUE_HEAD_BASE: u32 = 0x0011_0C00;
/// Queue HEAD register for queue `q` (0=cmdq, 1=msgq)
pub const fn pgsp_queue_head(q: u32) -> u32 { NV_PGSP_QUEUE_HEAD_BASE + q * 8 }
/// Queue TAIL register for queue `q`
pub const fn pgsp_queue_tail(q: u32) -> u32 { NV_PGSP_QUEUE_HEAD_BASE + q * 8 + 4 }

// Backward compat aliases
pub const NV_PGSP_MSGQ_HEAD:  u32 = 0x0011_0C00; // Queue 0 HEAD (cmdq)
pub const NV_PGSP_MSGQ_TAIL:  u32 = 0x0011_0C04; // Queue 0 TAIL (cmdq)
pub const NV_PGSP_STATQ_HEAD: u32 = 0x0011_0C08; // Queue 1 HEAD (msgq)
pub const NV_PGSP_STATQ_TAIL: u32 = 0x0011_0C0C; // Queue 1 TAIL (msgq)

// RISC-V boot control (Ampere GSP uses RISC-V, not classic Falcon)
pub const NV_PGSP_RISCV_CPUCTL:   u32 = 0x0011_0388;
pub const NV_PGSP_RISCV_BR_ADDR:  u32 = 0x0011_0390; // Branch address

// GSP RISC-V mode switch (Ampere — from nouveau ga102_gsp_reset)
pub const NV_PGSP_RISCV_MODE:      u32 = 0x0011_1668;
pub const NV_PGSP_RISCV_MODE_MASK: u32 = 0x0000_0111;

// WPR2 status register — nonzero after FWSEC/FRTS sets up write-protected region
pub const NV_WPR2_HI: u32 = 0x001F_A828;

// ── SEC2 Falcon registers (BAR0 offsets) ──
// SEC2 runs booter_load HS firmware on Ampere (NOT PGSP directly)
pub const NV_PSEC2_FALCON_MAILBOX0: u32 = 0x0084_0040;
pub const NV_PSEC2_FALCON_MAILBOX1: u32 = 0x0084_0044;
pub const NV_PSEC2_FALCON_SCRATCH0: u32 = 0x0084_0080;
pub const NV_PSEC2_FALCON_CPUCTL:   u32 = 0x0084_0100;
pub const NV_PSEC2_FALCON_BOOTVEC:  u32 = 0x0084_0104;
pub const NV_PSEC2_FALCON_IDLESTATE: u32 = 0x0084_0004;
pub const NV_PSEC2_FALCON_RESET:    u32 = 0x0084_0094;
pub const NV_PSEC2_FALCON_ENGINE:   u32 = 0x0084_03C0;
pub const NV_PSEC2_DMATRFBASE:      u32 = 0x0084_0110;
pub const NV_PSEC2_DMATRFMOFFS:     u32 = 0x0084_0114;
pub const NV_PSEC2_DMATRFCMD:       u32 = 0x0084_0118;
pub const NV_PSEC2_DMATRFFBOFFS:    u32 = 0x0084_011C;

// ── SEC2 Falcon extended registers (addr2 = +0x1000 from SEC2 base 0x840000) ──
// These are needed for GA102+ HS authenticated boot (booter_load)
pub const NV_PSEC2_FALCON_EMEM_ACCESS: u32 = 0x0084_1180;
pub const NV_PSEC2_FALCON_UCODE_ID:   u32 = 0x0084_1198;
pub const NV_PSEC2_FALCON_ENGINE_ID:  u32 = 0x0084_119C;
pub const NV_PSEC2_FALCON_DMEM_SIGN:  u32 = 0x0084_1210;
// DMA/cache control (from ga102_flcn_fw_load)
pub const NV_PSEC2_FALCON_DMACTL:     u32 = 0x0084_0600;
pub const NV_PSEC2_FALCON_IRQMSET:    u32 = 0x0084_0624;
pub const NV_PSEC2_FALCON_ENGCTL:     u32 = 0x0084_010C;

// Display / Graph / FIFO base addresses
pub const NV_PDISP_BASE: u32      = 0x0061_0000;
pub const NV_PDISP_HEAD_BASE: u32 = 0x0064_0000;
pub const NV_PGRAPH_BASE: u32     = 0x0080_0000;
pub const NV_PFIFO_BASE: u32      = 0x0002_0000;

// ═══════════════════════════════════════════════════════════════
// Queue sizes — nouveau uses 256KB per queue (64 pages)
// ═══════════════════════════════════════════════════════════════
pub const CMDQ_SIZE: usize = 0x4_0000; // 256KB command queue
pub const MSGQ_SIZE: usize = 0x4_0000; // 256KB message queue
const CMDQ_PAGES: usize = CMDQ_SIZE / GSP_PAGE_SIZE;
const MSGQ_PAGES: usize = MSGQ_SIZE / GSP_PAGE_SIZE;
const TOTAL_QUEUE_PAGES: usize = CMDQ_PAGES + MSGQ_PAGES; // 128 pages

// ═══════════════════════════════════════════════════════════════
// GspFwWprMeta — 256-byte VRAM layout descriptor
// Fuente: nvidia-open/arch/nvalloc/common/inc/gsp/gsp_fw_wpr_meta.h
// ═══════════════════════════════════════════════════════════════
pub const WPR_META_MAGIC: u64 = 0xdc3a_ae21_371a_60b3;
pub const WPR_META_REVISION: u64 = 1;
pub const WPR_VERIFIED_MAGIC: u64 = 0xa0a0_a0a0_a0a0_a0a0;

#[repr(C)]
pub struct GspFwWprMeta {
    pub magic: u64,                         // 0xdc3aae21371a60b3
    pub revision: u64,                      // = 1
    pub sysmem_addr_of_radix3_elf: u64,
    pub size_of_radix3_elf: u64,
    pub sysmem_addr_of_bootloader: u64,
    pub size_of_bootloader: u64,
    pub bootloader_code_offset: u64,
    pub bootloader_data_offset: u64,
    pub bootloader_manifest_offset: u64,
    pub sysmem_addr_of_signature: u64,
    pub size_of_signature: u64,
    // FB (VRAM) layout — calculated top-down from fb_size:
    pub gsp_fw_rsvd_start: u64,
    pub non_wpr_heap_offset: u64,
    pub non_wpr_heap_size: u64,
    pub gsp_fw_wpr_start: u64,             // 128KB aligned
    pub gsp_fw_heap_offset: u64,           // 1MB aligned
    pub gsp_fw_heap_size: u64,
    pub gsp_fw_offset: u64,                // 64KB aligned, ELF in VRAM
    pub boot_bin_offset: u64,              // 4KB aligned
    pub frts_offset: u64,
    pub frts_size: u64,
    pub gsp_fw_wpr_end: u64,
    pub fb_size: u64,
    pub vga_workspace_offset: u64,
    pub vga_workspace_size: u64,
    pub boot_count: u64,
    pub verified: u64,                     // 0xa0a0a0a0a0a0a0a0 when verified
    pub flags: u8,
    pub _pad: [u8; 7],
}
// static_assert: size == 256 bytes (32 u64 fields = 256)

// ═══════════════════════════════════════════════════════════════
// FIFO Channel class (Ampere)
// ═══════════════════════════════════════════════════════════════
pub const AMPERE_CHANNEL_GPFIFO: u32 = 0xC36F;

#[repr(C)]
pub struct MemoryInfo {
    pub base: u64,
    pub size: u64,
    pub address_space: u32,     // 1=SYSMEM, 2=FBMEM
    pub cache_attrib: u32,
}

#[repr(C)]
pub struct ChannelGpfifoAllocParams {
    pub gp_fifo_offset: u64,
    pub gp_fifo_entries: u32,
    pub flags: u32,
    pub h_va_space: u32,
    pub engine_type: u32,
    pub instance_mem: MemoryInfo,
    pub userd_mem: MemoryInfo,
    pub ramfc_mem: MemoryInfo,
    pub mthdbuf_mem: MemoryInfo,
    pub internal_flags: u32,
}

// ═══════════════════════════════════════════════════════════════
// Display allocation params
// ═══════════════════════════════════════════════════════════════
#[repr(C)]
pub struct Nvc670AllocationParams {
    pub num_heads: u32,
    pub num_sors: u32,
    pub num_dsis: u32,
}

// ═══════════════════════════════════════════════════════════════
// GspRpcRing — Real message queue implementation
// ═══════════════════════════════════════════════════════════════

pub struct GspRpcRing<'a> {
    bar0: &'a nv_hal::MmioRegion,
    // Command queue (CPU → GSP) — 256KB
    cmdq_phys: u64,
    cmdq_virt: *mut u8,
    // Message queue (GSP → CPU) — 256KB
    msgq_phys: u64,
    msgq_virt: *mut u8,
    // Sequence counter (monotonic per queue)
    cmd_seq: u32,
    // RM handle tracking
    next_handle: u32,
}

impl<'a> GspRpcRing<'a> {
    /// Create a new RPC ring. `base_phys` must point to 512KB of contiguous,
    /// page-aligned, zeroed physical memory (identity-mapped).
    pub fn new(bar0: &'a nv_hal::MmioRegion, base_phys: u64) -> Self {
        let msgq_phys = base_phys + CMDQ_SIZE as u64;
        Self {
            bar0,
            cmdq_phys: base_phys,
            cmdq_virt: base_phys as *mut u8,
            msgq_phys,
            msgq_virt: msgq_phys as *mut u8,
            cmd_seq: 1,
            next_handle: 0x0000_DEAD,
        }
    }

    /// Initialize both queues, zero memory, set MMIO doorbell registers.
    pub fn init(&mut self, con: &mut Console) {
        con.print_colored("=== GSP RPC Ring Init (256KB x2) ===\n", crate::fb::colors::ACCENT_CYAN);

        // Zero both queues
        unsafe {
            core::ptr::write_bytes(self.cmdq_virt, 0, CMDQ_SIZE);
            core::ptr::write_bytes(self.msgq_virt, 0, MSGQ_SIZE);
        }

        // Reset queue HEAD/TAIL doorbells to 0 (empty)
        self.bar0.write32(pgsp_queue_head(0), 0); // cmdq HEAD
        self.bar0.write32(pgsp_queue_tail(0), 0); // cmdq TAIL
        self.bar0.write32(pgsp_queue_head(1), 0); // msgq HEAD
        self.bar0.write32(pgsp_queue_tail(1), 0); // msgq TAIL

        // Verify readback
        let h0 = self.bar0.read32(pgsp_queue_head(0));
        let t0 = self.bar0.read32(pgsp_queue_tail(0));
        con.print("  [RPC] CMDQ HEAD=0x");
        con.print_hex32(h0);
        con.print(" TAIL=0x");
        con.print_hex32(t0);
        con.newline();

        let h1 = self.bar0.read32(pgsp_queue_head(1));
        let t1 = self.bar0.read32(pgsp_queue_tail(1));
        con.print("  [RPC] MSGQ HEAD=0x");
        con.print_hex32(h1);
        con.print(" TAIL=0x");
        con.print_hex32(t1);
        con.newline();

        con.print("  [RPC] CMDQ phys=0x");
        con.print_hex32((self.cmdq_phys >> 32) as u32);
        con.print_hex32(self.cmdq_phys as u32);
        con.print(" MSGQ phys=0x");
        con.print_hex32((self.msgq_phys >> 32) as u32);
        con.print_hex32(self.msgq_phys as u32);
        con.newline();

        con.print_colored("=== GSP RPC Ring READY ===\n", crate::fb::colors::TEXT_SUCCESS);
    }

    /// Allocate a unique RM handle for object tracking
    pub fn alloc_handle(&mut self) -> u32 {
        let h = self.next_handle;
        self.next_handle += 1;
        h
    }

    /// Compute checksum so XOR of all u32 words in the element == 0
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

    /// Build and send a complete RPC message:
    ///   [GspMsgQueueElement (48B)] + [RpcMessageHeader (32B)] + [payload]
    ///
    /// The element is written at `cmdq_virt + write_offset`, then the cmdq TAIL
    /// doorbell is updated to notify GSP.
    pub fn send_rpc(&mut self, function: u32, payload: &[u8],
                    con: &mut Console) -> Result<(), &'static str> {
        let seq = self.cmd_seq;
        self.cmd_seq += 1;

        let rpc_length = RPC_HDR_SIZE as u32 + payload.len() as u32;
        let total_elem = GSP_MSG_ELEM_SIZE + rpc_length as usize;
        // Round up to 4KB page boundary (GSP requirement)
        let elem_pages = (total_elem + GSP_PAGE_SIZE - 1) / GSP_PAGE_SIZE;
        let elem_size_aligned = elem_pages * GSP_PAGE_SIZE;

        // Get current TAIL (our write offset in the ring)
        let tail = self.bar0.read32(pgsp_queue_head(0));
        let write_off = (tail as usize) % CMDQ_SIZE;

        // Check space
        if write_off + elem_size_aligned > CMDQ_SIZE {
            // Wrap — for now, reset to 0 (simple approach)
            self.bar0.write32(pgsp_queue_tail(0), 0);
        }

        let base = unsafe { self.cmdq_virt.add(write_off) };

        con.print("  [RPC] fn=");
        con.print_hex32(function);
        con.print(" len=");
        con.print_hex32(total_elem as u32);
        con.print(" seq=");
        con.print_hex32(seq);
        con.newline();

        // Zero the entire element
        unsafe { core::ptr::write_bytes(base, 0, elem_size_aligned); }

        // Write GspMsgQueueElement
        let elem = base as *mut GspMsgQueueElement;
        unsafe {
            (*elem).sequence = seq;
            (*elem).elem_count = elem_pages as u32;
            (*elem).pad = 0;
        }

        // Write RpcMessageHeader at +48
        let rpc_hdr = unsafe { base.add(GSP_MSG_ELEM_SIZE) } as *mut RpcMessageHeader;
        unsafe {
            (*rpc_hdr).header_version = RPC_HEADER_VERSION;
            (*rpc_hdr).signature = RPC_SIGNATURE;
            (*rpc_hdr).length = rpc_length;
            (*rpc_hdr).function = function;
            (*rpc_hdr).rpc_result = 0xFFFF_FFFF; // pending
            (*rpc_hdr).rpc_result_private = 0;
            (*rpc_hdr).sequence = seq;
            (*rpc_hdr).spare = 0; // cpuRmGfid = 0 for physical GPU
        }

        // Copy payload after RPC header
        if !payload.is_empty() {
            let payload_dst = unsafe { base.add(GSP_MSG_ELEM_SIZE + RPC_HDR_SIZE) };
            unsafe {
                core::ptr::copy_nonoverlapping(payload.as_ptr(), payload_dst, payload.len());
            }
        }

        // Compute and set checksum (XOR of all u32s in element must == 0)
        unsafe {
            (*elem).checksum = 0;
            (*elem).checksum = Self::calc_checksum(base, total_elem);
        }

        // Ring doorbell: update TAIL to notify GSP of new message
        let new_tail = (write_off + elem_size_aligned) as u32;
        self.bar0.write32(pgsp_queue_tail(0), new_tail);

        // Poll for response on the message queue
        self.poll_response(seq, con)
    }

    /// Convenience: send RPC with no payload
    pub fn send_rpc_simple(&mut self, function: u32,
                           con: &mut Console) -> Result<(), &'static str> {
        self.send_rpc(function, &[], con)
    }

    /// Send GSP_RM_ALLOC RPC (fn=103) to create an RM object
    pub fn send_rm_alloc(&mut self, h_client: u32, h_parent: u32,
                         h_object: u32, h_class: u32,
                         con: &mut Console) -> Result<(), &'static str> {
        let alloc = RpcGspRmAlloc {
            h_client,
            h_parent,
            h_object,
            h_class,
            params_size: 0,
        };
        let payload = unsafe {
            core::slice::from_raw_parts(
                &alloc as *const RpcGspRmAlloc as *const u8,
                core::mem::size_of::<RpcGspRmAlloc>(),
            )
        };
        self.send_rpc(rpc_fn::GSP_RM_ALLOC, payload, con)
    }

    /// Send GSP_RM_CONTROL RPC (fn=76)
    pub fn send_rm_control(&mut self, h_client: u32, h_object: u32,
                           cmd: u32, con: &mut Console) -> Result<(), &'static str> {
        let ctrl = RpcGspRmControl {
            h_client,
            h_object,
            cmd,
            status: 0,
            params_size: 0,
        };
        let payload = unsafe {
            core::slice::from_raw_parts(
                &ctrl as *const RpcGspRmControl as *const u8,
                core::mem::size_of::<RpcGspRmControl>(),
            )
        };
        self.send_rpc(rpc_fn::GSP_RM_CONTROL, payload, con)
    }

    /// Poll message queue for a response matching `expected_seq`.
    /// Also handles GSP events (INIT_DONE, RUN_CPU_SEQ, etc).
    fn poll_response(&self, expected_seq: u32,
                     con: &mut Console) -> Result<(), &'static str> {
        con.print("  [RPC] polling ");

        for i in 0..200u32 {
            // Check message queue TAIL — if it advanced, GSP wrote something
            let msgq_head = self.bar0.read32(pgsp_queue_head(1));
            let msgq_tail = self.bar0.read32(pgsp_queue_tail(1));

            if msgq_tail != msgq_head {
                // Read the element from msgq
                let read_off = (msgq_head as usize) % MSGQ_SIZE;
                let elem_base = unsafe { self.msgq_virt.add(read_off) };
                let rpc_hdr = unsafe {
                    &*(elem_base.add(GSP_MSG_ELEM_SIZE) as *const RpcMessageHeader)
                };

                let fn_id = rpc_hdr.function;
                let result = rpc_hdr.rpc_result;
                let resp_seq = rpc_hdr.sequence;

                con.print(" ev=0x");
                con.print_hex32(fn_id);

                // Handle GSP events
                match fn_id {
                    rpc_fn::EVENT_GSP_INIT_DONE => {
                        con.print_colored(" GSP_INIT_DONE!", crate::fb::colors::TEXT_SUCCESS);
                    }
                    rpc_fn::EVENT_RUN_CPU_SEQ => {
                        con.print(" RUN_CPU_SEQ");
                    }
                    rpc_fn::EVENT_OS_ERROR_LOG => {
                        con.print_colored(" OS_ERROR", crate::fb::colors::ACCENT_RED);
                    }
                    _ => {}
                }

                // Advance msgq HEAD (consume the message)
                let elem_count = unsafe {
                    (*(elem_base as *const GspMsgQueueElement)).elem_count
                };
                let consumed = (elem_count as usize).max(1) * GSP_PAGE_SIZE;
                let new_head = ((read_off + consumed) % MSGQ_SIZE) as u32;
                self.bar0.write32(pgsp_queue_head(1), new_head);

                // Check if this is our response
                if resp_seq == expected_seq {
                    if result == 0 {
                        con.print_colored(" OK\n", crate::fb::colors::TEXT_SUCCESS);
                        return Ok(());
                    } else {
                        con.print(" err=0x");
                        con.print_hex32(result);
                        con.print_colored(" FAIL\n", crate::fb::colors::ACCENT_RED);
                        return Err("GSP RPC failed");
                    }
                }
                // Not our response — was an event, continue polling
            }

            // Also check MAILBOX1 for booter-style responses
            if i == 100 {
                let mb0 = self.bar0.read32(NV_PGSP_FALCON_MAILBOX0);
                let mb1 = self.bar0.read32(NV_PGSP_FALCON_MAILBOX1);
                if mb1 != 0 {
                    con.print(" MB0=0x");
                    con.print_hex32(mb0);
                    con.print(" MB1=0x");
                    con.print_hex32(mb1);
                }
            }

            if i % 40 == 0 { con.print("."); }
            for _ in 0..500_000 { core::hint::spin_loop(); }
        }

        con.print_colored(" TIMEOUT\n", crate::fb::colors::ACCENT_RED);
        Err("GSP RPC timeout")
    }

    /// Wait specifically for EVENT_GSP_INIT_DONE (0x1001) on the message queue.
    /// This is the first event GSP sends after successful boot + libos init.
    pub fn wait_gsp_init_done(&self, con: &mut Console) -> Result<(), &'static str> {
        con.print_colored("  [RPC] Waiting for GSP_INIT_DONE (0x1001)...\n",
            crate::fb::colors::ACCENT_CYAN);

        for i in 0..500u32 {
            let msgq_head = self.bar0.read32(pgsp_queue_head(1));
            let msgq_tail = self.bar0.read32(pgsp_queue_tail(1));

            if msgq_tail != msgq_head {
                let read_off = (msgq_head as usize) % MSGQ_SIZE;
                let elem_base = unsafe { self.msgq_virt.add(read_off) };
                let rpc_hdr = unsafe {
                    &*(elem_base.add(GSP_MSG_ELEM_SIZE) as *const RpcMessageHeader)
                };

                if rpc_hdr.function == rpc_fn::EVENT_GSP_INIT_DONE {
                    con.print_colored("  [RPC] GSP_INIT_DONE received!\n",
                        crate::fb::colors::TEXT_SUCCESS);

                    // Consume the event
                    let elem_count = unsafe {
                        (*(elem_base as *const GspMsgQueueElement)).elem_count
                    };
                    let consumed = (elem_count as usize).max(1) * GSP_PAGE_SIZE;
                    let new_head = ((read_off + consumed) % MSGQ_SIZE) as u32;
                    self.bar0.write32(pgsp_queue_head(1), new_head);
                    return Ok(());
                }

                // Handle other events while waiting
                if rpc_hdr.function == rpc_fn::EVENT_RUN_CPU_SEQ {
                    con.println("  [RPC] EVENT: RUN_CPU_SEQUENCER (handling...)");
                }

                // Consume and continue
                let elem_count = unsafe {
                    (*(elem_base as *const GspMsgQueueElement)).elem_count
                };
                let consumed = (elem_count as usize).max(1) * GSP_PAGE_SIZE;
                let new_head = ((read_off + consumed) % MSGQ_SIZE) as u32;
                self.bar0.write32(pgsp_queue_head(1), new_head);
            }

            if i % 100 == 0 && i > 0 { con.print("."); }
            for _ in 0..1_000_000 { core::hint::spin_loop(); }
        }

        con.print_colored("  [RPC] GSP_INIT_DONE timeout\n", crate::fb::colors::ACCENT_RED);
        Err("GSP_INIT_DONE timeout")
    }
}
