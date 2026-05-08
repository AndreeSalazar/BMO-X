
use crate::abi::wddm_structs::*;
use core::ffi::c_void;
use core::ptr::null_mut;
use crate::loader::{GpuBootState};
use crate::pci::GA106_DEVICE;

pub const STATUS_SUCCESS: i32 = 0;
pub const STATUS_NOT_IMPLEMENTED: i32 = 0xC0000002u32 as i32;

// Stubs DDI
#[no_mangle]
pub unsafe extern "C" fn DxgkDdiAddDevice(_device_object: *mut c_void, arg: *mut DXGKARG_ADD_DEVICE) -> i32 {
    STATUS_SUCCESS
}

#[no_mangle]
pub unsafe extern "C" fn DxgkDdiStartDevice(_device_context: *mut c_void, _arg: *mut DXGKARG_START_DEVICE) -> i32 {
    STATUS_SUCCESS
}

#[no_mangle]
pub unsafe extern "C" fn DxgkDdiQueryAdapterInfo(_context: *mut c_void, _arg: *mut DXGKARG_QUERYADAPTERINFO) -> i32 {
    STATUS_SUCCESS
}

/// DxgkInitialize interceptado.
/// El driver (nvlddmkm) llama a esto pasando la estructura vacía y su tamaño, 
/// esperando que el kernel le rellene con los punteros WDDM.
pub unsafe fn DxgkInitialize(
    _driver_object: *mut c_void,
    _registry_path: *mut c_void,
    init_data: *mut DRIVER_INITIALIZATION_DATA
) -> Result<(), &'static str> {
    
    if GA106_DEVICE.state != GpuBootState::GspReady {
        return Err("Prerequisite GSP not ready for DxgkInitialize");
    }
    
    if init_data.is_null() {
        return Err("DRIVER_INITIALIZATION_DATA pointer is null");
    }

    // El contrato ABI exige llenar la tabla con nuestras 188 funciones DDI
    // Aquí inyectamos nuestros stubs y trampolines (Solo rellenamos los críticos de Fase 1)
    
    (*init_data).Version = 0x3000; // WDDM 3.0 (Windows 11)
    (*init_data).DxgkDdiAddDevice = Some(core::mem::transmute(DxgkDdiAddDevice as *const ()));
    (*init_data).DxgkDdiStartDevice = Some(core::mem::transmute(DxgkDdiStartDevice as *const ()));
    (*init_data).DxgkDdiQueryAdapterInfo = Some(core::mem::transmute(DxgkDdiQueryAdapterInfo as *const ()));
    
    // (Resto de las 188 se dejan como None/NULL por ahora, el driver solo llamará a AddDevice al inicio)

    GA106_DEVICE.state = GpuBootState::WddmReady;
    Ok(())
}
