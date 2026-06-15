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
