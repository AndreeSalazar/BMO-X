use crate::hal;

pub fn heap_alloc(size: usize, align: usize) -> *mut u8 {
    if let Some(h) = unsafe { hal::HAL.as_ref() } { (h.heap_alloc)(size, align) } else { core::ptr::null_mut() }
}

pub fn heap_free(ptr: *mut u8, size: usize, align: usize) {
    if let Some(h) = unsafe { hal::HAL.as_ref() } { (h.heap_free)(ptr, size, align); }
}
