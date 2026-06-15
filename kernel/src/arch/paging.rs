#![allow(dead_code)]

//! Page table management for x86-64.

pub mod flags {
    pub const PRESENT: u64    = 1 << 0;
    pub const WRITABLE: u64   = 1 << 1;
    pub const USER: u64       = 1 << 2;
    pub const HUGE_PAGE: u64  = 1 << 7;
    pub const GLOBAL: u64     = 1 << 8;
    pub const NO_EXECUTE: u64 = 1 << 63;
}

#[derive(Clone, Copy)]
#[repr(transparent)]
pub struct PageTableEntry(u64);

impl PageTableEntry {
    pub const fn empty() -> Self { Self(0) }

    pub const fn new(phys: u64, f: u64) -> Self {
        Self((phys & 0x000F_FFFF_FFFF_F000) | f)
    }

    pub fn is_present(&self) -> bool { self.0 & flags::PRESENT != 0 }
    pub fn phys_addr(&self) -> u64 { self.0 & 0x000F_FFFF_FFFF_F000 }
}

#[repr(align(4096))]
pub struct PageTable {
    pub entries: [PageTableEntry; 512],
}

impl PageTable {
    pub const fn empty() -> Self {
        Self { entries: [PageTableEntry::empty(); 512] }
    }
}

#[inline]
pub fn read_cr3() -> u64 {
    let v: u64;
    unsafe { core::arch::asm!("mov {}, cr3", out(reg) v); }
    v
}

#[inline]
pub unsafe fn write_cr3(pml4: u64) {
    core::arch::asm!("mov cr3, {}", in(reg) pml4);
}

#[inline]
pub fn invlpg(addr: u64) {
    unsafe { core::arch::asm!("invlpg [{}]", in(reg) addr); }
}

#[inline]
pub fn flush_tlb() {
    unsafe { write_cr3(read_cr3()); }
}

const ADDR_MASK: u64 = 0x000F_FFFF_FFFF_F000;
const PAGE_SIZE: u64 = 4096;

/// Marca una región identity-mapped del CR3 actual como accesible desde Ring 3.
///
/// FastOS todavía hereda las tablas iniciales del boot path. Para que `sysretq`
/// pueda ejecutar un payload real de usuario, no basta con tener GDT/TSS/MSR:
/// las entradas PML4/PDPT/PD/PT que cubren código y stack deben tener el bit
/// `USER`. Si no, la CPU genera #PF al primer fetch/acceso y parece un freeze.
///
/// Esta función no crea mapeos nuevos; sólo endurece/abre una región que ya
/// debe estar presente por identidad física = virtual.
pub unsafe fn mark_current_identity_user_range(start: u64, len: usize) -> Result<(), &'static str> {
    if len == 0 {
        return Ok(());
    }
    let end = start.checked_add(len as u64).ok_or("user range overflow")?;
    let mut va = start & !(PAGE_SIZE - 1);
    let end = (end + PAGE_SIZE - 1) & !(PAGE_SIZE - 1);
    let pml4 = (read_cr3() & ADDR_MASK) as *mut PageTable;

    while va < end {
        let pml4_i = ((va >> 39) & 0x1FF) as usize;
        let pdpt_i = ((va >> 30) & 0x1FF) as usize;
        let pd_i = ((va >> 21) & 0x1FF) as usize;
        let pt_i = ((va >> 12) & 0x1FF) as usize;

        let pml4e = &mut (*pml4).entries[pml4_i];
        if !pml4e.is_present() { return Err("PML4 entry not present"); }
        pml4e.0 |= flags::USER | flags::WRITABLE;

        let pdpt = pml4e.phys_addr() as *mut PageTable;
        let pdpte = &mut (*pdpt).entries[pdpt_i];
        if !pdpte.is_present() { return Err("PDPT entry not present"); }
        pdpte.0 |= flags::USER | flags::WRITABLE;

        // 1 GiB huge page.
        if (pdpte.0 & flags::HUGE_PAGE) != 0 {
            pdpte.0 &= !flags::NO_EXECUTE;
            invlpg(va);
            va += PAGE_SIZE;
            continue;
        }

        let pd = pdpte.phys_addr() as *mut PageTable;
        let pde = &mut (*pd).entries[pd_i];
        if !pde.is_present() { return Err("PD entry not present"); }
        pde.0 |= flags::USER | flags::WRITABLE;

        // 2 MiB huge page.
        if (pde.0 & flags::HUGE_PAGE) != 0 {
            pde.0 &= !flags::NO_EXECUTE;
            invlpg(va);
            va += PAGE_SIZE;
            continue;
        }

        let pt = pde.phys_addr() as *mut PageTable;
        let pte = &mut (*pt).entries[pt_i];
        if !pte.is_present() { return Err("PT entry not present"); }
        pte.0 |= flags::USER | flags::WRITABLE;
        pte.0 &= !flags::NO_EXECUTE;
        invlpg(va);

        va += PAGE_SIZE;
    }

    Ok(())
}

