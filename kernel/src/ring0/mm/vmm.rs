//! Virtual Memory Manager — x86-64 4-level page tables, demand paging, CoW.
//!
//! Page table pages are allocated from the physical frame allocator
//! (identity-mapped in low 4 GB). All page table walks use the shared
//! `super::PAGE_SIZE` constant.
//!
//! # High-Mem Architecture
//!
//! All physical RAM is mapped into a contiguous virtual region starting at
//! `HIGH_MEM_BASE`. This allows the kernel to access ANY physical page
//! via `phys_to_virt(phys_addr)`, regardless of the identity-mapping limit.
//!
//!   virt_to_phys(v) = v - HIGH_MEM_BASE
//!   phys_to_virt(p) = p + HIGH_MEM_BASE
//!
//! This scales to 1 TB of RAM:
//!   1 TB = 256M pages × 4 KB bitmap = 32 MB bitmap overhead.
//!   Virtual range: 0xFFFF_8000_0000_0000 → 0xFFFF_8001_0000_0000 (1 TB)

/// Base virtual address for the high-mem region.
/// All physical RAM is identity-mapped here: virt = phys + HIGH_MEM_BASE.
/// Chosen to be in the upper half (kernel space) and aligned for 1 GB pages.
pub const HIGH_MEM_BASE: u64 = 0xFFFF_8000_0000_0000;

/// Maximum supported physical address (1 TB).
pub const MAX_PHYS_ADDR: u64 = 1 << 40; // 1 TB

/// Convert a physical address to a kernel virtual address.
#[inline]
pub const fn phys_to_virt(phys: u64) -> u64 {
    phys + HIGH_MEM_BASE
}

/// Convert a kernel virtual address to a physical address.
#[inline]
pub const fn virt_to_phys(virt: u64) -> u64 {
    virt - HIGH_MEM_BASE
}

pub mod flags {
    pub const PRESENT: u64    = 1 << 0;
    pub const WRITABLE: u64   = 1 << 1;
    pub const USER: u64       = 1 << 2;
    pub const HUGE_PAGE: u64  = 1 << 7;
    pub const GLOBAL: u64     = 1 << 8;
    pub const NO_EXECUTE: u64 = 1 << 63;
}

pub const DEMAND: u64 = 1 << 9;
pub const COW: u64 = 1 << 10;

const ADDR_MASK: u64 = 0x000F_FFFF_FFFF_F000;
const PAGE_SIZE: u64 = super::PAGE_SIZE;

#[derive(Clone, Copy)]
#[repr(transparent)]
pub struct PageTableEntry(u64);

impl PageTableEntry {
    pub const fn empty() -> Self { Self(0) }

    pub const fn new(phys: u64, f: u64) -> Self {
        debug_assert!(phys <= 0x000F_FFFF_FFFF_FFFF, "PageTableEntry: phys addr too large");
        Self((phys & ADDR_MASK) | f)
    }

