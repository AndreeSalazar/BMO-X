

use core::ffi::c_void;
use crate::scheduler::{create_context, destroy_context, submit_to_hw_queue};
use crate::allocations::create_allocation;
use crate::telemetry::log_ioctl;
use core::ptr::null;

pub const STATUS_SUCCESS: i32 = 0;
pub const STATUS_INVALID_PARAMETER: i32 = 0xC000000Du32 as i32;

// ==============================================================================
// ESTRUCTURAS D3DKMT MINIMAS (ABI EXACTA)
// ==============================================================================

#[repr(C)]
#[derive(Copy, Clone)]
pub struct D3DKMT_CREATEDEVICE {
    pub hAdapter: u32,
    pub Flags: u32,
    pub hDevice: u32, // [out]
    pub pCommandBuffer: *mut c_void,
    pub CommandBufferSize: u32,
    pub pAllocationList: *mut c_void,
    pub AllocationListSize: u32,
    pub pPatchLocationList: *mut c_void,
    pub PatchLocationListSize: u32,
}

#[repr(C)]
#[derive(Copy, Clone)]
pub struct D3DKMT_CREATECONTEXT {
    pub hDevice: u32,
    pub NodeOrdinal: u32,
    pub EngineAffinity: u32,
    pub Flags: u32,
    pub pPrivateDriverData: *mut c_void,
    pub PrivateDriverDataSize: u32,
    pub ClientHint: u32,
    pub hContext: u32, // [out]
    pub pCommandBuffer: *mut c_void,
    pub CommandBufferSize: u32,
    pub pAllocationList: *mut c_void,
    pub AllocationListSize: u32,
    pub pPatchLocationList: *mut c_void,
    pub PatchLocationListSize: u32,
    pub CommandBuffer: u64,
}

#[repr(C)]
#[derive(Copy, Clone)]
pub struct D3DKMT_CREATEALLOCATION {
    pub hDevice: u32,
    pub hResource: u32,
    pub hGlobalShare: u32,
    pub pPrivateRuntimeData: *const c_void,
    pub PrivateRuntimeDataSize: u32,
    pub pPrivateDriverData: *mut c_void,
    pub PrivateDriverDataSize: u32,
    pub NumAllocations: u32,
    pub pAllocationInfo: *mut D3DKMT_ALLOCATIONINFO,
}

#[repr(C)]
#[derive(Copy, Clone)]
pub struct D3DKMT_ALLOCATIONINFO {
    pub hAllocation: u32, // [out]
    pub pSystemMem: *const c_void,
    pub pPrivateDriverData: *mut c_void,
    pub PrivateDriverDataSize: u32,
    pub VidPnSourceId: u32,
    pub Flags: u32,
}

#[repr(C)]
#[derive(Copy, Clone)]
pub struct D3DKMT_SUBMITCOMMAND {
    pub hDevice: u32,
    pub hContext: u32,
    pub SubmitFlags: u32,
    pub CommandBuffer: u64,
    pub CommandLength: u32,
    pub BroadcastContextCount: u32,
    pub BroadcastContext: [u32; 64],
    pub pPrivateDriverData: *mut c_void,
    pub PrivateDriverDataSize: u32,
    pub NumPrimaries: u32,
    pub WrittenPrimaries: *mut u32,
    pub NumHistoryBuffers: u32,
    pub HistoryBufferArray: *mut u64,
}

#[repr(C)]
#[derive(Copy, Clone)]
pub struct D3DKMT_WAITFORSYNCHRONIZATIONOBJECT {
    pub hContext: u32,
    pub ObjectCount: u32,
    pub ObjectHandleArray: [u32; 32],
}

#[repr(C)]
#[derive(Copy, Clone)]
pub struct D3DKMT_DESTROYCONTEXT {
    pub hContext: u32,
}

// Códigos de IOCTL / Funciones D3DKMT ficticias para el dispatcher
pub const D3DKMT_CODE_CREATEDEVICE: u32 = 0x01;
pub const D3DKMT_CODE_CREATECONTEXT: u32 = 0x02;
pub const D3DKMT_CODE_CREATEALLOCATION: u32 = 0x03;
pub const D3DKMT_CODE_SUBMITCOMMAND: u32 = 0x04;
pub const D3DKMT_CODE_WAITFORSYNC: u32 = 0x05;
pub const D3DKMT_CODE_DESTROYCONTEXT: u32 = 0x06;

// ==============================================================================
// IOCTL DISPATCHER PRINCIPAL
// ==============================================================================

/// Central IOCTL Dispatcher for Usermode D3DKMT calls
#[no_mangle]
pub unsafe extern "C" fn FastGpuIoctlDispatcher(ioctl_code: u32, buffer: *mut c_void, _buffer_size: u32) -> i32 {
    if buffer.is_null() {
        return STATUS_INVALID_PARAMETER;
    }

    match ioctl_code {
        D3DKMT_CODE_CREATEDEVICE => {
            let data = &mut *(buffer as *mut D3DKMT_CREATEDEVICE);
            log_ioctl(D3DKMT_CODE_CREATEDEVICE, data.hAdapter, null());
            data.hDevice = 0x100;
            STATUS_SUCCESS
        },
        
        D3DKMT_CODE_CREATECONTEXT => {
            let data = &mut *(buffer as *mut D3DKMT_CREATECONTEXT);
            log_ioctl(D3DKMT_CODE_CREATECONTEXT, data.hDevice, null());
            if let Some(h_ctx) = create_context(data.NodeOrdinal, data.EngineAffinity, false) {
                data.hContext = h_ctx;
                STATUS_SUCCESS
            } else {
                STATUS_INVALID_PARAMETER
            }
        },
        
        D3DKMT_CODE_CREATEALLOCATION => {
            let data = &mut *(buffer as *mut D3DKMT_CREATEALLOCATION);
            log_ioctl(D3DKMT_CODE_CREATEALLOCATION, data.hDevice, null());
            if data.NumAllocations > 0 && !data.pAllocationInfo.is_null() {
                for i in 0..data.NumAllocations {
                    let info = &mut *data.pAllocationInfo.offset(i as isize);
                    // For dummy size, we just pass 4096. Real size is nested.
                    if let Some(h_alloc) = create_allocation(4096, true, 0) {
                        info.hAllocation = h_alloc;
                    }
                }
            }
            STATUS_SUCCESS
        },

        D3DKMT_CODE_SUBMITCOMMAND => {
            let data = &*(buffer as *const D3DKMT_SUBMITCOMMAND);
            log_ioctl(D3DKMT_CODE_SUBMITCOMMAND, data.hContext, data.CommandBuffer as *const u8);
            if submit_to_hw_queue(data.hContext, data.CommandBuffer, 1) {
                STATUS_SUCCESS
            } else {
                STATUS_INVALID_PARAMETER
            }
        },

        D3DKMT_CODE_WAITFORSYNC => {
            let data = &*(buffer as *const D3DKMT_WAITFORSYNCHRONIZATIONOBJECT);
            log_ioctl(D3DKMT_CODE_WAITFORSYNC, data.hContext, null());
            STATUS_SUCCESS
        },

        D3DKMT_CODE_DESTROYCONTEXT => {
            let data = &*(buffer as *const D3DKMT_DESTROYCONTEXT);
            log_ioctl(D3DKMT_CODE_DESTROYCONTEXT, data.hContext, null());
            destroy_context(data.hContext);
            STATUS_SUCCESS
        },

        _ => STATUS_INVALID_PARAMETER,
    }
}
