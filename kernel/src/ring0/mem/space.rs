#![allow(dead_code)]

//! ⚠️  DEPRECATED v1.8.7 — No usar. Los tipos VM reales del sistema
//! están en `crate::mem::virt` (`AddressSpace`, `Vma`, `VmaKind`).
//!
//! Este módulo se conserva temporalmente para no romper referencias
//! externas accidentales. Se eliminará en la próxima major. Si estás
//! añadiendo código nuevo, importa desde `crate::mem::virt` directamente.
//!
//! Histórico: `space.rs` definía `VmaFlags`, `VmaType`, `Vma` y
//! `VmSpace` como una versión paralela al subsistema de paginación.
//! `mem::virt` consolidó todo y estos tipos quedaron redundantes.

use crate::mem::phys;
use crate::boot::serial::{u32_dec, hex as serial_hex};
use crate::dev::console;

/// Virtual memory area flags.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct VmaFlags {
    pub readable: bool,
    pub writable: bool,
    pub executable: bool,
    pub user: bool,
}

impl VmaFlags {
    pub const READ_ONLY: Self = Self {
        readable: true, writable: false, executable: false, user: true,
    };
    pub const READ_WRITE: Self = Self {
        readable: true, writable: true, executable: false, user: true,
    };
    pub const READ_EXEC: Self = Self {
        readable: true, writable: false, executable: true, user: true,
    };
    pub const READ_WRITE_EXEC: Self = Self {
        readable: true, writable: true, executable: true, user: true,
    };
    pub const KERNEL_ONLY: Self = Self {
        readable: true, writable: true, executable: false, user: false,
    };
}

/// VMA type.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum VmaType {
    Fixed,      // Pre-allocated at specific address
    Demand,     // Allocated on first page fault
    Cow,        // Copy-on-write (fork)
    Stack,      // Thread stack (grows down)
    Mapped,     // Memory-mapped I/O or file
}

/// Virtual Memory Area — contiguous region of virtual address space.
#[derive(Debug, Clone, Copy)]
pub struct Vma {
    pub start: u64,
    pub end: u64,
    pub flags: VmaFlags,
    pub vtype: VmaType,
}

impl Vma {
    pub fn contains(&self, addr: u64) -> bool {
        addr >= self.start && addr < self.end
    }

    pub fn size(&self) -> u64 {
        self.end - self.start
    }
}

/// Maximum VMAs per address space.
const MAX_VMAS: usize = 32;

/// Virtual address space for a process.
#[derive(Clone, Copy)]
pub struct VmSpace {
    pub vmas: [Vma; MAX_VMAS],
    pub count: usize,
    pub heap_end: u64,     // Program break
    pub stack_bottom: u64, // Bottom of stack region
    pub mmap_top: u64,     // Top of mmap region (grows down)
}

impl VmSpace {
    pub const fn empty() -> Self {
        Self {
            vmas: [Vma {
                start: 0, end: 0,
                flags: VmaFlags::READ_ONLY,
                vtype: VmaType::Fixed,
            }; MAX_VMAS],
            count: 0,
            heap_end: 0x0060_0000,     // 6 MB default heap start
            stack_bottom: 0x7F00_0000, // Stack below 2 GB
            mmap_top: 0x7E00_0000,     // mmap region below stack
        }
    }

    /// Add a VMA to this address space.
    pub fn add_vma(&mut self, vma: Vma) -> bool {
        if self.count >= MAX_VMAS {
            return false;
        }
        self.vmas[self.count] = vma;
        self.count += 1;
        true
    }

    /// Find VMA containing `addr`.
    pub fn find_vma(&self, addr: u64) -> Option<&Vma> {
        for i in 0..self.count {
            if self.vmas[i].contains(addr) {
                return Some(&self.vmas[i]);
            }
        }
        None
    }

    /// Find mutable VMA containing `addr`.
    pub fn find_vma_mut(&mut self, addr: u64) -> Option<&mut Vma> {
        for i in 0..self.count {
            if self.vmas[i].contains(addr) {
                return Some(&mut self.vmas[i]);
            }
        }
        None
    }
}