    pub fn is_present(&self) -> bool { self.0 & flags::PRESENT != 0 }
    pub fn phys_addr(&self) -> u64 { self.0 & ADDR_MASK }
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

pub unsafe fn create_kernel_page_table() -> Option<u64> {
    crate::dev::console::serial_write("[vmm] create_kernel_page_table: STUBBED (use UEFI PML4)\n");
    None
}

/// Allocate a 4 KiB-aligned page table page from the physical frame allocator.
/// Returns the physical address (or None on OOM).
///
/// SAFETY: Page table pages are always allocated from low memory (< 4 GB)
/// because the frame allocator starts at BASE (16 MB) and page tables are
/// small. Low memory is identity-mapped by UEFI, so `phys as *mut u8` is
/// a valid virtual pointer. After map_high_mem() completes, `phys_to_virt`
/// can be used instead.
pub unsafe fn alloc_page_table() -> Option<u64> {
    let phys = crate::mm::phys::alloc_pages_contiguous(1)?;
    // Identity-map safe: page table pages are always in low memory (< 4 GB)
    // during early boot, before high-mem mapping exists.
    core::ptr::write_bytes(phys as *mut u8, 0, PAGE_SIZE as usize);
    Some(phys)
}

/// Convert a physical page table address to a virtual PageTable pointer.
/// Used throughout the VMM to access page table entries.
/// SAFE ONLY AFTER map_high_mem() has completed.
#[inline]
unsafe fn phys_to_pt(phys: u64) -> *mut PageTable {
    phys_to_virt(phys) as *mut PageTable
}

/// Convert a physical address to an identity-mapped pointer.
/// Used during early boot (before map_high_mem) when page table pages
/// are in low memory (< 4 GB) and identity-mapped by UEFI.
#[inline]
unsafe fn phys_to_id(phys: u64) -> *mut PageTable {
    phys as *mut PageTable
}

pub unsafe fn create_user_page_table(kernel_cr3: u64) -> Option<u64> {
    let user_pml4_phys = alloc_page_table()?;
    let user_pml4 = phys_to_pt(user_pml4_phys);
    let kernel_pml4 = phys_to_pt(kernel_cr3 & ADDR_MASK) as *const PageTable;

    core::ptr::write_bytes(user_pml4 as *mut u8, 0, PAGE_SIZE as usize);

    for i in 256..512 {
        (*user_pml4).entries[i] = (*kernel_pml4).entries[i];
    }
    (*user_pml4).entries[511] = (*kernel_pml4).entries[511];

    Some(user_pml4_phys)
}

pub unsafe fn map_kernel_mmio_huge(
    phys_start: u64,
    virt_start: u64,
    bytes: usize,
) -> Result<(), &'static str> {
    if bytes == 0 { return Ok(()); }
    const HUGE_2MB: u64 = 2 * 1024 * 1024;
    let pages = (bytes + (HUGE_2MB as usize) - 1) / (HUGE_2MB as usize);
    if (virt_start & (HUGE_2MB - 1)) != 0 || (phys_start & (HUGE_2MB - 1)) != 0 {
        return Err("map_kernel_mmio_huge: addresses must be 2 MiB aligned");
    }

    let pml4 = phys_to_pt(read_cr3() & ADDR_MASK);

    for i in 0..pages {
        let va = virt_start + (i as u64) * HUGE_2MB;
        let pa = phys_start + (i as u64) * HUGE_2MB;

        let pml4_i = ((va >> 39) & 0x1FF) as usize;
        let pdpt_i = ((va >> 30) & 0x1FF) as usize;
        let pd_i   = ((va >> 21) & 0x1FF) as usize;

        let pml4e = &mut (*pml4).entries[pml4_i];
        let pdpt_phys: u64 = if !pml4e.is_present() {
            let new = alloc_page_table().ok_or("OOM: PML4->PDPT")?;
            pml4e.0 = PageTableEntry::new(new, flags::PRESENT | flags::WRITABLE).0;
            new
        } else {
            pml4e.0 |= flags::WRITABLE;
            pml4e.0 &= !flags::NO_EXECUTE;
            pml4e.phys_addr()
        };

        let pdpt = phys_to_pt(pdpt_phys);
        let pdpte = &mut (*pdpt).entries[pdpt_i];
        let pd_phys: u64 = if !pdpte.is_present() {
            let new = alloc_page_table().ok_or("OOM: PDPT->PD")?;
            pdpte.0 = PageTableEntry::new(new, flags::PRESENT | flags::WRITABLE).0;
            new
        } else if (pdpte.0 & flags::HUGE_PAGE) != 0 {
            return Err("map_kernel_mmio_huge: PDPT entry is 1 GiB huge page, cannot sub-allocate 2 MiB");
        } else {
            pdpte.0 |= flags::WRITABLE;
            pdpte.0 &= !flags::NO_EXECUTE;
            pdpte.phys_addr()
        };

        let pd = phys_to_pt(pd_phys);
        let pde = &mut (*pd).entries[pd_i];
        if pde.is_present() {
            return Err("map_kernel_mmio_huge: page already mapped");
        }
        pde.0 = PageTableEntry::new(
            pa,
            flags::PRESENT | flags::WRITABLE | flags::HUGE_PAGE | flags::NO_EXECUTE,
        ).0;
        invlpg(va);
    }
    Ok(())
}

