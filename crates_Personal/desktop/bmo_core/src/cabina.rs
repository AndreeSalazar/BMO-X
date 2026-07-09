use crate::hal;

pub fn init() {
    if let Some(h) = unsafe { hal::HAL.as_ref() } { (h.cabina_init)(); }
}
pub fn boot_ready() {
    if let Some(h) = unsafe { hal::HAL.as_ref() } { (h.cabina_boot_ready)(); }
}
pub fn info(tag: &str, msg: &str) {
    if let Some(h) = unsafe { hal::HAL.as_ref() } { (h.cabina_info)(tag, msg); }
}
pub fn fault(tag: &str, msg: &str) {
    if let Some(h) = unsafe { hal::HAL.as_ref() } { (h.cabina_fault)(tag, msg); }
}
pub fn warn(tag: &str, msg: &str) {
    if let Some(h) = unsafe { hal::HAL.as_ref() } { (h.cabina_warn)(tag, msg); }
}
pub fn trace(tag: &str, msg: &str) {
    if let Some(h) = unsafe { hal::HAL.as_ref() } { (h.cabina_trace)(tag, msg); }
}
pub fn panic_msg(tag: &str, msg: &str) {
    if let Some(h) = unsafe { hal::HAL.as_ref() } { (h.cabina_panic_msg)(tag, msg); }
}
pub fn info_u64(tag: &str, label: &str, val: u64) {
    if let Some(h) = unsafe { hal::HAL.as_ref() } { (h.cabina_info_u64)(tag, label, val); }
}
pub fn warn_u64(tag: &str, label: &str, val: u64) {
    if let Some(h) = unsafe { hal::HAL.as_ref() } { (h.cabina_warn_u64)(tag, label, val); }
}
pub fn fault_u64(tag: &str, label: &str, val: u64) {
    if let Some(h) = unsafe { hal::HAL.as_ref() } { (h.cabina_fault_u64)(tag, label, val); }
}
pub fn trace_u64(tag: &str, label: &str, val: u64) {
    if let Some(h) = unsafe { hal::HAL.as_ref() } { (h.cabina_trace_u64)(tag, label, val); }
}
pub fn is_overlay_enabled() -> bool {
    if let Some(h) = unsafe { hal::HAL.as_ref() } { (h.cabina_is_overlay_enabled)() } else { false }
}
pub fn set_overlay_enabled(v: bool) {
    if let Some(h) = unsafe { hal::HAL.as_ref() } { (h.cabina_set_overlay_enabled)(v); }
}
pub fn cycle_tab() {
    if let Some(h) = unsafe { hal::HAL.as_ref() } { (h.cabina_cycle_tab)(); }
}
pub fn cycle_query() -> bool {
    if let Some(h) = unsafe { hal::HAL.as_ref() } { (h.cabina_cycle_query)() } else { false }
}
pub fn paint_overlay() {
    if let Some(h) = unsafe { hal::HAL.as_ref() } { (h.cabina_paint_overlay)(); }
}
