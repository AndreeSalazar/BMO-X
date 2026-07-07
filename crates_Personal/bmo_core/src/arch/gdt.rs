use crate::hal;

pub const KERNEL_CS: u64 = 0x08;
pub const KERNEL_DS: u64 = 0x10;
pub const USER_CS: u64  = 0x1B;
pub const USER_DS: u64  = 0x23;

pub fn set_kernel_stack(top: u64) {
    if let Some(h) = unsafe { hal::HAL.as_ref() } { (h.set_kernel_stack)(top); }
}
