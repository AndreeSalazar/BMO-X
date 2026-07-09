use crate::hal;

pub fn set_syscall_kernel_stack(top: u64) {
    if let Some(h) = unsafe { hal::HAL.as_ref() } { (h.set_syscall_kernel_stack)(top); }
}