/// Map a user virtual address range to physical pages in the given PML4.
/// On OOM, any partially-allocated page tables are leaked (acceptable for
/// process teardown paths that will free the entire page table tree).
pub unsafe fn map_user_range(
    pml4_phys: u64,
    virt_start: u64,
    phys_start: u64,
    pages: usize,
    flags: u64,
) -> Result<(), &'static str> {
    if pages == 0 { return Ok(()); }
    let pml4 = phys_to_pt(pml4_phys);
    let mut va = virt_start;
    let mut pa = phys_start;

    for _ in 0..pages {
        let pml4_i = ((va >> 39) & 0x1FF) as usize;
        let pdpt_i = ((va >> 30) & 0x1FF) as usize;
        let pd_i = ((va >> 21) & 0x1FF) as usize;
        let pt_i = ((va >> 12) & 0x1FF) as usize;

        let pml4e = &mut (*pml4).entries[pml4_i];
        let pdpt_phys: u64;
        if !pml4e.is_present() {
            pdpt_phys = alloc_page_table().ok_or("OOM allocating PDPT")?;
            pml4e.0 = PageTableEntry::new(pdpt_phys, flags::PRESENT | flags::WRITABLE | flags::USER).0;
        } else {
            pdpt_phys = pml4e.phys_addr();
            pml4e.0 |= flags::USER | flags::WRITABLE;
        }

        let pdpt = phys_to_pt(pdpt_phys);
        let pdpte = &mut (*pdpt).entries[pdpt_i];
        let pd_phys: u64;
        if !pdpte.is_present() {
            pd_phys = alloc_page_table().ok_or("OOM allocating PD")?;
            pdpte.0 = PageTableEntry::new(pd_phys, flags::PRESENT | flags::WRITABLE | flags::USER).0;
        } else {
            pd_phys = pdpte.phys_addr();
            pdpte.0 |= flags::USER | flags::WRITABLE;
        }

        if (pdpte.0 & flags::HUGE_PAGE) != 0 {
            return Err("Unexpected huge page in user mapping");
        }

        let pd = phys_to_pt(pd_phys);
        let pde = &mut (*pd).entries[pd_i];
        let pt_phys: u64;
        if !pde.is_present() {
            pt_phys = alloc_page_table().ok_or("OOM allocating PT")?;
            pde.0 = PageTableEntry::new(pt_phys, flags::PRESENT | flags::WRITABLE | flags::USER).0;
        } else {
            pt_phys = pde.phys_addr();
            pde.0 |= flags::USER | flags::WRITABLE;
        }

        if (pde.0 & flags::HUGE_PAGE) != 0 {
            return Err("Unexpected huge page in user mapping");
        }

        let pt = phys_to_pt(pt_phys);
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

/// Free all user page tables for a process.
/// Uses the frame allocator (same allocator as alloc_page_table).
pub unsafe fn free_user_page_tables(pml4_phys: u64) {
    let pml4 = phys_to_pt(pml4_phys);

    for pml4_i in 0..256 {
        let pml4e = &mut (*pml4).entries[pml4_i];
        if !pml4e.is_present() { continue; }

        let pdpt = phys_to_pt(pml4e.phys_addr());
        for pdpt_i in 0..512 {
            let pdpte = &mut (*pdpt).entries[pdpt_i];
            if !pdpte.is_present() { continue; }

            let pd = phys_to_pt(pdpte.phys_addr());
            for pd_i in 0..512 {
                let pde = &mut (*pd).entries[pd_i];
                if !pde.is_present() { continue; }

                if (pde.0 & flags::HUGE_PAGE) != 0 {
                    crate::mm::phys::free_pages(pde.phys_addr(), 512);
                    pde.0 = 0;
                    continue;
                }

                let pt = phys_to_pt(pde.phys_addr());
                for pt_i in 0..512 {
                    let pte = &mut (*pt).entries[pt_i];
                    if pte.is_present() {
                        crate::mm::phys::free_pages(pte.phys_addr(), 1);
                        pte.0 = 0;
                    }
                }
                crate::mm::phys::free_pages(pde.phys_addr(), 1);
                pde.0 = 0;
            }
            crate::mm::phys::free_pages(pdpte.phys_addr(), 1);
            pdpte.0 = 0;
        }
        crate::mm::phys::free_pages(pml4e.phys_addr(), 1);
        pml4e.0 = 0;
    }
}

#[derive(Clone, Copy, Debug)]
pub struct Vma {
    pub virt_start: u64,
    pub virt_end: u64,
    pub flags: u64,
    pub kind: VmaKind,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum VmaKind {
    Fixed,
    Demand,
    Cow(u64),
}

impl Vma {
    pub const fn empty() -> Self {
        Self { virt_start: 0, virt_end: 0, flags: 0, kind: VmaKind::Fixed }
    }

    pub fn contains(&self, addr: u64) -> bool {
        addr >= self.virt_start && addr < self.virt_end
    }
}

#[derive(Debug)]
pub struct AddressSpace {
    pub pml4_phys: u64,
    pub vmas: [Vma; 32],
    pub vma_count: usize,
}

impl AddressSpace {
    pub const fn empty() -> Self {
        Self {
            pml4_phys: 0,
            vmas: [Vma::empty(); 32],
            vma_count: 0,
        }
    }

    pub fn add_vma(&mut self, vma: Vma) -> bool {
        if self.vma_count >= self.vmas.len() { return false; }
        self.vmas[self.vma_count] = vma;
        self.vma_count += 1;
        true
    }

    pub fn find_vma(&self, addr: u64) -> Option<&Vma> {
        for i in 0..self.vma_count {
            if self.vmas[i].contains(addr) {
                return Some(&self.vmas[i]);
            }
        }
        None
    }

    pub fn find_vma_mut(&mut self, addr: u64) -> Option<&mut Vma> {
        for i in 0..self.vma_count {
            if self.vmas[i].contains(addr) {
                return Some(&mut self.vmas[i]);
            }
        }
        None
    }
}

pub unsafe fn map_user_demand(
    pml4_phys: u64,
    virt_start: u64,
    pages: usize,
    flags: u64,
) -> Result<(), &'static str> {
    if pages == 0 { return Ok(()); }
    let pml4 = phys_to_pt(pml4_phys);
    let mut va = virt_start;

    for _ in 0..pages {
        let pml4_i = ((va >> 39) & 0x1FF) as usize;
        let pdpt_i = ((va >> 30) & 0x1FF) as usize;
        let pd_i = ((va >> 21) & 0x1FF) as usize;
        let pt_i = ((va >> 12) & 0x1FF) as usize;

        let pml4e = &mut (*pml4).entries[pml4_i];
        let pdpt_phys: u64;
        if !pml4e.is_present() {
            pdpt_phys = alloc_page_table().ok_or("OOM allocating PDPT")?;
            pml4e.0 = PageTableEntry::new(pdpt_phys, flags::PRESENT | flags::WRITABLE | flags::USER).0;
        } else {
            pdpt_phys = pml4e.phys_addr();
        }

        let pdpt = phys_to_pt(pdpt_phys);
        let pdpte = &mut (*pdpt).entries[pdpt_i];
        let pd_phys: u64;
        if !pdpte.is_present() {
            pd_phys = alloc_page_table().ok_or("OOM allocating PD")?;
            pdpte.0 = PageTableEntry::new(pd_phys, flags::PRESENT | flags::WRITABLE | flags::USER).0;
        } else {
            pd_phys = pdpte.phys_addr();
        }

        let pd = phys_to_pt(pd_phys);
        let pde = &mut (*pd).entries[pd_i];
        let pt_phys: u64;
        if !pde.is_present() {
            pt_phys = alloc_page_table().ok_or("OOM allocating PT")?;
            pde.0 = PageTableEntry::new(pt_phys, flags::PRESENT | flags::WRITABLE | flags::USER).0;
        } else {
            pt_phys = pde.phys_addr();
        }

        let pt = phys_to_pt(pt_phys);
        let pte = &mut (*pt).entries[pt_i];
        if pte.is_present() {
            return Err("Page already mapped");
        }
        pte.0 = (flags | DEMAND) & !flags::PRESENT;

        va += PAGE_SIZE;
    }
    Ok(())
}

pub unsafe fn resolve_demand_page(
    fault_addr: u64,
    pml4_phys: u64,
    vma: &Vma,
) -> Result<bool, &'static str> {
    let page_addr = fault_addr & !(PAGE_SIZE - 1);
    let pml4 = phys_to_pt(pml4_phys);

    let pml4_i = ((page_addr >> 39) & 0x1FF) as usize;
    let pdpt_i = ((page_addr >> 30) & 0x1FF) as usize;
    let pd_i = ((page_addr >> 21) & 0x1FF) as usize;
    let pt_i = ((page_addr >> 12) & 0x1FF) as usize;

    let pml4e = &mut (*pml4).entries[pml4_i];
    if !pml4e.is_present() { return Ok(false); }

    let pdpt = phys_to_pt(pml4e.phys_addr());
    let pdpte = &mut (*pdpt).entries[pdpt_i];
    if !pdpte.is_present() { return Ok(false); }

    let pd = phys_to_pt(pdpte.phys_addr());
    let pde = &mut (*pd).entries[pd_i];
    if !pde.is_present() { return Ok(false); }

    let pt = phys_to_pt(pde.phys_addr());
    let pte = &mut (*pt).entries[pt_i];

    if pte.0 & DEMAND == 0 { return Ok(false); }

    let phys = crate::mm::phys::alloc_pages_contiguous(1)
        .ok_or("OOM resolving demand page")?;
    // alloc_page_table zeroes, but alloc_pages_contiguous doesn't — zero manually
    core::ptr::write_bytes(phys_to_virt(phys) as *mut u8, 0, PAGE_SIZE as usize);

    let final_flags = (vma.flags | flags::PRESENT) & !DEMAND;
    pte.0 = PageTableEntry::new(phys, final_flags).0;
    invlpg(page_addr);

    crate::cabina::trace_u64("vm", "demand page resolved", page_addr);
    Ok(true)
}

