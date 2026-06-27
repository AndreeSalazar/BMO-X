//! mmap/munmap Syscalls (Ring 0 HAL).
//!
//! Provides memory mapping services for Ring 3 processes:
//!   - mmap: Map files or anonymous memory into process address space
//!   - munmap: Unmap previously mapped regions
//!   - mprotect: Change protection on mapped regions
//!
//! Architecture:
//!   - Each process has its own address space (page tables)
//!   - mmap creates VMAs (Virtual Memory Areas) in the process
//!   - Page faults trigger demand allocation or file read
//!
//! These are Ring 0 service stubs — BMO Core calls them
//! when handling Ring 3 syscalls.

/// Memory protection flags.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MemProt {
    pub read: bool,
    pub write: bool,
    pub execute: bool,
}

impl MemProt {
    pub const READ: Self = Self { read: true, write: false, execute: false };
    pub const READ_WRITE: Self = Self { read: true, write: true, execute: false };
    pub const READ_EXEC: Self = Self { read: true, write: false, execute: true };
    pub const NONE: Self = Self { read: false, write: false, execute: false };
}

/// Memory mapping type.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MapType {
    /// Map a file region (needs file descriptor + offset)
    File { fd: u32, offset: u64 },
    /// Anonymous mapping (zero-filled, not backed by file)
    Anonymous,
    /// Shared memory between processes
    Shared { name: u32 }, // shared memory ID
}

/// mmap result — the base address of the mapping.
#[derive(Debug, Clone, Copy)]
pub struct MapResult {
    pub base: u64,
    pub size: u64,
}

/// Map memory into the current process's address space.
///
/// # Arguments
/// * `addr` - Hint address (0 = kernel chooses)
/// * `size` - Size in bytes (rounded up to page boundary)
/// * `prot` - Memory protection
/// * `map_type` - Mapping type (file, anonymous, shared)
///
/// Returns the mapped address, or an error code.
pub fn mmap(
    addr: u64,
    size: u64,
    prot: MemProt,
    map_type: MapType,
) -> Result<MapResult, MmapError> {
    // Align size to page boundary
    let page_size = crate::mm::PAGE_SIZE;
    let aligned_size = (size + page_size - 1) & !(page_size - 1);

    if aligned_size == 0 {
        return Err(MmapError::InvalidArgument);
    }

    // TODO: Find free virtual address range in process address space
    // TODO: Create VMA entry
    // TODO: For file mappings, set up demand paging from file
    // TODO: For anonymous, set up demand zero-fill

    crate::dev::console::serial_write("[mmap] stub: addr=0x");
    crate::dev::console::serial_write_u64(addr, 16);
    crate::dev::console::serial_write(" size=");
    crate::dev::console::serial_write_u64(aligned_size, 10);
    crate::dev::console::serial_write("\n");

    // Temporary: allocate from slab
    let ptr = unsafe {
        alloc::alloc::alloc(core::alloc::Layout::from_size_align(
            aligned_size as usize,
            page_size as usize,
        ).unwrap())
    };

    if ptr.is_null() {
        return Err(MmapError::OutOfMemory);
    }

    Ok(MapResult {
        base: ptr as u64,
        size: aligned_size,
    })
}

/// Unmap a previously mapped region.
pub fn munmap(addr: u64, size: u64) -> Result<(), MmapError> {
    let page_size = crate::mm::PAGE_SIZE;
    let aligned_size = (size + page_size - 1) & !(page_size - 1);

    // TODO: Find and remove VMA
    // TODO: Unmap pages
    // TODO: Free physical frames if anonymous

    crate::dev::console::serial_write("[munmap] stub: addr=0x");
    crate::dev::console::serial_write_u64(addr, 16);
    crate::dev::console::serial_write("\n");

    Ok(())
}

/// Change memory protection on a mapped region.
pub fn mprotect(addr: u64, size: u64, prot: MemProt) -> Result<(), MmapError> {
    // TODO: Find VMA, update protection, update page table entries
    crate::dev::console::serial_write("[mprotect] stub\n");
    Ok(())
}

/// mmap error codes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MmapError {
    InvalidArgument,
    OutOfMemory,
    PermissionDenied,
    FileError,
    AlreadyMapped,
}
