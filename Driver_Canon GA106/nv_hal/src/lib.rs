//! # nv_hal — Hardware Abstraction Layer
//!
//! Provides safe wrappers over raw hardware access:
//! - MMIO register read/write (volatile)
//! - PCI configuration space access
//! - DMA buffer allocation interface
//! - Microsecond-precision delays
//!
//! Mirrors the 4 HAL.dll imports nvlddmkm.sys uses:
//!   HalGetBusDataByOffset, HalSetBusDataByOffset,
//!   KeStallExecutionProcessor, KeQueryPerformanceCounter
//!
//! `#![no_std]` compatible. The OS must provide a `Platform` implementation.

#![no_std]

use nv_error::{NvError, NvResult};
use nv_regs::{NVIDIA_VENDOR_ID, GA106_DEVICE_ID};

// ── MMIO Register Access ────────────────────────────────────────────────────

/// A mapped GPU register region (BAR0).
/// Created by mapping physical BAR address into virtual address space.
pub struct MmioRegion {
    base: *mut u8,
    size: usize,
}

impl MmioRegion {
    /// Create from a raw pointer to the mapped BAR region.
    ///
    /// # Safety
    /// `base` must point to a valid MMIO-mapped region of `size` bytes.
    pub unsafe fn new(base: *mut u8, size: usize) -> Self {
        Self { base, size }
    }

    /// Read a 32-bit register at `offset` bytes from BAR0 base.
    #[inline]
    pub fn read32(&self, offset: u32) -> u32 {
        assert!((offset as usize + 4) <= self.size, "MMIO read out of bounds");
        unsafe {
            core::ptr::read_volatile(self.base.add(offset as usize) as *const u32)
        }
    }

    /// Write a 32-bit value to register at `offset`.
    #[inline]
    pub fn write32(&self, offset: u32, value: u32) {
        assert!((offset as usize + 4) <= self.size, "MMIO write out of bounds");
        unsafe {
            core::ptr::write_volatile(self.base.add(offset as usize) as *mut u32, value);
        }
    }

    /// Read-modify-write: set bits in `mask`.
    #[inline]
    pub fn set_bits(&self, offset: u32, mask: u32) {
        let v = self.read32(offset);
        self.write32(offset, v | mask);
    }

    /// Read-modify-write: clear bits in `mask`.
    #[inline]
    pub fn clear_bits(&self, offset: u32, mask: u32) {
        let v = self.read32(offset);
        self.write32(offset, v & !mask);
    }

    /// Poll a register until `(read32(offset) & mask) == expected`,
    /// with a maximum of `timeout_us` microseconds.
    pub fn poll(&self, offset: u32, mask: u32, expected: u32, timeout_us: u32) -> NvResult<u32> {
        let mut remaining = timeout_us;
        loop {
            let val = self.read32(offset);
            if (val & mask) == expected {
                return Ok(val);
            }
            if remaining == 0 {
                return Err(NvError::Timeout);
            }
            // Caller must provide stall function via Platform trait
            // For now, simple busy-loop decrement
            remaining = remaining.saturating_sub(1);
        }
    }

    pub fn size(&self) -> usize { self.size }
}

// ── PCI Configuration Space ─────────────────────────────────────────────────
// Mirrors HAL.dll: HalGetBusDataByOffset / HalSetBusDataByOffset

/// PCI device location on the bus.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PciAddress {
    pub bus: u8,
    pub device: u8,
    pub function: u8,
}

impl PciAddress {
    /// Build the PCI config address for I/O port 0xCF8.
    pub const fn config_addr(self, offset: u8) -> u32 {
        0x8000_0000
            | ((self.bus as u32) << 16)
            | ((self.device as u32) << 11)
            | ((self.function as u32) << 8)
            | ((offset as u32) & 0xFC)
    }
}

/// Trait that the OS kernel must implement to provide PCI access.
/// This replaces the 4 HAL.dll functions that nvlddmkm.sys imports.
pub trait Platform {
    /// Read 32 bits from PCI config space (HalGetBusDataByOffset).
    fn pci_config_read32(&self, addr: PciAddress, offset: u8) -> u32;

    /// Write 32 bits to PCI config space (HalSetBusDataByOffset).
    fn pci_config_write32(&self, addr: PciAddress, offset: u8, value: u32);

    /// Map physical MMIO into virtual address space (MmMapIoSpace).
    /// Returns a raw pointer to the mapped region.
    fn map_mmio(&self, phys_addr: u64, size: usize) -> *mut u8;

    /// Unmap a previously mapped MMIO region (MmUnmapIoSpace).
    fn unmap_mmio(&self, virt_addr: *mut u8, size: usize);

    /// Allocate a physically contiguous DMA buffer (MmAllocateContiguousMemory).
    /// Returns (virtual_address, physical_address).
    fn alloc_dma(&self, size: usize) -> Option<DmaBuffer>;

    /// Free a DMA buffer.
    fn free_dma(&self, buf: DmaBuffer);

    /// Busy-wait for `us` microseconds (KeStallExecutionProcessor).
    fn stall_us(&self, us: u32);
}

// ── DMA Buffer ──────────────────────────────────────────────────────────────

/// A physically contiguous DMA buffer.
/// NVIDIA's nvlddmkm.sys uses MmAllocateContiguousMemory for these.
pub struct DmaBuffer {
    pub virt: *mut u8,
    pub phys: u64,
    pub size: usize,
}

