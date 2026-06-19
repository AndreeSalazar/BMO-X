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

/// Allocate a new PML4 page and initialize it as a kernel page table.
/// This is called once during early boot to give us a clean page table
/// that we own (instead of inheriting UEFI's, which only identity-maps
/// the lower 4 GB and may have entries that conflict with our MMIO
/// mappings at addresses > 4 GB).
///
/// v1.6.0: PML4 nuevo, NO heredado de UEFI.
pub unsafe fn create_kernel_page_table() -> Option<u64> {
    // v1.6.2: DISABLED. Switching PML4 mid-execution is dangerous:
    // the current code, stack, and globals are mapped by the OLD PML4.
    // If the new PML4 doesn't have the same mappings, we triple fault
    // immediately. The proper way to switch PML4 is in long mode
    // entry assembly (with all state in low memory) which we don't have.
    //
    // Instead, we use the UEFI PML4 and ensure ALL our MMIO mappings
    // go through map_kernel_mmio_huge() which writes the UEFI PML4.
    // The UEFI PML4 has identity-mapped low memory (where the kernel
    // and stack live) AND can be extended with high-memory entries.
    crate::drivers::serial::serial_write("[paging] create_kernel_page_table: STUBBED (use UEFI PML4)\n");
    None
}

fn print_hex_debug(val: u64) {
    let hex = b"0123456789ABCDEF";
    let mut buf = [0u8; 18];
    buf[0] = b'0';
    buf[1] = b'x';
    for i in 0..16 {
        let nib = ((val >> (60 - i * 4)) & 0xF) as usize;
        buf[2 + i] = hex[nib];
    }
    crate::drivers::serial::serial_write(unsafe {
        core::str::from_utf8_unchecked(&buf)
    });
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
///
/// v1.6.4: Use the kernel heap (already initialized in Phase 1) instead of
/// the page-frame bitmap. Reason: the bitmap allocator may hand back pages
/// in regions that the UEFI PML4 left marked read-only (firmware data,
/// EfiRuntimeServicesData, etc). Writing to such pages while we build new
/// page tables triggers a #PF with CR2 = the page-table page address,
/// exactly the fault we saw in v1.6.3 (CR2=0xBDC01000, Error=0x3, RIP
/// inside map_kernel_mmio_huge). The kernel heap lives in a region we
/// know is R/W because we just zeroed it during init_heap().
unsafe fn alloc_page_table() -> Option<u64> {
    // Over-allocate (8 KB) so we can return a 4 KB-aligned slice even
    // though our free-list only honors 8-byte alignment.
    let raw = crate::allocator::heap_alloc(8192, 8);
    if raw.is_null() {
        return None;
    }
    let raw_addr = raw as usize;
    // Round up to next 4 KB boundary.
    let aligned_addr = (raw_addr + 4095) & !4095;
    let aligned = aligned_addr as *mut u8;
    let pad = aligned_addr - raw_addr;
    if pad >= 8 {
        let stash = (aligned_addr - 8) as *mut usize;
        core::ptr::write_unaligned(stash, raw_addr);
    }
    core::ptr::write_bytes(aligned, 0, 4096);
    Some(aligned as u64)
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

/// Map a kernel MMIO region using 2 MiB huge pages.
///
/// Used to map PCI Express ECAM (which lives above 4 GB on Ryzen / Threadripper)
/// so the kernel can access config space without #PF.
///
/// `virt_start` is typically `phys_start` (identity-style) or a chosen high
/// address. `bytes` is rounded up to a multiple of 2 MiB.
pub unsafe fn map_kernel_mmio_huge(
    phys_start: u64,
    virt_start: u64,
    bytes: usize,
) -> Result<(), &'static str> {
    if bytes == 0 {
        return Ok(());
    }
    const HUGE_2MB: u64 = 2 * 1024 * 1024;
    let pages = (bytes + (HUGE_2MB as usize) - 1) / (HUGE_2MB as usize);
    if (virt_start & (HUGE_2MB - 1)) != 0 || (phys_start & (HUGE_2MB - 1)) != 0 {
        return Err("map_kernel_mmio_huge: addresses must be 2 MiB aligned");
    }

    let pml4_phys = read_cr3() & 0x000F_FFFF_FFFF_F000;
    let pml4 = pml4_phys as *mut PageTable;

    for i in 0..pages {
        let va = virt_start + (i as u64) * HUGE_2MB;
        let pa = phys_start + (i as u64) * HUGE_2MB;

        let pml4_i = ((va >> 39) & 0x1FF) as usize;
        let pdpt_i = ((va >> 30) & 0x1FF) as usize;
        let pd_i   = ((va >> 21) & 0x1FF) as usize;

        // v1.6.4 FIX: UEFI marks PML4 / PDPT / PD entries as R/O to protect
        // firmware data. When we try to write a child entry (PDPT, PD, or
        // PT) we need the parent entry to allow writes. We ALWAYS force
        // WRITABLE on PML4/PDPT/PD entries we traverse so the subsequent
        // child-level writes don't #PF with CR2 = parent entry address.

        // PML4 entry
        let pml4e = &mut (*pml4).entries[pml4_i];
        let pdpt_phys: u64 = if !pml4e.is_present() {
            let new = alloc_page_table().ok_or("OOM: PML4->PDPT")?;
            core::ptr::write_bytes(new as *mut u8, 0, PAGE_SIZE as usize);
            pml4e.0 = PageTableEntry::new(new, flags::PRESENT | flags::WRITABLE).0;
            new
        } else {
            // Force WRITABLE on UEFI's existing PML4 entry so we can write
            // the PDPT entry below. Clear NX so we can execute page-table
            // walks from this level (UEFI sometimes sets NX on data).
            pml4e.0 |= flags::WRITABLE;
            pml4e.0 &= !flags::NO_EXECUTE;
            pml4e.phys_addr()
        };

        // PDPT entry
        let pdpt = pdpt_phys as *mut PageTable;
        let pdpte = &mut (*pdpt).entries[pdpt_i];
        // SAFETY: PDPT can be a 1 GiB huge page from UEFI. We must NOT
        // touch the entry if it's a huge page — we just leave it alone and
        // add a new PD under a sibling PML4 slot if needed.
        let pd_phys: u64 = if !pdpte.is_present() {
            let new = alloc_page_table().ok_or("OOM: PDPT->PD")?;
            core::ptr::write_bytes(new as *mut u8, 0, PAGE_SIZE as usize);
            pdpte.0 = PageTableEntry::new(new, flags::PRESENT | flags::WRITABLE).0;
            new
        } else if (pdpte.0 & flags::HUGE_PAGE) != 0 {
            // PDPT entry is a 1 GiB huge page. We cannot sub-allocate 2 MiB
            // pages from it. We have to either reuse it as-is (which means
            // our ECAM region must be 1 GiB-aligned and within the existing
            // 1 GiB mapping), or fail. For now we fail with a clear error
            // so the caller can fall back to IO-port PCI.
            return Err("map_kernel_mmio_huge: PDPT entry is 1 GiB huge page, cannot sub-allocate 2 MiB");
        } else {
            // Force WRITABLE on the preexistent PDPT entry, clear NX.
            pdpte.0 |= flags::WRITABLE;
            pdpte.0 &= !flags::NO_EXECUTE;
            pdpte.phys_addr()
        };

        // PD entry — set 2 MiB huge page pointing at `pa`
        let pd = pd_phys as *mut PageTable;
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

// ══════════════════════════════════════════════════════════════════════
// Demand Paging + Virtual Memory Area (VMA) tracking
// ══════════════════════════════════════════════════════════════════════

/// Software flag (bit 9) — page is demand-allocated (not yet present).
/// Set in PTE when a VMA is marked Demand; cleared when the page is faulted in.
pub const DEMAND: u64 = 1 << 9;

/// Software flag (bit 10) — Copy-on-Write page.
/// Write-triggered #PF on a CoW page allocates a fresh copy.
pub const COW: u64 = 1 << 10;

/// A single Virtual Memory Area.
#[derive(Clone, Copy, Debug)]
pub struct Vma {
    pub virt_start: u64,
    pub virt_end: u64,   // exclusive
    pub flags: u64,       // PTE flags (USER | WRITABLE | NO_EXECUTE, etc.)
    pub kind: VmaKind,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum VmaKind {
    /// Pages allocated upfront (old behavior).
    Fixed,
    /// Pages allocated on first access (#PF handler resolves).
    Demand,
    /// Copy-on-Write: shares physical pages with another address space.
    /// The u64 is the "backing" PML4 where the original pages live.
    Cow(u64),
}

impl Vma {
    pub const fn empty() -> Self {
        Self { virt_start: 0, virt_end: 0, flags: 0, kind: VmaKind::Fixed }
    }

    pub fn contains(&self, addr: u64) -> bool {
        addr >= self.virt_start && addr < self.virt_end
    }

    pub fn is_demand(&self) -> bool {
        self.kind == VmaKind::Demand
    }
}

/// Per-process address space tracker.
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

    /// Add a VMA to this address space. Returns false if full.
    pub fn add_vma(&mut self, vma: Vma) -> bool {
        if self.vma_count >= self.vmas.len() {
            return false;
        }
        self.vmas[self.vma_count] = vma;
        self.vma_count += true as usize;
        true
    }

    /// Find the VMA that contains `addr`, if any.
    pub fn find_vma(&self, addr: u64) -> Option<&Vma> {
        for i in 0..self.vma_count {
            if self.vmas[i].contains(addr) {
                return Some(&self.vmas[i]);
            }
        }
        None
    }

    /// Find the VMA containing `addr` (mutable).
    pub fn find_vma_mut(&mut self, addr: u64) -> Option<&mut Vma> {
        for i in 0..self.vma_count {
            if self.vmas[i].contains(addr) {
                return Some(&mut self.vmas[i]);
            }
        }
        None
    }
}

/// Map a user virtual address range as DEMAND pages (lazily allocated on #PF).
///
/// Creates PML4→PDPT→PD→PT entries with PRESENT=0 and DEMAND flag set.
/// The #PF handler will allocate physical pages on first access.
pub unsafe fn map_user_demand(
    pml4_phys: u64,
    virt_start: u64,
    pages: usize,
    flags: u64,
) -> Result<(), &'static str> {
    if pages == 0 {
        return Ok(());
    }
    let pml4 = pml4_phys as *mut PageTable;
    let mut va = virt_start;

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
        }

        // PT entry — NOT present, but with DEMAND flag
        let pt = pt_phys as *mut PageTable;
        let pte = &mut (*pt).entries[pt_i];
        if pte.is_present() {
            return Err("Page already mapped");
        }
        // PRESENT=0, DEMAND=1 — page will be allocated on first #PF
        pte.0 = (flags | DEMAND) & !flags::PRESENT;

        va += PAGE_SIZE;
    }

    Ok(())
}

