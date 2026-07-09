use crate::hal;

pub unsafe fn ring3_transition(entry: u64, stack_top: u64) -> ! {
    if let Some(h) = unsafe { hal::HAL.as_ref() } { (h.ring3_transition)(entry, stack_top); }
    loop { core::arch::asm!("hlt"); }
}
