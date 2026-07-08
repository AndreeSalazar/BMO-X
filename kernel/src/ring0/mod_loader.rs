//! Module Loader — starts pre-loaded Ring 3 modules.
//!
//! The UEFI bootloader loads all module ELFs into memory before ExitBootServices
//! and registers them in BootInfo.modules[]. This minimal loader just calls each
//! module's entry point with a HalServices pointer.

use bmo_boot_protocol::BootInfo;
use bmo_hal_defs::HalServices;

pub fn load_bmo_core(hal: &HalServices, boot_info: &BootInfo) -> ! {
    crate::dev::console::serial_write("[mod_loader] bootloader loaded modules: ");
    crate::dev::console::serial_write_u64(boot_info.module_count as u64, 10);
    crate::dev::console::serial_write("\n");

    for i in 0..boot_info.module_count as usize {
        let m = &boot_info.modules[i];
        if m.entry_point == 0 { continue; }

        crate::dev::console::serial_write("[mod_loader] module at 0x");
        crate::dev::console::serial_write_u64(m.base, 16);
        crate::dev::console::serial_write(" entry=0x");
        crate::dev::console::serial_write_u64(m.entry_point, 16);
        crate::dev::console::serial_write(" size=");
        crate::dev::console::serial_write_u64(m.size, 10);
        crate::dev::console::serial_write("\n");

        let entry_fn: extern "C" fn(*const HalServices) -> ! =
            unsafe { core::mem::transmute(m.entry_point) };
        entry_fn(hal as *const _);
    }

    // No module found — display boot message and halt
    crate::dev::console::serial_write("[mod_loader] no modules loaded, halting\n");
    loop { unsafe { core::arch::asm!("hlt"); } }
}