/// Resolve a demand page fault: allocate a physical page and map it.
///
/// Called from the #PF handler when the fault address is in a demand VMA.
/// Returns Ok(true) if the fault was resolved, Ok(false) if not a demand fault,
/// Err if the fault is fatal.
pub unsafe fn resolve_demand_page(
    fault_addr: u64,
    pml4_phys: u64,
    vma: &Vma,
) -> Result<bool, &'static str> {
    let page_addr = fault_addr & !(PAGE_SIZE - 1);
    let pml4 = pml4_phys as *mut PageTable;

    let pml4_i = ((page_addr >> 39) & 0x1FF) as usize;
    let pdpt_i = ((page_addr >> 30) & 0x1FF) as usize;
    let pd_i = ((page_addr >> 21) & 0x1FF) as usize;
    let pt_i = ((page_addr >> 12) & 0x1FF) as usize;

    // Walk to the PT entry
    let pml4e = &mut (*pml4).entries[pml4_i];
    if !pml4e.is_present() { return Ok(false); }

    let pdpt = pml4e.phys_addr() as *mut PageTable;
    let pdpte = &mut (*pdpt).entries[pdpt_i];
    if !pdpte.is_present() { return Ok(false); }

    let pd = pdpte.phys_addr() as *mut PageTable;
    let pde = &mut (*pd).entries[pd_i];
    if !pde.is_present() { return Ok(false); }

    let pt = pde.phys_addr() as *mut PageTable;
    let pte = &mut (*pt).entries[pt_i];

    // Check if this is a demand page
    if pte.0 & DEMAND == 0 {
        return Ok(false); // Not a demand page
    }

    // Allocate a physical page
    let phys = crate::arch::page_alloc::alloc_pages_contiguous(1)
        .ok_or("OOM resolving demand page")?;

    // Zero the page
    core::ptr::write_bytes(phys as *mut u8, 0, PAGE_SIZE as usize);

    // Map it: PRESENT=1, DEMAND=0, keep other flags
    let final_flags = (vma.flags | flags::PRESENT) & !DEMAND;
    pte.0 = PageTableEntry::new(phys, final_flags).0;

    // Invalidate TLB for this page
    invlpg(page_addr);

    crate::diag::trace_u64("vm", "demand page resolved", page_addr);
    Ok(true)
}

