//! Radix3 Page Table Builder — GSP Firmware VRAM Loading
//!
//! NVIDIA GSP firmware (Ampere+) uses a 3-level radix page table to map
//! the GSP-RM ELF from system RAM into the GPU's virtual address space.
//!
//! Fuente: nouveau/nvkm/subdev/gsp/tu102.c — nvkm_gsp_radix3_sg()
//!
//! Structure (3 levels, 4KB pages):
//!   Level 0 (root): 1 page — 512 entries pointing to Level 1 pages
//!   Level 1:        N pages — 512 entries each pointing to Level 2 pages
//!   Level 2:        M pages — 512 entries each pointing to firmware data pages
//!
//! Each entry is a u64: [PA >> 12 | flags]
//!   bit 0 = valid
//!   bit 1 = volatile (for sysmem)
//!
//! The root page's physical address goes into GspFwWprMeta.sysmem_addr_of_radix3_elf

use crate::console::Console;

const PAGE_SIZE: usize = 4096;
const ENTRIES_PER_PAGE: usize = PAGE_SIZE / 8; // 512 u64 entries per 4KB page

// Entry flags
const RADIX3_VALID: u64    = 1 << 0;
const RADIX3_VOLATILE: u64 = 1 << 1; // Set for sysmem pages

/// Radix3 page table for mapping firmware data to GPU address space.
///
/// Layout in memory (contiguous allocation):
///   [Level 0: 1 page] [Level 1: l1_pages pages] [Level 2: l2_pages pages]
pub struct Radix3PageTable {
    /// Physical base of the entire radix3 allocation
    pub base_phys: u64,
    /// Total pages allocated
    pub total_pages: usize,
    /// Size of firmware data being mapped
    pub fw_size: usize,
}

impl Radix3PageTable {
    /// Build a 3-level radix page table that maps `fw_data` (in sysmem)
    /// so the GPU can read it.
    ///
    /// # Arguments
    /// * `fw_phys` - Physical address of the firmware ELF data
    /// * `fw_size` - Size of firmware data in bytes
    /// * `con` - Console for diagnostic output
    ///
    /// # Returns
    /// The Radix3PageTable with base_phys pointing to the Level 0 root page.
    pub fn build(fw_phys: u64, fw_size: usize, con: &mut Console) -> Option<Self> {
        // Calculate number of data pages
        let data_pages = (fw_size + PAGE_SIZE - 1) / PAGE_SIZE;

        // Level 2: each page maps 512 data pages
        let l2_pages = (data_pages + ENTRIES_PER_PAGE - 1) / ENTRIES_PER_PAGE;

        // Level 1: each page maps 512 Level 2 pages
        let l1_pages = (l2_pages + ENTRIES_PER_PAGE - 1) / ENTRIES_PER_PAGE;

        // Level 0: always 1 page (maps up to 512 Level 1 pages = 512^3 * 4KB = 512GB)
        let total_pages = 1 + l1_pages + l2_pages;

        con.print("  [RADIX3] FW size=0x");
        con.print_hex32(fw_size as u32);
        con.print(" data_pages=");
        con.print_hex32(data_pages as u32);
        con.print(" L2=");
        con.print_hex32(l2_pages as u32);
        con.print(" L1=");
        con.print_hex32(l1_pages as u32);
        con.print(" total=");
        con.print_hex32(total_pages as u32);
        con.newline();

        // Allocate contiguous pages for the entire radix tree
        let base_phys = unsafe {
            crate::arch::page_alloc::alloc_pages_contiguous(total_pages)?
        };

        // Zero the entire allocation
        unsafe {
            core::ptr::write_bytes(base_phys as *mut u8, 0, total_pages * PAGE_SIZE);
        }

        // Physical addresses of each level
        let l0_phys = base_phys;
        let l1_phys = base_phys + PAGE_SIZE as u64;
        let l2_phys = l1_phys + (l1_pages as u64 * PAGE_SIZE as u64);

        // Fill Level 0 → points to Level 1 pages
        let l0_ptr = l0_phys as *mut u64;
        for i in 0..l1_pages {
            let l1_page_phys = l1_phys + (i as u64 * PAGE_SIZE as u64);
            let entry = (l1_page_phys & !0xFFF) | RADIX3_VALID;
            unsafe { core::ptr::write_volatile(l0_ptr.add(i), entry); }
        }

        // Fill Level 1 → points to Level 2 pages
        for i in 0..l2_pages {
            let l1_page_idx = i / ENTRIES_PER_PAGE;
            let l1_entry_idx = i % ENTRIES_PER_PAGE;
            let l1_page_ptr = (l1_phys + (l1_page_idx as u64 * PAGE_SIZE as u64)) as *mut u64;

            let l2_page_phys = l2_phys + (i as u64 * PAGE_SIZE as u64);
            let entry = (l2_page_phys & !0xFFF) | RADIX3_VALID;
            unsafe { core::ptr::write_volatile(l1_page_ptr.add(l1_entry_idx), entry); }
        }

        // Fill Level 2 → points to firmware data pages (in sysmem)
        for i in 0..data_pages {
            let l2_page_idx = i / ENTRIES_PER_PAGE;
            let l2_entry_idx = i % ENTRIES_PER_PAGE;
            let l2_page_ptr = (l2_phys + (l2_page_idx as u64 * PAGE_SIZE as u64)) as *mut u64;

            let data_page_phys = fw_phys + (i as u64 * PAGE_SIZE as u64);
            let entry = (data_page_phys & !0xFFF) | RADIX3_VALID | RADIX3_VOLATILE;
            unsafe { core::ptr::write_volatile(l2_page_ptr.add(l2_entry_idx), entry); }
        }

        con.print("  [RADIX3] Root=0x");
        con.print_hex32((l0_phys >> 32) as u32);
        con.print_hex32(l0_phys as u32);
        con.print(" → L1=0x");
        con.print_hex32(l1_phys as u32);
        con.print(" → L2=0x");
        con.print_hex32(l2_phys as u32);
        con.newline();

        // Verify first entry of each level
        let l0_e0 = unsafe { core::ptr::read_volatile(l0_phys as *const u64) };
        let l1_e0 = unsafe { core::ptr::read_volatile(l1_phys as *const u64) };
        let l2_e0 = unsafe { core::ptr::read_volatile(l2_phys as *const u64) };
        con.print("  [RADIX3] L0[0]=0x");
        con.print_hex32(l0_e0 as u32);
        con.print(" L1[0]=0x");
        con.print_hex32(l1_e0 as u32);
        con.print(" L2[0]=0x");
        con.print_hex32(l2_e0 as u32);
        con.newline();

        Some(Self {
            base_phys,
            total_pages,
            fw_size,
        })
    }

    /// Get the physical address of the Level 0 root page.
    /// This goes into GspFwWprMeta.sysmem_addr_of_radix3_elf.
    pub fn root_phys(&self) -> u64 {
        self.base_phys
    }

    /// Free the radix3 page table allocation.
    pub unsafe fn free(self) {
        crate::arch::page_alloc::free_pages(self.base_phys, self.total_pages);
    }
}
