/// GSP Firmware Loader for GA10x (Ampere)
/// 
/// Handles loading the 36MB GSP-RM firmware, building the radix3 page tree,
/// and constructing the WPR metadata structure that booter_load reads via MAILBOX.

use alloc::vec::Vec;
use crate::console::Console;
use crate::fs::DiskReader;

/// GspFwWprMeta — 256 bytes, exactly matching nvidia-open gsp_fw_wpr_meta.h
/// Passed to booter_load via SEC2 MAILBOX0/1 (physical address in system memory)
#[repr(C, packed)]
pub struct GspFwWprMeta {
    pub magic: u64,                       // 0x00: 0xdc3aae21371a60b3
    pub revision: u64,                    // 0x08: 1

    // SYSMEM addresses (consumed by Booter for DMA)
    pub sysmem_addr_of_radix3_elf: u64,   // 0x10: phys addr of radix3 tree root
    pub size_of_radix3_elf: u64,          // 0x18: size of GSP firmware ELF

    pub sysmem_addr_of_bootloader: u64,   // 0x20: phys addr of boot binary
    pub size_of_bootloader: u64,          // 0x28: size of boot binary

    pub bootloader_code_offset: u64,      // 0x30
    pub bootloader_data_offset: u64,      // 0x38
    pub bootloader_manifest_offset: u64,  // 0x40

    pub sysmem_addr_of_signature: u64,    // 0x48
    pub size_of_signature: u64,           // 0x50

    // FB layout
    pub gsp_fw_rsvd_start: u64,           // 0x58
    pub non_wpr_heap_offset: u64,         // 0x60
    pub non_wpr_heap_size: u64,           // 0x68
    pub gsp_fw_wpr_start: u64,            // 0x70
    pub gsp_fw_heap_offset: u64,          // 0x78
    pub gsp_fw_heap_size: u64,            // 0x80
    pub gsp_fw_offset: u64,               // 0x88
    pub boot_bin_offset: u64,             // 0x90
    pub frts_offset: u64,                 // 0x98
    pub frts_size: u64,                   // 0xA0
    pub gsp_fw_wpr_end: u64,             // 0xA8
    pub fb_size: u64,                     // 0xB0
    pub vga_workspace_offset: u64,        // 0xB8
    pub vga_workspace_size: u64,          // 0xC0
    pub boot_count: u64,                  // 0xC8

    // Partition/CrashCat fields (union in C, we use the simpler form)
    pub partition_rpc_addr: u64,          // 0xD0
    pub partition_rpc_request_offset: u16,// 0xD8
    pub partition_rpc_reply_offset: u16,  // 0xDA
    pub elf_code_offset: u32,             // 0xDC
    pub elf_data_offset: u32,             // 0xE0
    pub elf_code_size: u32,               // 0xE4
    pub elf_data_size: u32,               // 0xE8
    pub ls_ucode_version: u32,            // 0xEC

    pub gsp_fw_heap_vf_partition_count: u8, // 0xF0
    pub flags: u8,                         // 0xF1
    pub padding: [u8; 2],                  // 0xF2
    pub pmu_reserved_size: u32,            // 0xF4
    pub verified: u64,                     // 0xF8
}

const GSP_FW_WPR_META_MAGIC: u64 = 0xdc3aae21371a60b3;
const GSP_FW_WPR_META_REVISION: u64 = 1;
const RADIX_PAGE_SIZE: u64 = 4096;
const RADIX_ENTRIES_PER_PAGE: u64 = 512; // 4096 / 8

/// Fixed physical addresses for GSP buffers (identity-mapped in FastOS)
/// These must be in available system RAM (>256MB, which is safe on 16GB+ systems)
const GSP_FW_BASE:    u64 = 0x2000_0000; // 512MB - GSP firmware blob
const RADIX3_BASE:    u64 = 0x4400_0000; // 1088MB - Radix3 tree 
const WPR_META_BASE:  u64 = 0x4800_0000; // 1152MB - WPR metadata (256 bytes)
const BOOT_BIN_BASE:  u64 = 0x4800_1000; // 1152MB+4K - Boot binary

/// Result of GSP firmware preparation
pub struct GspPrepared {
    pub wpr_meta_phys: u64,
    pub gsp_fw_loaded: bool,
}