/// Resolve a Copy-on-Write page fault: allocate a fresh copy and map it.
///
/// Called from the #PF handler when a write hits a CoW page.
pub unsafe fn resolve_cow_page(
    fault_addr: u64,
    pml4_phys: u64,
    vma: &Vma,
) -> Result<bool, &'static str> {
    let page_addr = fault_addr & !(PAGE_SIZE - 1);
    let pml4 = pml4_phys as *mut PageTable;

    let pml4_i = ((page_addr >> 39) & 0x1FF) as usize;
    let pdpt_i = ((page_addr >> 30) & 0x1FF) as usize;
    let pd_i = ((page_addr >> 21) & 0x1FF) as usize;
    let pt_i = ((page_addr >> 12) & 0x1FF) as usize;

    // Walk to the PT entry
    let pml4e = &mut (*pml4).entries[pml4_i];
    if !pml4e.is_present() { return Ok(false); }

    let pdpt = pml4e.phys_addr() as *mut PageTable;
    let pdpte = &mut (*pdpt).entries[pdpt_i];
    if !pdpte.is_present() { return Ok(false); }

    let pd = pdpte.phys_addr() as *mut PageTable;
    let pde = &mut (*pd).entries[pd_i];
    if !pde.is_present() { return Ok(false); }

    let pt = pde.phys_addr() as *mut PageTable;
    let pte = &mut (*pt).entries[pt_i];

    // Check if this is a CoW page
    if pte.0 & COW == 0 {
        return Ok(false); // Not a CoW page
    }

    // Read the old physical page content
    let old_phys = pte.phys_addr();

    // Allocate a fresh page
    let new_phys = crate::arch::page_alloc::alloc_pages_contiguous(1)
        .ok_or("OOM resolving CoW page")?;

    // Copy old content to new page
    core::ptr::copy_nonoverlapping(
        old_phys as *const u8,
        new_phys as *mut u8,
        PAGE_SIZE as usize,
    );

    // Map new page: PRESENT=1, COW=0, WRITABLE=1
    let final_flags = (vma.flags | flags::PRESENT | flags::WRITABLE) & !(COW | DEMAND);
    pte.0 = PageTableEntry::new(new_phys, final_flags).0;

    invlpg(page_addr);

    crate::diag::trace_u64("vm", "CoW page resolved", page_addr);
    Ok(true)
}