pub unsafe fn resolve_cow_page(
    fault_addr: u64,
    pml4_phys: u64,
    vma: &Vma,
) -> Result<bool, &'static str> {
    let page_addr = fault_addr & !(PAGE_SIZE - 1);
    let pml4 = phys_to_pt(pml4_phys);

    let pml4_i = ((page_addr >> 39) & 0x1FF) as usize;
    let pdpt_i = ((page_addr >> 30) & 0x1FF) as usize;
    let pd_i = ((page_addr >> 21) & 0x1FF) as usize;
    let pt_i = ((page_addr >> 12) & 0x1FF) as usize;

    let pml4e = &mut (*pml4).entries[pml4_i];
    if !pml4e.is_present() { return Ok(false); }

    let pdpt = phys_to_pt(pml4e.phys_addr());
    let pdpte = &mut (*pdpt).entries[pdpt_i];
    if !pdpte.is_present() { return Ok(false); }

    let pd = phys_to_pt(pdpte.phys_addr());
    let pde = &mut (*pd).entries[pd_i];
    if !pde.is_present() { return Ok(false); }

    let pt = phys_to_pt(pde.phys_addr());
    let pte = &mut (*pt).entries[pt_i];

    if pte.0 & COW == 0 { return Ok(false); }

    let old_phys = pte.phys_addr();
    let new_phys = crate::mm::phys::alloc_pages_contiguous(1)
        .ok_or("OOM resolving CoW page")?;

    core::ptr::copy_nonoverlapping(
        phys_to_virt(old_phys) as *const u8,
        phys_to_virt(new_phys) as *mut u8,
        PAGE_SIZE as usize,
    );

    let final_flags = (vma.flags | flags::PRESENT | flags::WRITABLE) & !(COW | DEMAND);
    pte.0 = PageTableEntry::new(new_phys, final_flags).0;
    invlpg(page_addr);

    crate::cabina::trace_u64("vm", "CoW page resolved", page_addr);
    Ok(true)
}

