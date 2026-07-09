use crate::hal;

pub fn main() -> ! {
    if let Some(h) = unsafe { hal::HAL.as_ref() } { (h.profile_main)(); }
    loop { unsafe { core::arch::asm!("pause") } }
}
