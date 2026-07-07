use crate::hal;

#[derive(Clone, Copy)]
pub struct Color(pub u32);

pub fn backbuffer_ptr() -> *mut u32 {
    if let Some(h) = unsafe { hal::HAL.as_ref() } { (h.backbuffer_ptr)() } else { core::ptr::null_mut() }
}
pub fn backbuffer_stride() -> usize {
    if let Some(h) = unsafe { hal::HAL.as_ref() } { (h.backbuffer_stride)() } else { 0 }
}
pub fn present() {
    if let Some(h) = unsafe { hal::HAL.as_ref() } { (h.framebuffer_present)(); }
}
pub fn put_pixel(x: u32, y: u32, color: Color) {
    if let Some(h) = unsafe { hal::HAL.as_ref() } { (h.framebuffer_put_pixel)(x, y, color.0); }
}