/// Allocate a new page table page (4 KB, aligned).
/// Returns physical address of the new page table, or None if OOM.
unsafe fn alloc_page_table() -> Option<u64> {
    crate::arch::page_alloc::alloc_pages_contiguous(1)
}

/// Clone kernel PML4 into a new user PML4, sharing kernel mappings (above 0xFFFF_8000_0000_0000).
/// Returns physical address of new PML4.
pub unsafe fn create_user_page_table(kernel_cr3: u64) -> Option<u64> {
    let user_pml4_phys = alloc_page_table()?;
    let user_pml4 = user_pml4_phys as *mut PageTable;
    let kernel_pml4 = (kernel_cr3 & ADDR_MASK) as *const PageTable;

    // Zero the new PML4
    core::ptr::write_bytes(user_pml4 as *mut u8, 0, PAGE_SIZE as usize);

    // Copy kernel mappings (entries 256..511 cover high half: 0xFFFF_8000_0000_0000..)
    // Entry 256 = 512 GB region starting at 0xFFFF_8000_0000_0000
    for i in 256..512 {
        (*user_pml4).entries[i] = (*kernel_pml4).entries[i];
    }

    // Entry 511: recursive mapping for page table self-reference (if used by kernel)
    // This allows kernel to access its own page tables via virtual addresses.
    (*user_pml4).entries[511] = (*kernel_pml4).entries[511];

    Some(user_pml4_phys)
}

