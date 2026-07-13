//! Module Loader stub — modules are loaded directly from main.rs
//! using the HalServices entry point from BootContext stage_entry[].

pub fn load_bmo_core(hal: &bmo_hal::HalServices, entry: u64) -> ! {
    if entry != 0 {
        let entry_fn: extern "C" fn(*const bmo_hal::HalServices) -> ! =
            unsafe { core::mem::transmute(entry) };
        entry_fn(hal as *const _);
    }
    loop { unsafe { core::arch::asm!("hlt"); } }
}
