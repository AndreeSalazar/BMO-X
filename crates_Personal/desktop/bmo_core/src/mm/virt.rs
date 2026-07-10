use crate::hal;

#[derive(Clone, Copy, Debug)]
pub struct Vma {
    pub virt_start: u64, pub virt_end: u64, pub flags: u64, pub kind: VmaKind,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum VmaKind { Fixed, Demand, Cow(u64) }

impl Vma {
    pub const fn empty() -> Self { Self { virt_start: 0, virt_end: 0, flags: 0, kind: VmaKind::Fixed } }
    pub fn contains(&self, addr: u64) -> bool { addr >= self.virt_start && addr < self.virt_end }
}

#[derive(Debug)]
pub struct AddressSpace {
    pub pml4_phys: u64, pub vmas: [Vma; 32], pub vma_count: usize,
}

impl AddressSpace {
    pub const fn empty() -> Self { Self { pml4_phys: 0, vmas: [Vma::empty(); 32], vma_count: 0 } }
    pub fn add_vma(&mut self, vma: Vma) -> bool {
        if self.vma_count >= self.vmas.len() { return false; }
        self.vmas[self.vma_count] = vma; self.vma_count += 1; true
    }
    pub fn find_vma(&self, addr: u64) -> Option<&Vma> {
        for i in 0..self.vma_count { if self.vmas[i].contains(addr) { return Some(&self.vmas[i]); } } None
    }
    pub fn find_vma_mut(&mut self, addr: u64) -> Option<&mut Vma> {
        for i in 0..self.vma_count { if self.vmas[i].contains(addr) { return Some(&mut self.vmas[i]); } } None
    }
}

pub fn map_user_range(pml4: u64, virt: u64, phys: u64, pages: usize, flags: u64) -> Result<(), &'static str> {
    if let Some(h) = unsafe { hal::HAL.as_ref() } {
        let rc = (h.map_user_range)(pml4, virt, phys, pages, flags);
        if rc == 0 { Ok(()) } else { Err("map_user_range failed") }
    } else { Err("HAL not initialized") }
}

pub fn mark_current_identity_user_range(phys: u64, size: usize) -> Result<(), &'static str> {
    if let Some(h) = unsafe { hal::HAL.as_ref() } {
        let rc = (h.mark_current_identity_user_range)(phys, size);
        if rc == 0 { Ok(()) } else { Err("mark failed") }
    } else { Err("HAL not initialized") }
}

pub fn read_cr3() -> u64 {
    if let Some(h) = unsafe { hal::HAL.as_ref() } { (h.read_cr3)() } else { 0 }
}

pub unsafe fn write_cr3(val: u64) {
    if let Some(h) = unsafe { hal::HAL.as_ref() } { (h.write_cr3)(val); }
}

pub unsafe fn free_user_page_tables(pml4: u64) {
    if let Some(h) = unsafe { hal::HAL.as_ref() } { (h.free_user_page_tables)(pml4); }
}

pub unsafe fn create_user_page_table(kernel_cr3: u64) -> Option<u64> {
    if let Some(h) = unsafe { hal::HAL.as_ref() } {
        let r = (h.create_user_page_table)(kernel_cr3);
        if r == 0 { None } else { Some(r) }
    } else { None }
}

pub fn phys_to_virt(phys: u64) -> u64 {
    if let Some(h) = unsafe { hal::HAL.as_ref() } {
        phys.wrapping_add(h.HIGH_MEM_BASE)
    } else { phys }
}

pub fn virt_to_phys(virt: u64) -> u64 {
    if let Some(h) = unsafe { hal::HAL.as_ref() } {
        if virt < h.HIGH_MEM_BASE { return 0; } // Underflow guard
        virt - h.HIGH_MEM_BASE
    } else { virt }
}

pub mod flags {
    pub const PRESENT: u64    = 1 << 0;
    pub const WRITABLE: u64   = 1 << 1;
    pub const USER: u64       = 1 << 2;
    pub const HUGE_PAGE: u64  = 1 << 7;
    pub const GLOBAL: u64     = 1 << 8;
    pub const NO_EXECUTE: u64 = 1 << 63;
    pub const DEMAND: u64     = 1 << 9;
    pub const COW: u64        = 1 << 10;
}

pub const HIGH_MEM_BASE: u64 = 0xFFFF_8000_0000_0000;