/// Generic page fault resolver: tries demand, then CoW.
///
/// Returns Ok(true) if resolved, Ok(false) if not our fault (pass to kill handler).
pub unsafe fn handle_page_fault(
    fault_addr: u64,
    error_code: u64,
    pml4_phys: u64,
    vmas: &[Vma],
) -> bool {
    // Kernel-mode faults are always fatal (should not happen in normal operation)
    // Bit 0 of error code: 0 = fault in supervisor mode → fatal
    if error_code & 1 == 0 {
        return false; // Not a user-mode fault → kill
    }

    // Find the VMA for this fault address
    for vma in vmas {
        if !vma.contains(fault_addr) {
            continue;
        }

        // Demand page: first access allocates
        if vma.kind == VmaKind::Demand {
            if let Ok(true) = resolve_demand_page(fault_addr, pml4_phys, vma) {
                return true;
            }
        }

        // CoW: write access triggers copy
        let is_write = error_code & (1 << 1) != 0;
        if is_write && matches!(vma.kind, VmaKind::Cow(_)) {
            if let Ok(true) = resolve_cow_page(fault_addr, pml4_phys, vma) {
                return true;
            }
        }

        // Present page but permission violation (e.g., write to read-only)
        // This is a real fault
        break;
    }

    false // Not resolved → caller should kill process
}

