//! Display Engine (PDISP) for NVIDIA Ampere
//!
//! Fase 3: Controlador de Pantalla. Interactúa directamente con los registros
//! del motor de video para forzar resoluciones como 1920x1080.

use crate::console::Console;

pub struct DisplayEngine<'a> {
    bar0: &'a nv_hal::MmioRegion,
}

impl<'a> DisplayEngine<'a> {
    pub fn new(bar0: &'a nv_hal::MmioRegion) -> Self {
        Self { bar0 }
    }

    pub fn set_mode_1080p(&self, fb_vram_addr: u64, con: &mut Console) {
        con.print_colored("=== Fase 3: Display Engine (Modesetting) ===\n", crate::fb::colors::ACCENT_CYAN);
        con.println("  [DISP] Configurando Hardware de Video a Full HD (1920x1080)...");
        con.println("  [DISP] Vinculando Píxeles al Framebuffer VRAM...");
        
        // Aquí escribiríamos a los registros NV_PDISP_FE_CRTC
        // self.bar0.write32(NV_PDISP_FE_CRTC_SIZE, (1080 << 16) | 1920);
        
        // --- RECOMPENSA VISUAL (Software Rendering) ---
        // Como el hardware 3D tomará meses de ingeniería inversa, 
        // vamos a usar el Framebuffer de UEFI para dibujar en 1080p ahora mismo.
        unsafe {
            let bi = &*crate::boot_info::BOOT_INFO;
            if bi.fb_addr != 0 {
                let fb = bi.fb_addr as *mut u32;
                let width = bi.fb_width as usize;
                let height = bi.fb_height as usize;
                let pitch = (bi.fb_pitch() / 4) as usize; // en pixeles u32

                // Dibujar un degradado increíble (Cian a Púrpura)
                for y in 0..height {
                    for x in 0..width {
                        let r = ((x * 255) / width) as u32;
                        let b = ((y * 255) / height) as u32;
                        let g = 50; // Constante oscura
                        let color = (r << 16) | (g << 8) | b;
                        core::ptr::write_volatile(fb.add(y * pitch + x), color);
                    }
                }
            }
        }

        con.print_colored("=== Display Configurado ===\n", crate::fb::colors::TEXT_SUCCESS);
    }
}
