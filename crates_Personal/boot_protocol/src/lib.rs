//! FastOS Boot Protocol — shared types between UEFI bootloader and kernel.
//!
//! The bootloader fills `BootInfo`, the kernel reads it.
//! Single pointer in RDI — that's the entire ABI.
//!
//! v2: Builder pattern, better docs, memory helpers.

#![no_std]

/// Protocol version — bump on breaking layout changes.
pub const PROTOCOL_VERSION: u32 = 2;

/// Magic value to verify BootInfo integrity on kernel entry.
pub const BOOT_MAGIC: u64 = 0xFA57_0505_B007_1AF0;

// ── Memory Types ─────────────────────────────────────────────────────────────

#[repr(u32)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
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

impl MemoryType {
    #[inline]
    pub fn is_usable(self) -> bool {
        matches!(self, Self::Usable | Self::AcpiReclaimable | Self::Bootloader)
    }
}

// ── Memory Map ───────────────────────────────────────────────────────────────

/// Maximum memory map entries (fits in a single page).
pub const MAX_MEMORY_ENTRIES: usize = 256;

#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct MemoryEntry {
    pub base: u64,
    pub size: u64,
    pub mem_type: MemoryType,
    pub _pad: u32,
}

impl MemoryEntry {
    #[inline]
    pub fn end(&self) -> u64 {
        self.base.wrapping_add(self.size)
    }

    #[inline]
    pub fn contains(&self, addr: u64) -> bool {
        addr >= self.base && addr < self.end()
    }
}

// ── Pixel Format ─────────────────────────────────────────────────────────────

#[repr(u32)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum PixelFormat {
    Bgr = 0,
    Rgb = 1,
    Unknown = 255,
}

// ── Boot Info ────────────────────────────────────────────────────────────────

/// Boot information passed from UEFI bootloader to kernel via RDI.
///
/// Fixed layout — no pointers, no allocations, pure data.
#[repr(C)]
#[derive(Clone, Copy)]
pub struct BootInfo {
    /// Must be `BOOT_MAGIC`.
    pub magic: u64,
    /// Protocol version — kernel checks this.
    pub version: u32,
    pub _pad: u32,

    // ── Framebuffer (flat for backwards compat) ──
    pub fb_addr: u64,
    pub fb_size: u64,
    pub fb_width: u32,
    pub fb_height: u32,
    pub fb_stride: u32,
    pub fb_pixel_format: PixelFormat,

    // ── Memory map ──
    pub memory_map_count: u32,
    pub _pad2: u32,
    pub memory_map: [MemoryEntry; MAX_MEMORY_ENTRIES],

    // ── ACPI ──
    pub rsdp_addr: u64,

    // ── Kernel ──
    pub kernel_base: u64,
    pub kernel_size: u64,

    // ── Stack ──
    pub stack_top: u64,
    pub stack_size: u64,

    // ── Reserved (0 in GOP path) ──
    pub reserved_addr: u64,
    pub reserved_size: u64,

    // ── UEFI Runtime Services ──
    /// Physical address of UEFI System Table. After ExitBootServices,
    /// only Runtime Services are valid. Kernel uses this for NVRAM access.
    pub uefi_system_table: u64,
}

impl BootInfo {
    /// Verify magic + version.
    #[inline]
    pub fn is_valid(&self) -> bool {
        self.magic == BOOT_MAGIC && self.version >= PROTOCOL_VERSION
    }

    /// Framebuffer pitch in bytes (stride × 4 for 32bpp).
    #[inline]
    pub fn fb_pitch(&self) -> u64 {
        self.fb_stride as u64 * 4
    }

    /// Usable memory entries iterator.
    pub fn usable_memory(&self) -> impl Iterator<Item = &MemoryEntry> {
        self.memory_map[..self.memory_map_count as usize]
            .iter()
            .filter(|e| e.mem_type.is_usable())
    }

    /// Total usable memory in bytes.
    pub fn total_usable_memory(&self) -> u64 {
        self.usable_memory().map(|e| e.size).sum()
    }
}

// ── Builder ──────────────────────────────────────────────────────────────────

/// Builder for `BootInfo` — fluent API for the bootloader.
pub struct BootInfoBuilder {
    inner: BootInfo,
}

impl BootInfoBuilder {
    pub fn new() -> Self {
        Self {
            inner: BootInfo {
                magic: BOOT_MAGIC,
                version: PROTOCOL_VERSION,
                _pad: 0,
                fb_addr: 0, fb_size: 0, fb_width: 0, fb_height: 0,
                fb_stride: 0, fb_pixel_format: PixelFormat::Unknown,
                memory_map_count: 0, _pad2: 0,
                memory_map: [MemoryEntry {
                    base: 0, size: 0, mem_type: MemoryType::Reserved, _pad: 0,
                }; MAX_MEMORY_ENTRIES],
                rsdp_addr: 0,
                kernel_base: 0, kernel_size: 0,
                stack_top: 0, stack_size: 0,
                reserved_addr: 0, reserved_size: 0,
                uefi_system_table: 0,
            },
        }
    }

    pub fn framebuffer(mut self, addr: u64, size: u64, w: u32, h: u32, stride: u32, fmt: PixelFormat) -> Self {
        self.inner.fb_addr = addr;
        self.inner.fb_size = size;
        self.inner.fb_width = w;
        self.inner.fb_height = h;
        self.inner.fb_stride = stride;
        self.inner.fb_pixel_format = fmt;
        self
    }

    pub fn rsdp(mut self, addr: u64) -> Self {
        self.inner.rsdp_addr = addr;
        self
    }

    pub fn kernel(mut self, base: u64, size: u64) -> Self {
        self.inner.kernel_base = base;
        self.inner.kernel_size = size;
        self
    }

    pub fn stack(mut self, top: u64, size: u64) -> Self {
        self.inner.stack_top = top;
        self.inner.stack_size = size;
        self
    }

    pub fn uefi_system_table(mut self, addr: u64) -> Self {
        self.inner.uefi_system_table = addr;
        self
    }

    pub fn reserved(mut self, addr: u64, size: u64) -> Self {
        self.inner.reserved_addr = addr;
        self.inner.reserved_size = size;
        self
    }

    /// Add a memory map entry. Returns false if map is full.
    pub fn add_memory_entry(&mut self, entry: MemoryEntry) -> bool {
        let idx = self.inner.memory_map_count as usize;
        if idx >= MAX_MEMORY_ENTRIES {
            return false;
        }
        self.inner.memory_map[idx] = entry;
        self.inner.memory_map_count += 1;
        true
    }

    /// Build the final BootInfo.
    pub fn build(self) -> BootInfo {
        self.inner
    }
}