/// Global address spaces for all processes (indexed by PID).
const MAX_PROCS: usize = 64;
static mut VM_SPACES: [VmSpace; MAX_PROCS] = [VmSpace::empty(); MAX_PROCS];

/// Inicializa el VMM. v1.7.4: no-op (las VM_SPACES ya están en BSS).
/// El VMM se popula dinámicamente con `get_or_create(pid)` y
/// `create_process_space(pid)`.
pub fn init() {
    // No-op por ahora. v2.0: bootstrap kernel page tables aquí.
}

/// Get or create a virtual address space for a process.
pub fn get_or_create(pid: u32) -> &'static mut VmSpace {
    let idx = (pid as usize) % MAX_PROCS;
    unsafe { &mut VM_SPACES[idx] }
}

/// Handle a page fault within a VMA.
/// Returns true if the fault was resolved.
pub fn handle_page_fault(addr: u64, write: bool) -> bool {
    let pid = 0; // TODO: get current PID
    let vms = get_or_create(pid);

    if let Some(vma) = vms.find_vma_mut(addr) {
        // Check permissions
        if write && !vma.flags.writable {
            return false; // Permission denied
        }

        match vma.vtype {
            VmaType::Demand => {
                // Allocate a physical page
                let phys = unsafe { phys::alloc_pages_contiguous(1) };
                match phys {
                    Some(p) => {
                        // Zero the page
                        let page = p as *mut u8;
                        unsafe { core::ptr::write_bytes(page, 0, 4096); }
                        // Map it at the faulting address
                        // TODO: call virt::map_page()
                        true
                    }
                    None => false,
                }
            }
            VmaType::Stack => {
                // Stack overflow — allocate more pages below
                let phys = unsafe { phys::alloc_pages_contiguous(1) };
                match phys {
                    Some(_p) => {
                        // TODO: map the new page
                        true
                    }
                    None => false,
                }
            }
            _ => false,
        }
    } else {
        false // No VMA at this address
    }
}

/// Create a new process address space with standard layout.
pub fn create_process_space(pid: u32) -> &'static mut VmSpace {
    let vms = get_or_create(pid);

    // Code segment (read+execute)
    vms.add_vma(Vma {
        start: 0x0040_0000,
        end: 0x0060_0000,
        flags: VmaFlags::READ_EXEC,
        vtype: VmaType::Fixed,
    });

    // Heap (read+write, demand-paged)
    vms.add_vma(Vma {
        start: 0x0060_0000,
        end: 0x0080_0000,
        flags: VmaFlags::READ_WRITE,
        vtype: VmaType::Demand,
    });

    // Stack (read+write, demand-paged, grows down)
    vms.add_vma(Vma {
        start: 0x7E00_0000,
        end: 0x7F00_0000,
        flags: VmaFlags::READ_WRITE,
        vtype: VmaType::Stack,
    });

    vms.heap_end = 0x0060_0000;
    vms
}

/// Print VMA info for a process.
pub fn dump_vmas(pid: u32) {
    let vms = get_or_create(pid);
    console::serial_write("[vmm] VMA dump for PID=");
    u32_dec(pid);
    console::serial_write(" count=");
    u32_dec(vms.count as u32);
    console::serial_write("\n");

    for i in 0..vms.count {
        let vma = &vms.vmas[i];
        console::serial_write("  [");
        serial_hex(vma.start);
        console::serial_write(" - ");
        serial_hex(vma.end);
        console::serial_write("] ");
        if vma.flags.readable { console::serial_write("R"); }
        if vma.flags.writable { console::serial_write("W"); }
        if vma.flags.executable { console::serial_write("X"); }
        match vma.vtype {
            VmaType::Fixed => console::serial_write(" FIXED"),
            VmaType::Demand => console::serial_write(" DEMAND"),
            VmaType::Cow => console::serial_write(" COW"),
            VmaType::Stack => console::serial_write(" STACK"),
            VmaType::Mapped => console::serial_write(" MAPPED"),
        }
        console::serial_write("\n");
    }
}
