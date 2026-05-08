

use crate::loader::{FastGpuDevice, GpuBootState};
// use crate::serial_print;

pub const NVIDIA_VENDOR_ID: u16 = 0x10DE;
pub const GA106_DEVICE_ID: u16 = 0x2504;

/// Represents a raw PCI configuration space header
#[repr(C)]
#[derive(Debug, Copy, Clone)]
pub struct PciDeviceHeader {
    pub vendor_id: u16,
    pub device_id: u16,
    pub command: u16,
    pub status: u16,
    pub revision_id: u8,
    pub prog_if: u8,
    pub subclass: u8,
    pub class_code: u8,
    pub cache_line_size: u8,
    pub latency_timer: u8,
    pub header_type: u8,
    pub bist: u8,
    pub bar0: u32,
    pub bar1: u32,
    pub bar2: u32,
    pub bar3: u32,
    pub bar4: u32,
    pub bar5: u32,
    pub cardbus_cis_pointer: u32,
    pub subsystem_vendor_id: u16,
    pub subsystem_id: u16,
    pub expansion_rom_base_address: u32,
    pub capabilities_pointer: u8,
    pub reserved0: [u8; 7],
    pub interrupt_line: u8,
    pub interrupt_pin: u8,
    pub min_grant: u8,
    pub max_latency: u8,
}

/// Global instance of the GA106 Device State
pub static mut GA106_DEVICE: FastGpuDevice = FastGpuDevice::new();

/// FastOS Kernel calls this function when scanning the PCI bus.
/// It checks if the device is the RTX 3060 (GA106), maps its BARs, 
/// and transitions the FastGPU boot state.
pub unsafe fn probe_and_initialize_ga106(bus: u8, slot: u8, func: u8, header: *const PciDeviceHeader) -> bool {
    if header.is_null() {
        return false;
    }

    let vendor = (*header).vendor_id;
    let device = (*header).device_id;

    if vendor == NVIDIA_VENDOR_ID && device == GA106_DEVICE_ID {
        // serial_print!("FastOS PCI: NVIDIA RTX 3060 12GB (GA106) Detected at {:02x}:{:02x}.{}\n", bus, slot, func);
        
        // 1. Enable Memory Space, I/O Space, and Bus Mastering in PCI Command Register
        // (In a real kernel, you would use outl() to the PCI configuration port here)
        // pci_config_write_word(bus, slot, func, 0x04, (*header).command | 0x07);
        
        // 2. Extract BARs (Base Address Registers)
        // BAR0 contains the MMIO registers (16MB usually)
        let bar0_raw = (*header).bar0;
        let bar1_raw = (*header).bar1;
        
        // Clear lower bits to get actual 32-bit/64-bit physical addresses
        // (Assuming 64-bit BARs for modern GPUs, BAR0 + BAR1 might form a 64-bit address,
        // or BAR0 is MMIO and BAR1 is Framebuffer. Usually NVIDIA uses BAR0 for MMIO and BAR1 for VRAM mapping)
        let mmio_base = (bar0_raw & 0xFFFFFFF0) as u64; 
        let fb_base = (bar1_raw & 0xFFFFFFF0) as u64;

        // serial_print!("FastOS PCI: GA106 BAR0 (MMIO) mapped at 0x{:X}\n", mmio_base);
        // serial_print!("FastOS PCI: GA106 BAR1 (Framebuffer) mapped at 0x{:X}\n", fb_base);

        // 3. Update the Golden Device State
        GA106_DEVICE.pci_bar0 = mmio_base;
        GA106_DEVICE.pci_bar1 = fb_base;
        
        GA106_DEVICE.state = GpuBootState::BarsMapped;

        // serial_print!("FastOS PCI: GA106 State transitioned to BarsMapped. Ready for SEC2 Boot.\n");
        return true;
    }

    false
}
