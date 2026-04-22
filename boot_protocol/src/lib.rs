//! FastOS Boot Protocol — shared types between bootloader and kernel.
//!
//! Defines the BootInfo structure passed from UEFI bootloader to kernel.
//! The bootloader fills this, the kernel reads it. One pointer in RDI.

#![no_std]

/// Magic value to verify BootInfo integrity.
pub const BOOT_MAGIC: u64 = 0xFA57_0505_B007_1AF0;

/// Memory type tags (simplified from UEFI memory types).
#[repr(u32)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MemoryType {
    Usable = 0,
    Reserved = 1,
    AcpiReclaimable = 2,
    AcpiNvs = 3,
    Unusable = 4,
    Bootloader = 5,
    KernelCode = 6,
    Framebuffer = 7,
}

/// A single memory map entry.
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct MemoryEntry {
    pub base: u64,
    pub size: u64,
    pub mem_type: MemoryType,
    pub _pad: u32,
}

/// Maximum memory map entries we support.
pub const MAX_MEMORY_ENTRIES: usize = 256;

/// Pixel format of the framebuffer.
#[repr(u32)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PixelFormat {
    Bgr = 0,  // Blue-Green-Red (most common UEFI GOP format)
    Rgb = 1,
    Unknown = 255,
}

/// Boot information passed from UEFI bootloader to kernel via RDI.
///
/// The bootloader allocates this in memory that persists after ExitBootServices,
/// fills all fields, then jumps to `_start` with a pointer to this in RDI.
#[repr(C)]
pub struct BootInfo {
    /// Must be BOOT_MAGIC.
    pub magic: u64,

    // ── Framebuffer (from UEFI GOP) ──
    pub fb_addr: u64,
    pub fb_size: u64,
    pub fb_width: u32,
    pub fb_height: u32,
    pub fb_stride: u32,    // pixels per scanline
    pub fb_pixel_format: PixelFormat,

    // ── Memory map ──
    pub memory_map_count: u64,
    pub memory_map: [MemoryEntry; MAX_MEMORY_ENTRIES],

    // ── ACPI ──
    pub rsdp_addr: u64,

    // ── Kernel location ──
    pub kernel_base: u64,
    pub kernel_size: u64,

    // ── Stack ──
    pub stack_top: u64,
    pub stack_size: u64,

    // ── GPU firmware (optional, 0 if not loaded) ──
    pub gsp_addr: u64,
    pub gsp_size: u64,
}

impl BootInfo {
    /// Verify the magic field.
    pub fn is_valid(&self) -> bool {
        self.magic == BOOT_MAGIC
    }

    /// Framebuffer pitch in bytes (UEFI GOP stride is already in bytes per scanline).
    pub fn fb_pitch(&self) -> u64 {
        self.fb_stride as u64
    }
}