pub unsafe fn handle_page_fault(
    fault_addr: u64,
    error_code: u64,
    pml4_phys: u64,
    vmas: &[Vma],
) -> bool {
    if error_code & 1 == 0 {
        return false;
    }

    for vma in vmas {
        if !vma.contains(fault_addr) { continue; }

        if vma.kind == VmaKind::Demand {
            if let Ok(true) = resolve_demand_page(fault_addr, pml4_phys, vma) {
                return true;
            }
        }

        let is_write = error_code & (1 << 1) != 0;
        if is_write && matches!(vma.kind, VmaKind::Cow(_)) {
            if let Ok(true) = resolve_cow_page(fault_addr, pml4_phys, vma) {
                return true;
            }
        }

        break;
    }

    false
}

/// Map all physical RAM into the high-mem region (HIGH_MEM_BASE).
/// Uses 2 MiB huge pages for efficiency. Called once at boot after
/// the UEFI memory map is parsed.
///
/// This maps every usable physical page so that `phys_to_virt(phys)`
/// works for any address in the system. Scales to 1 TB of RAM.
pub unsafe fn map_high_mem(memory_map: &[fastos_boot_protocol::MemoryEntry], count: usize) {
    use fastos_boot_protocol::MemoryType;

    crate::dev::console::serial_write("[vmm] mapping high-mem: base=0x");
    crate::boot::serial::hex(HIGH_MEM_BASE);
    crate::dev::console::serial_write("\n");

    // CRITICAL: map_high_mem runs BEFORE the high-mem mapping exists.
    // All page table pages are in low memory (< 4 GB), identity-mapped by UEFI.
    // We use phys_to_id() (identity map) here, NOT phys_to_pt() (high-mem).
    let pml4 = phys_to_id(read_cr3() & ADDR_MASK);
    let mut mapped_pages: u64 = 0;
    let mut mapped_huge: u64 = 0;

    const HUGE_2MB: u64 = 2 * 1024 * 1024;

    for i in 0..count {
        let entry = &memory_map[i];
        if entry.mem_type != MemoryType::Usable { continue; }

        let region_start = entry.base;
        let region_end = entry.base + entry.size;

        // Align to 2 MB boundary (huge page)
        let start = (region_start + HUGE_2MB - 1) & !(HUGE_2MB - 1);
        let end = (region_end) & !(HUGE_2MB - 1);

        if start >= end { continue; }

        let mut phys = start;
        while phys < end {
            let virt = HIGH_MEM_BASE + phys;

            let pml4_i = ((virt >> 39) & 0x1FF) as usize;
            let pdpt_i = ((virt >> 30) & 0x1FF) as usize;
            let pd_i   = ((virt >> 21) & 0x1FF) as usize;

            // Ensure PML4 entry exists
            let pml4e = &mut (*pml4).entries[pml4_i];
            let pdpt_phys: u64 = if !pml4e.is_present() {
                let new = alloc_page_table().expect("OOM: high-mem PML4");
                pml4e.0 = PageTableEntry::new(new, flags::PRESENT | flags::WRITABLE).0;
                new
            } else {
                pml4e.0 |= flags::WRITABLE;
                pml4e.0 &= !flags::NO_EXECUTE;
                pml4e.phys_addr()
            };

            // Ensure PDPT entry exists (identity-mapped)
            let pdpt = phys_to_id(pdpt_phys);
            let pdpte = &mut (*pdpt).entries[pdpt_i];
            let pd_phys: u64 = if !pdpte.is_present() {
                let new = alloc_page_table().expect("OOM: high-mem PDPT");
                pdpte.0 = PageTableEntry::new(new, flags::PRESENT | flags::WRITABLE).0;
                new
            } else if (pdpte.0 & flags::HUGE_PAGE) != 0 {
                // 1 GB huge page already mapped — skip this 2 MB range
                phys = (phys & !((1u64 << 30) - 1)) + (1u64 << 30);
                continue;
            } else {
                pdpte.0 |= flags::WRITABLE;
                pdpte.0 &= !flags::NO_EXECUTE;
                pdpte.phys_addr()
            };

            // Map 2 MB huge page (identity-mapped)
            let pd = phys_to_id(pd_phys);
            let pde = &mut (*pd).entries[pd_i];
            if !pde.is_present() {
                pde.0 = PageTableEntry::new(
                    phys,
                    flags::PRESENT | flags::WRITABLE | flags::HUGE_PAGE | flags::NO_EXECUTE,
                ).0;
                mapped_huge += 1;
            }
            mapped_pages += 512; // 2 MB = 512 pages

            phys += HUGE_2MB;
        }
    }

    crate::dev::console::serial_write("[vmm] high-mem mapped: ");
    crate::dev::console::serial_write_u64(mapped_huge, 10);
    crate::dev::console::serial_write(" huge pages (");
    crate::dev::console::serial_write_u64(mapped_pages / 256, 10);
    crate::dev::console::serial_write(" MB)\n");
}

