
use core::ffi::c_void;

pub const STATUS_SUCCESS: i32 = 0;

#[no_mangle]
pub unsafe extern "C" fn D3DKMTEnumAdapters2(_arg: *mut c_void) -> i32 {
    STATUS_SUCCESS
}

#[no_mangle]
pub unsafe extern "C" fn D3DKMTEnumAdapters3(_arg: *mut c_void) -> i32 {
    STATUS_SUCCESS
}

#[no_mangle]
pub unsafe extern "C" fn D3DKMTQueryAdapterInfo(_arg: *mut c_void) -> i32 {
    STATUS_SUCCESS
}

#[no_mangle]
pub unsafe extern "C" fn D3DKMTCloseAdapter(_arg: *mut c_void) -> i32 {
    STATUS_SUCCESS
}
