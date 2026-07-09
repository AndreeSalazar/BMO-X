//! Module Loader — starts pre-loaded Ring 3 modules.
//!
//! The UEFI bootloader loads all module ELFs into memory before ExitBootServices
//! and registers them in BootInfo.modules[]. This loader only starts the first
//! module (mod_bmo_core). Remaining modules are started later by the desktop
//! via their entry points from BootInfo.

use bmo_boot_protocol::BootInfo;
use bmo_hal_defs::HalServices;

/// Start only the first module (mod_bmo_core).
/// Other modules are available via BootInfo.modules[] for deferred startup.
pub fn load_bmo_core(hal: &HalServices, boot_info: &BootInfo) -> ! {
    crate::dev::console::serial_write("[mod_loader] modules loaded: ");
    crate::dev::console::serial_write_u64(boot_info.module_count as u64, 10);
    crate::dev::console::serial_write("\n");

    // Log all modules
    for i in 0..boot_info.module_count as usize {
        let m = &boot_info.modules[i];
        if m.entry_point == 0 { continue; }
        crate::dev::console::serial_write("[mod_loader]   [");
        crate::dev::console::serial_write_u64(i as u64, 10);
        crate::dev::console::serial_write("] at 0x");
        crate::dev::console::serial_write_u64(m.base, 16);
        crate::dev::console::serial_write(" entry=0x");
        crate::dev::console::serial_write_u64(m.entry_point, 16);
        crate::dev::console::serial_write("\n");
    }

    // Start only module 0 (mod_bmo_core). The desktop starts the rest.
    if boot_info.module_count > 0 {
        let m = &boot_info.modules[0];
        if m.entry_point != 0 {
            let entry_fn: extern "C" fn(*const HalServices) -> ! =
                unsafe { core::mem::transmute(m.entry_point) };
            entry_fn(hal as *const _);
        }
    }

    crate::dev::console::serial_write("[mod_loader] no modules loaded, halting\n");
    loop { unsafe { core::arch::asm!("hlt"); } }
}