pub unsafe fn mark_current_identity_user_range(start: u64, len: usize) -> Result<(), &'static str> {
    if len == 0 { return Ok(()); }
    let end = start.checked_add(len as u64).ok_or("user range overflow")?;
    let mut va = start & !(PAGE_SIZE - 1);
    let end = (end + PAGE_SIZE - 1) & !(PAGE_SIZE - 1);
    let pml4 = phys_to_pt(read_cr3() & ADDR_MASK);

    while va < end {
        let pml4_i = ((va >> 39) & 0x1FF) as usize;
        let pdpt_i = ((va >> 30) & 0x1FF) as usize;
        let pd_i = ((va >> 21) & 0x1FF) as usize;
        let pt_i = ((va >> 12) & 0x1FF) as usize;

        let pml4e = &mut (*pml4).entries[pml4_i];
        if !pml4e.is_present() { return Err("PML4 entry not present"); }
        pml4e.0 |= flags::USER | flags::WRITABLE;

        let pdpt = phys_to_pt(pml4e.phys_addr());
        let pdpte = &mut (*pdpt).entries[pdpt_i];
        if !pdpte.is_present() { return Err("PDPT entry not present"); }
        pdpte.0 |= flags::USER | flags::WRITABLE;

        if (pdpte.0 & flags::HUGE_PAGE) != 0 {
            pdpte.0 &= !flags::NO_EXECUTE;
            invlpg(va);
            va = (va & !((1u64 << 30) - 1)) + (1u64 << 30);
            continue;
        }

        let pd = phys_to_pt(pdpte.phys_addr());
        let pde = &mut (*pd).entries[pd_i];
        if !pde.is_present() { return Err("PD entry not present"); }
        pde.0 |= flags::USER | flags::WRITABLE;

        if (pde.0 & flags::HUGE_PAGE) != 0 {
            pde.0 &= !flags::NO_EXECUTE;
            invlpg(va);
            va = (va & !((1u64 << 21) - 1)) + (1u64 << 21);
            continue;
        }

        let pt = phys_to_pt(pde.phys_addr());
        let pte = &mut (*pt).entries[pt_i];
        if !pte.is_present() { return Err("PT entry not present"); }
        pte.0 |= flags::USER | flags::WRITABLE;
        pte.0 &= !flags::NO_EXECUTE;
        invlpg(va);

        va += PAGE_SIZE;
    }
    Ok(())
}
