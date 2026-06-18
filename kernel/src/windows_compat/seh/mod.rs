//! SEH/VEH exception handling compatibility.

#![allow(dead_code)]

/// Vectored exception handler chain.
static mut VEH_CHAIN: [u64; 16] = [0; 16];
static mut VEH_COUNT: u32 = 0;

/// AddVectoredExceptionHandler — register a VEH handler.
#[no_mangle]
pub extern "C" fn AddVectoredExceptionHandler(_first: u32, handler: u64) -> u64 {
    unsafe {
        if VEH_COUNT < 16 {
            VEH_CHAIN[VEH_COUNT as usize] = handler;
            VEH_COUNT += 1;
        }
    }
    1
}

/// RemoveVectoredExceptionHandler — unregister a VEH handler.
#[no_mangle]
pub extern "C" fn RemoveVectoredExceptionHandler(_handle: u64) -> u32 { 1 }

/// RtlAddFunctionTable — register function table for unwind.
#[no_mangle]
pub extern "C" fn RtlAddFunctionTable(_table: u64, _count: u32, _base: u64) -> u64 { 1 }

/// RtlDeleteFunctionTable — unregister function table.
#[no_mangle]
pub extern "C" fn RtlDeleteFunctionTable(_table: u64) -> u64 { 1 }
