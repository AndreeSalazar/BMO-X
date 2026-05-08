
use crate::pci::{probe_and_initialize_ga106, PciDeviceHeader};
use crate::sec2::bootstrap_sec2_falcon;
use crate::gsp::load_gsp_firmware;
use crate::boot::DxgkInitialize;
use crate::abi::wddm_structs::DRIVER_INITIALIZATION_DATA;
use core::ptr::null_mut;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GpuBootState {
    Uninitialized,
    PciMapped,
    BarsMapped,
    FalconReady,
    Sec2Ready,
    GspReady,
    WddmReady,
    UsermodeReady,
    Error(&'static str), // Nuevo estado para registrar fallos en la cadena
}

pub struct FastGpuDevice {
    pub pci_bar0: u64,
    pub pci_bar1: u64,
    pub vram_ptr: u64, // Virtual address for VRAM access
    pub vram_size: u64,
    pub state: GpuBootState,
}

impl FastGpuDevice {
    pub const fn new() -> Self {
        Self {
            pci_bar0: 0,
            pci_bar1: 0,
            vram_ptr: 0,
            vram_size: 12 * 1024 * 1024 * 1024,
            state: GpuBootState::Uninitialized,
        }
    }
}

/// Ejecuta toda la cadena de inicialización determinista para GA106.
pub unsafe fn bootstrap_fastgpu(bus: u8, slot: u8, func: u8, header: *const PciDeviceHeader, gsp_fw_addr: u64, gsp_fw_size: u64) -> Result<(), &'static str> {
    
    // 1. PCI & BAR Mapeo (Avanza a BarsMapped)
    if !probe_and_initialize_ga106(bus, slot, func, header) {
        crate::pci::GA106_DEVICE.state = GpuBootState::Error("Fallo en inicializacion PCI / Hardware no coincide");
        return Err("Fallo PCI");
    }
    
    // 2. SEC2 Bootstrap (Avanza a Sec2Ready)
    if !bootstrap_sec2_falcon() {
        crate::pci::GA106_DEVICE.state = GpuBootState::Error("Fallo en Handshake SEC2 (WPR2, Mailbox o RISC-V)");
        return Err("Fallo SEC2");
    }
    
    // 3. GSP Firmware Load (Avanza a GspReady)
    if let Err(e) = load_gsp_firmware(gsp_fw_addr, gsp_fw_size) {
        crate::pci::GA106_DEVICE.state = GpuBootState::Error(e);
        return Err(e);
    }
    
    // 4. WDDM Handshake DxgkInitialize (Avanza a WddmReady)
    // Inicializamos una estructura dummy en FastOS para que sirva de tabla DDI global
    static mut DDI_TABLE: DRIVER_INITIALIZATION_DATA = unsafe { core::mem::zeroed() };
    
    if let Err(e) = DxgkInitialize(null_mut(), null_mut(), &mut DDI_TABLE) {
        crate::pci::GA106_DEVICE.state = GpuBootState::Error(e);
        return Err(e);
    }

    // Fin exitoso
    Ok(())
}
