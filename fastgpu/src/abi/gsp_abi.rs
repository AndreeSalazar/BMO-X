#![no_std]

/// SigDead-BIB Extracted: NVIDIA GSP Boot Parameters for GA106 (Ampere)
/// 
/// This structure is passed to the GSP-RM firmware during the MSG_INIT sequence.
/// Magic signature: "VNKV" (0x564B4E56)
#[repr(C, packed)]
pub struct GspBootParams {
    pub signature: u32,             // 0x00: 0x564B4E56
    pub version: u32,               // 0x04: 0x1 or 0x2
    pub wpr_base: u64,              // 0x08: Physical base of WPR region
    pub wpr_size: u64,              // 0x10: Size of WPR region
    
    // LibOS Log Regions (Extracted from nvlddmkm.sys)
    pub log_init_offset: u32,       // 0x18
    pub log_init_size: u32,         // 0x1C
    pub log_intr_offset: u32,       // 0x20
    pub log_intr_size: u32,         // 0x24
    pub log_rm_offset: u32,         // 0x28
    pub log_rm_size: u32,           // 0x2C
    
    // RPC Control Structures
    pub rpc_structure_offset: u32,  // 0x30
    pub rpc_structure_size: u32,    // 0x34
    
    // RISC-V Vectoring
    pub entry_point: u64,           // 0x38: RISCV_BR_ADDR target
}

impl GspBootParams {
    pub const SIGNATURE: u32 = 0x564B4E56; // "VNKV"
    
    pub fn new(wpr_base: u64, wpr_size: u64, entry: u64) -> Self {
        Self {
            signature: Self::SIGNATURE,
            version: 1,
            wpr_base,
            wpr_size,
            log_init_offset: 0,
            log_init_size: 0x40000,   // 256KB Standard
            log_intr_offset: 0x40000,
            log_intr_size: 0x10000,   // 64KB Standard
            log_rm_offset: 0x50000,
            log_rm_size: 0x100000,    // 1MB Standard
            rpc_structure_offset: 0x150000,
            rpc_structure_size: 0x10000,
            entry_point: entry,
        }
    }
}
