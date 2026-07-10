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
pub fn load_bmo_core(hal: &HalServices, boot_info: *const BootInfo) -> ! {
    let bi = unsafe { &*boot_info };
    crate::dev::console::serial_write("[mod_loader] modules loaded: ");
    crate::dev::console::serial_write_u64(bi.module_count as u64, 10);
    crate::dev::console::serial_write("\n");

    // Log all modules
    for i in 0..bi.module_count as usize {
        let m = &bi.modules[i];
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
    if bi.module_count > 0 {
        let m = &bi.modules[0];
        if m.entry_point != 0 {
            let entry_fn: extern "C" fn(*const HalServices) -> ! =
                unsafe { core::mem::transmute(m.entry_point) };
            entry_fn(hal as *const _);
        }
    }

    // ── Diagnostic: no module loaded ──
    // The previous version simply halted here, which left the user
    // staring at a blank screen with no clue. Log a detailed diagnostic
    // and try to draw something visible on the framebuffer before halt.
    crate::dev::console::serial_write("\n[mod_loader] FATAL: no module with valid entry point\n");
    crate::dev::console::serial_write("                bi.module_count = ");
    crate::dev::console::serial_write_u64(bi.module_count as u64, 10);
    crate::dev::console::serial_write("\n");
    if bi.module_count > 0 {
        let m = &bi.modules[0];
        crate::dev::console::serial_write("                module[0].base        = 0x");
        crate::dev::console::serial_write_u64(m.base, 16);
        crate::dev::console::serial_write("\n                module[0].entry_point = 0x");
        crate::dev::console::serial_write_u64(m.entry_point, 16);
        crate::dev::console::serial_write("\n");
    }
    crate::dev::console::serial_write("                Check that the bootloader copied the .elf modules\n");
    crate::dev::console::serial_write("                and registered them in BootInfo.modules[].\n");

    // Try to paint a fault screen so the user is not staring at black
    let (fb, w, h, s) = unsafe { (crate::info::FB_ADDR, crate::info::FB_WIDTH, crate::info::FB_HEIGHT, crate::info::FB_STRIDE) };
    let w = w as usize;
    let h = h as usize;
    let s = s as usize;
    if fb != 0 && w > 0 && h > 0 && s > 0 {
        unsafe {
            let buf = fb as *mut u32;
            for y in 0..h {
                for x in 0..w {
                    let color = if y < 8 { 0xFFFFD000 } else { 0xFF220000 };
                    buf.add(y * s + x).write_volatile(color);
                }
            }
        }
    }

    loop { unsafe { core::arch::asm!("hlt"); } }
}