impl DmaBuffer {
    /// Write a slice into the DMA buffer at `offset`.
    pub fn write(&self, offset: usize, data: &[u8]) {
        assert!(offset + data.len() <= self.size, "DMA write out of bounds");
        unsafe {
            core::ptr::copy_nonoverlapping(data.as_ptr(), self.virt.add(offset), data.len());
        }
    }

    /// Write a u32 at `offset` (4-byte aligned).
    pub fn write32(&self, offset: usize, value: u32) {
        assert!(offset + 4 <= self.size && offset % 4 == 0);
        unsafe {
            core::ptr::write_volatile(self.virt.add(offset) as *mut u32, value);
        }
    }

    /// Read a u32 at `offset` (4-byte aligned).
    pub fn read32(&self, offset: usize) -> u32 {
        assert!(offset + 4 <= self.size && offset % 4 == 0);
        unsafe {
            core::ptr::read_volatile(self.virt.add(offset) as *const u32)
        }
    }
}

// ── PCI Enumeration ─────────────────────────────────────────────────────────

/// Scan the PCI bus and find the NVIDIA RTX 3060 12G (GA106).
pub fn find_gpu(platform: &dyn Platform) -> Option<PciAddress> {
    for bus in 0..=255u8 {
        for device in 0..32u8 {
            for function in 0..8u8 {
                let addr = PciAddress { bus, device, function };
                let vendor_device = platform.pci_config_read32(addr, 0x00);
                let vendor = (vendor_device & 0xFFFF) as u16;
                let dev_id = ((vendor_device >> 16) & 0xFFFF) as u16;
                if vendor == NVIDIA_VENDOR_ID && dev_id == GA106_DEVICE_ID {
                    return Some(addr);
                }
                // If vendor is 0xFFFF, no device — skip remaining functions
                if vendor == 0xFFFF {
                    break;
                }
            }
        }
    }
    None
}

/// Read BAR0 base address from PCI config space.
pub fn read_bar0(platform: &dyn Platform, addr: PciAddress) -> u64 {
    let bar0_lo = platform.pci_config_read32(addr, 0x10);
    let is_64bit = (bar0_lo & 0x06) == 0x04;
    let base_lo = (bar0_lo & 0xFFFF_FFF0) as u64;
    if is_64bit {
        let bar0_hi = platform.pci_config_read32(addr, 0x14);
        base_lo | ((bar0_hi as u64) << 32)
    } else {
        base_lo
    }
}

/// Read BAR1 base address (VRAM aperture) from PCI config space.
pub fn read_bar1(platform: &dyn Platform, addr: PciAddress) -> u64 {
    // BAR1 is at offset 0x18 (or 0x18/0x1C for 64-bit)
    let bar1_lo = platform.pci_config_read32(addr, 0x18);
    let is_64bit = (bar1_lo & 0x06) == 0x04;
    let base_lo = (bar1_lo & 0xFFFF_FFF0) as u64;
    if is_64bit {
        let bar1_hi = platform.pci_config_read32(addr, 0x1C);
        base_lo | ((bar1_hi as u64) << 32)
    } else {
        base_lo
    }
}

/// Set GPU to D0 power state (full power).
/// Required before accessing GPU registers on cold boot.
/// Walks PCI capability list to find PM capability (ID=0x01).
pub fn set_power_d0(platform: &dyn Platform, addr: PciAddress) {
    // Check if device has capabilities (bit 4 of status register)
    let status = platform.pci_config_read32(addr, 0x04) >> 16;
    if status & (1 << 4) == 0 {
        return; // No capabilities list
    }

    // Read capabilities pointer (offset 0x34, bottom byte)
    let cap_ptr = (platform.pci_config_read32(addr, 0x34) & 0xFF) as u8;
    let mut offset = cap_ptr;

    // Walk capability list to find PM capability (ID = 0x01)
    let mut iterations = 0;
    while offset != 0 && iterations < 48 {
        let cap_header = platform.pci_config_read32(addr, offset);
        let cap_id = (cap_header & 0xFF) as u8;

        if cap_id == 0x01 {
            // Found Power Management capability
            // PM Control/Status register is at offset + 4
            let pmcsr_off = offset + 4;
            let pmcsr = platform.pci_config_read32(addr, pmcsr_off);

            // Current power state is bits [1:0]
            let current_state = pmcsr & 0x03;
            if current_state != 0 {
                // Not in D0 — transition to D0
                let new_pmcsr = (pmcsr & !0x03) | 0x00; // Clear power state bits = D0
                platform.pci_config_write32(addr, pmcsr_off, new_pmcsr);

                // Wait for power transition (PCI spec: 10ms from D3→D0)
                platform.stall_us(20_000); // 20ms to be safe
            }
            return;
        }

        // Next capability (upper byte of cap_header bits [15:8])
        offset = ((cap_header >> 8) & 0xFF) as u8;
        iterations += 1;
    }
}

/// Enable bus mastering (required for DMA).
pub fn enable_bus_master(platform: &dyn Platform, addr: PciAddress) {
    let cmd = platform.pci_config_read32(addr, 0x04);
    // Set bit 2 (Bus Master Enable) and bit 1 (Memory Space Enable)
    platform.pci_config_write32(addr, 0x04, cmd | 0x06);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pci_address_config() {
        let addr = PciAddress { bus: 1, device: 0, function: 0 };
        let cfg = addr.config_addr(0);
        assert_eq!(cfg & 0x8000_0000, 0x8000_0000); // Enable bit
        assert_eq!((cfg >> 16) & 0xFF, 1);           // Bus 1
    }
}