/// Load GSP firmware from SATA into system memory and build radix3 tree + WPR metadata.
/// Returns the physical address of the WPR metadata for MAILBOX0/1.
pub fn prepare_gsp_firmware(
    con: &mut Console,
    ahci: &mut crate::drivers::ahci::AhciDriver,
    fb_size_mb: u64,
) -> Option<GspPrepared> {
    con.println("[GSP] Preparing GSP firmware...");

    // ── Step 1: Load GSP firmware from SATA ──
    // GSP firmware is written at LBA 4096 by write_gsp.ps1
    // Size: ~36MB = ~73728 sectors
    const GSP_FW_LBA_START: u64 = 4096;
    const GSP_FW_SIZE: usize = 38_061_600; // exact size of gsp-535.113.01.bin
    let sectors_needed = ((GSP_FW_SIZE + 511) / 512) as u32;
    
    con.print("  Loading GSP firmware: ");
    con.print_u64(GSP_FW_SIZE as u64 / 1024 / 1024);
    con.print("MB from LBA ");
    con.print_u64(GSP_FW_LBA_START as u64);
    con.println("...");
    
    let gsp_buf = unsafe {
        core::slice::from_raw_parts_mut(GSP_FW_BASE as *mut u8, (sectors_needed as usize) * 512)
    };
    
    // Read in chunks of 128 sectors (64KB) for reliability
    let chunk_size = 128u32;
    let mut loaded: u32 = 0;
    while loaded < sectors_needed {
        let remain = sectors_needed - loaded;
        let this_chunk = if remain > chunk_size { chunk_size } else { remain };
        let buf_offset = (loaded as usize) * 512;
        let chunk_buf = &mut gsp_buf[buf_offset..buf_offset + (this_chunk as usize) * 512];
        
        match ahci.read_sectors(GSP_FW_LBA_START + loaded as u64, this_chunk, chunk_buf) {
            Ok(()) => { loaded += this_chunk; },
            Err(_) => {
                con.print("  [ERROR] SATA read failed at sector ");
                con.print_u64(loaded as u64);
                con.println("");
                return None;
            }
        }
    }
    con.print("  Loaded ");
    con.print_u64(loaded as u64);
    con.println(" sectors OK");

    // ── Step 2: Build Radix3 tree ──
    // The radix3 tree is a 3-level page table that the Falcon DMA engine walks.
    // Level 0 (root): 1 page, entries point to L1 pages
    // Level 1: entries point to L2 pages  
    // Level 2: entries point to data pages (4KB each)
    con.println("  Building radix3 tree...");
    
    let n_data_pages = (GSP_FW_SIZE as u64 + RADIX_PAGE_SIZE - 1) / RADIX_PAGE_SIZE;
    let n_l2_pages = (n_data_pages + RADIX_ENTRIES_PER_PAGE - 1) / RADIX_ENTRIES_PER_PAGE;
    let n_l1_pages = (n_l2_pages + RADIX_ENTRIES_PER_PAGE - 1) / RADIX_ENTRIES_PER_PAGE;
    let total_tree_pages = 1 + n_l1_pages + n_l2_pages; // root + L1 + L2
    
    let tree_buf = unsafe {
        let size = (total_tree_pages * RADIX_PAGE_SIZE) as usize;
        let ptr = RADIX3_BASE as *mut u8;
        // Zero the tree area
        core::ptr::write_bytes(ptr, 0, size);
        core::slice::from_raw_parts_mut(ptr as *mut u64, size / 8)
    };
    
    // Fill Level 0 (root): entries pointing to L1 pages
    let l1_base = RADIX3_BASE + RADIX_PAGE_SIZE; // L1 starts after root
    for i in 0..n_l1_pages {
        tree_buf[i as usize] = l1_base + i * RADIX_PAGE_SIZE;
    }
    
    // Fill Level 1: entries pointing to L2 pages
    let l2_base = l1_base + n_l1_pages * RADIX_PAGE_SIZE;
    let l1_offset = (RADIX_PAGE_SIZE / 8) as usize; // offset in u64 array
    for i in 0..n_l2_pages {
        tree_buf[l1_offset + i as usize] = l2_base + i * RADIX_PAGE_SIZE;
    }
    
    // Fill Level 2: entries pointing to data pages
    let l2_offset = l1_offset + (n_l1_pages * RADIX_PAGE_SIZE / 8) as usize;
    for i in 0..n_data_pages {
        tree_buf[l2_offset + i as usize] = GSP_FW_BASE + i * RADIX_PAGE_SIZE;
    }
    
    con.print("  Radix3: L0=1 L1=");
    con.print_u64(n_l1_pages);
    con.print(" L2=");
    con.print_u64(n_l2_pages);
    con.print(" data=");
    con.print_u64(n_data_pages);
    con.println(" pages");

    // ── Step 3: Load GSP-RM Boot binary from SATA ──
    // Boot binary is written right after GSP firmware at LBA 78436
    // (4096 + 74340 sectors of GSP = 78436)
    // Size: 24576 bytes = 48 sectors
    const BOOT_BIN_LBA: u64 = 78436;
    const BOOT_BIN_SIZE: usize = 24576;
    let boot_sectors = ((BOOT_BIN_SIZE + 511) / 512) as u32;
    
    con.println("  Loading GSP-RM boot binary...");
    let boot_buf = unsafe {
        core::ptr::write_bytes(BOOT_BIN_BASE as *mut u8, 0, BOOT_BIN_SIZE);
        core::slice::from_raw_parts_mut(BOOT_BIN_BASE as *mut u8, boot_sectors as usize * 512)
    };
    match ahci.read_sectors(BOOT_BIN_LBA, boot_sectors, boot_buf) {
        Ok(()) => {
            con.print("  Boot binary: ");
            con.print_u64(BOOT_BIN_SIZE as u64);
            con.println("B OK");
        },
        Err(_) => {
            con.println("  [WARN] Boot binary not at SATA LBA 78436");
            con.println("  Continuing without boot binary...");
        }
    }

    // ── Step 4: Build WPR Metadata ──
    con.println("  Building WPR metadata...");
    let fb_size = fb_size_mb * 1024 * 1024; // 6GB for RTX 3060
    
    // Layout computation (from nvidia-open kgspPopulateWprMeta_TU102)
    let vga_workspace_size: u64 = 0x20000; // 128KB
    let vga_workspace_offset = fb_size - vga_workspace_size;
    let wpr_alignment: u64 = 0x20000; // 128KB
    let gsp_fw_wpr_end = (vga_workspace_offset) & !(wpr_alignment - 1);
    let frts_size: u64 = 0x100000; // 1MB
    let frts_offset = gsp_fw_wpr_end - frts_size;
    let boot_bin_offset = (frts_offset - 0x10000) & !0xFFF; // 4K aligned
    let gsp_fw_offset = (boot_bin_offset - GSP_FW_SIZE as u64) & !0xFFFF; // 64K aligned
    let heap_size: u64 = 64 * 1024 * 1024; // 64MB heap
    let gsp_fw_heap_offset = (gsp_fw_offset - heap_size) & !(0x100000 - 1);
    let gsp_fw_heap_size = (gsp_fw_offset - gsp_fw_heap_offset) & !(0x100000 - 1);
    let wpr_meta_size: u64 = 0x100000;
    let gsp_fw_wpr_start = gsp_fw_heap_offset - wpr_meta_size;
    let non_wpr_heap_size: u64 = 0x100000;
    let non_wpr_heap_offset = gsp_fw_wpr_start - non_wpr_heap_size;
    let gsp_fw_rsvd_start = non_wpr_heap_offset;

    let meta = unsafe {
        let ptr = WPR_META_BASE as *mut GspFwWprMeta;
        core::ptr::write_bytes(ptr as *mut u8, 0, core::mem::size_of::<GspFwWprMeta>());
        &mut *ptr
    };
    
    meta.magic = GSP_FW_WPR_META_MAGIC;
    meta.revision = GSP_FW_WPR_META_REVISION;
    
    // SYSMEM pointers (where booter_load DMA's from)
    meta.sysmem_addr_of_radix3_elf = RADIX3_BASE;
    meta.size_of_radix3_elf = GSP_FW_SIZE as u64;
    meta.sysmem_addr_of_bootloader = BOOT_BIN_BASE;
    meta.size_of_bootloader = BOOT_BIN_SIZE as u64;
    
    // Boot binary offsets (from DESC_PROD extraction)
    meta.bootloader_code_offset = 0x00000000;   // monitorCodeOffset
    meta.bootloader_data_offset = 0x00000000;   // monitorDataOffset  
    meta.bootloader_manifest_offset = 0x00000005; // manifestOffset

    // FB layout
    meta.fb_size = fb_size;
    meta.vga_workspace_offset = vga_workspace_offset;
    meta.vga_workspace_size = vga_workspace_size;
    meta.gsp_fw_wpr_end = gsp_fw_wpr_end;
    meta.frts_offset = frts_offset;
    meta.frts_size = frts_size;
    meta.boot_bin_offset = boot_bin_offset;
    meta.gsp_fw_offset = gsp_fw_offset;
    meta.gsp_fw_heap_offset = gsp_fw_heap_offset;
    meta.gsp_fw_heap_size = gsp_fw_heap_size;
    meta.gsp_fw_wpr_start = gsp_fw_wpr_start;
    meta.non_wpr_heap_offset = non_wpr_heap_offset;
    meta.non_wpr_heap_size = non_wpr_heap_size;
    meta.gsp_fw_rsvd_start = gsp_fw_rsvd_start;
    meta.boot_count = 0;
    meta.verified = 0;

    con.print("  WPR meta @0x"); con.print_hex32(WPR_META_BASE as u32);
    con.print(" FB="); con.print_u64(fb_size / 1024 / 1024); con.println("MB");
    con.print("  BootBin @0x"); con.print_hex32(BOOT_BIN_BASE as u32);
    con.print(" size="); con.print_u64(BOOT_BIN_SIZE as u64); con.println("B");
    con.print("  WPR: 0x"); con.print_hex32(gsp_fw_wpr_start as u32);
    con.print("-0x"); con.print_hex32(gsp_fw_wpr_end as u32); con.println("");

    Some(GspPrepared {
        wpr_meta_phys: WPR_META_BASE,
        gsp_fw_loaded: true,
    })
}

