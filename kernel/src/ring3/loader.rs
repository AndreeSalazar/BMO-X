//! Module Loader — Orquestador de módulos Ring 3.
//!
//! El bootloader UEFI carga todos los ELF de módulos en RAM antes de
//! ExitBootServices y los registra en BootInfo.modules[]. Este orquestador
//! solo arranca el primer módulo (mod_bmo_core). El desktop inicia el resto.
//!
//! La implementación real (BEF loader, parsing ELF) vive en crates_Personal/.

use bmo_boot_protocol::BootInfo;
use bmo_hal_defs::HalServices;

/// Arranca solo el primer módulo (mod_bmo_core).
/// Otros módulos están disponibles en BootInfo.modules[] para inicio diferido.
pub fn load_bmo_core(hal: &HalServices, boot_info: *const BootInfo) -> ! {
    let bi = unsafe { &*boot_info };
    crate::dev::console::serial_write("[ring3:loader] modules loaded: ");
    crate::dev::console::serial_write_u64(bi.module_count as u64, 10);
    crate::dev::console::serial_write("\n");

    for i in 0..bi.module_count as usize {
        let m = &bi.modules[i];
        if m.entry_point == 0 { continue; }
        crate::dev::console::serial_write("[ring3:loader]   [");
        crate::dev::console::serial_write_u64(i as u64, 10);
        crate::dev::console::serial_write("] at 0x");
        crate::dev::console::serial_write_u64(m.base, 16);
        crate::dev::console::serial_write(" entry=0x");
        crate::dev::console::serial_write_u64(m.entry_point, 16);
        crate::dev::console::serial_write("\n");
    }

    if bi.module_count > 0 {
        let m = &bi.modules[0];
        if m.entry_point != 0 {
            let entry_fn: extern "C" fn(*const HalServices) -> ! =
                unsafe { core::mem::transmute(m.entry_point) };
            entry_fn(hal as *const _);
        }
    }

    // ── Diagnóstico: ningún módulo cargado ──
    crate::dev::console::serial_write("\n[ring3:loader] FATAL: no module with valid entry point\n");
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

    // Pintar pantalla de error para que el usuario no vea pantalla negra
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
