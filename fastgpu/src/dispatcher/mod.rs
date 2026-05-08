
use core::ffi::c_void;
use core::ptr::null_mut;
use crate::boot::{DxgkDdiAddDevice, DxgkDdiStartDevice, DxgkDdiQueryAdapterInfo};
use crate::abi::wddm_structs::*;

pub const STATUS_SUCCESS: i32 = 0;
pub const STATUS_NOT_IMPLEMENTED: i32 = 0xC0000002u32 as i32;

// Master Dispatcher mapping indices to WDDM DDI Stubs
pub unsafe fn ddi_dispatch(ddi_index: u32, arg1: *mut c_void, arg2: *mut c_void) -> i32 {
    // serial_print!("DDI Dispatch Call: Index {}\n", ddi_index);
    match ddi_index {
        0 => DxgkDdiAddDevice(arg1, arg2 as *mut *mut c_void),
        1 => DxgkDdiStartDevice(arg1, arg2 as *mut DXGK_START_INFO, null_mut(), null_mut(), null_mut()),
        16 => DxgkDdiQueryAdapterInfo(arg1, arg2 as *mut DXGKARG_QUERYADAPTERINFO),
        _ => {
            // serial_print!("Unhandled DDI: {}\n", ddi_index);
            STATUS_NOT_IMPLEMENTED
        }
    }
}
