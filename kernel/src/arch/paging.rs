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