/// Map a user virtual address range to physical pages in the given PML4.
/// Flags should include USER bit; NX for stack, !NX for code.
/// Assumes physical pages are already allocated and identity-mapped (phys == virt for low 4 GB).
pub unsafe fn map_user_range(
    pml4_phys: u64,
    virt_start: u64,
    phys_start: u64,
    pages: usize,
    flags: u64,
) -> Result<(), &'static str> {
    if pages == 0 {
        return Ok(());
    }
    let pml4 = pml4_phys as *mut PageTable;
    let mut va = virt_start;
    let mut pa = phys_start;

    for _ in 0..pages {
        let pml4_i = ((va >> 39) & 0x1FF) as usize;
        let pdpt_i = ((va >> 30) & 0x1FF) as usize;
        let pd_i = ((va >> 21) & 0x1FF) as usize;
        let pt_i = ((va >> 12) & 0x1FF) as usize;

        // PML4 entry
        let pml4e = &mut (*pml4).entries[pml4_i];
        let pdpt_phys: u64;
        if !pml4e.is_present() {
            pdpt_phys = alloc_page_table().ok_or("OOM allocating PDPT")?;
            core::ptr::write_bytes(pdpt_phys as *mut u8, 0, PAGE_SIZE as usize);
            pml4e.0 = PageTableEntry::new(pdpt_phys, flags::PRESENT | flags::WRITABLE | flags::USER).0;
        } else {
            pdpt_phys = pml4e.phys_addr();
            pml4e.0 |= flags::USER | flags::WRITABLE;
        }

        // PDPT entry
        let pdpt = pdpt_phys as *mut PageTable;
        let pdpte = &mut (*pdpt).entries[pdpt_i];
        let pd_phys: u64;
        if !pdpte.is_present() {
            pd_phys = alloc_page_table().ok_or("OOM allocating PD")?;
            core::ptr::write_bytes(pd_phys as *mut u8, 0, PAGE_SIZE as usize);
            pdpte.0 = PageTableEntry::new(pd_phys, flags::PRESENT | flags::WRITABLE | flags::USER).0;
        } else {
            pd_phys = pdpte.phys_addr();
            pdpte.0 |= flags::USER | flags::WRITABLE;
        }

        // 1 GiB huge page? (not for user mappings)
        if (pdpte.0 & flags::HUGE_PAGE) != 0 {
            return Err("Unexpected huge page in user mapping");
        }

        // PD entry
        let pd = pd_phys as *mut PageTable;
        let pde = &mut (*pd).entries[pd_i];
        let pt_phys: u64;
        if !pde.is_present() {
            pt_phys = alloc_page_table().ok_or("OOM allocating PT")?;
            core::ptr::write_bytes(pt_phys as *mut u8, 0, PAGE_SIZE as usize);
            pde.0 = PageTableEntry::new(pt_phys, flags::PRESENT | flags::WRITABLE | flags::USER).0;
        } else {
            pt_phys = pde.phys_addr();
            pde.0 |= flags::USER | flags::WRITABLE;
        }

        // 2 MiB huge page? (not for user mappings)
        if (pde.0 & flags::HUGE_PAGE) != 0 {
            return Err("Unexpected huge page in user mapping");
        }

        // PT entry
        let pt = pt_phys as *mut PageTable;
        let pte = &mut (*pt).entries[pt_i];
        if pte.is_present() {
            return Err("Page already mapped");
        }
        pte.0 = PageTableEntry::new(pa, flags | flags::PRESENT).0;
        invlpg(va);

        va += PAGE_SIZE;
        pa += PAGE_SIZE;
    }

    Ok(())
}

/// Free all user page tables (PDPTs, PDs, PTs) for a process.
/// Does not free the PML4 itself (caller should free via page_alloc).
pub unsafe fn free_user_page_tables(pml4_phys: u64) {
    let pml4 = pml4_phys as *mut PageTable;

    // Only free user half (entries 0..255)
    for pml4_i in 0..256 {
        let pml4e = &mut (*pml4).entries[pml4_i];
        if !pml4e.is_present() { continue; }

        let pdpt = pml4e.phys_addr() as *mut PageTable;
        for pdpt_i in 0..512 {
            let pdpte = &mut (*pdpt).entries[pdpt_i];
            if !pdpte.is_present() { continue; }

            let pd = pdpte.phys_addr() as *mut PageTable;
            for pd_i in 0..512 {
                let pde = &mut (*pd).entries[pd_i];
                if !pde.is_present() { continue; }

                // 2 MiB huge page
                if (pde.0 & flags::HUGE_PAGE) != 0 {
                    crate::arch::page_alloc::free_pages(pde.phys_addr(), 512);
                    pde.0 = 0;
                    continue;
                }

                let pt = pde.phys_addr() as *mut PageTable;
                for pt_i in 0..512 {
                    let pte = &mut (*pt).entries[pt_i];
                    if pte.is_present() {
                        crate::arch::page_alloc::free_pages(pte.phys_addr(), 1);
                        pte.0 = 0;
                    }
                }
                crate::arch::page_alloc::free_pages(pde.phys_addr(), 1);
                pde.0 = 0;
            }
            crate::arch::page_alloc::free_pages(pdpte.phys_addr(), 1);
            pdpte.0 = 0;
        }
        crate::arch::page_alloc::free_pages(pml4e.phys_addr(), 1);
        pml4e.0 = 0;
    }
}
