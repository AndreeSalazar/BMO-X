use crate::hal;

pub fn clear() {
    if let Some(hal) = unsafe { hal::HAL.as_ref() } { (hal.clear)(); }
}

pub fn print_at(x: u64, y: u64, color: u32, s: &str) {
    if let Some(hal) = unsafe { hal::HAL.as_ref() } { (hal.print_at)(x, y, color, s); }
}

pub fn print_at_u64(x: u64, y: u64, color: u32, val: u64) {
    if let Some(hal) = unsafe { hal::HAL.as_ref() } { (hal.print_at_u64)(x, y, color, val); }
}

pub fn fill_rect(x: u64, y: u64, w: u64, h: u64, color: u32) {
    if let Some(hal) = unsafe { hal::HAL.as_ref() } { (hal.fill_rect)(x, y, w, h, color); }
}

pub fn draw_image(data: *const u8, x: u64, y: u64, w: u64, h: u64) {
    if let Some(hal) = unsafe { hal::HAL.as_ref() } { (hal.draw_image)(data, x, y, w, h); }
}

pub fn draw_image_clip(data: *const u8, x: u64, y: u64, w: u64, h: u64, cx: u64, cy: u64, cw: u64, ch: u64) {
    if let Some(hal) = unsafe { hal::HAL.as_ref() } { (hal.draw_image_clip)(data, x, y, w, h, cx, cy, cw, ch); }
}
