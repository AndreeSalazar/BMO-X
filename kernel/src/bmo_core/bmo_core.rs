//! v1.7.4 — BMO Core coordinator.
//!
//! Coordina la inicialización de todos los subsistemas de BMO Core.
//! NO contiene lógica de aplicación — sólo:
//!   1. diag (logging + overlay)
//!   2. ui (framebuffer + font)
//!   3. lang (BMOasm + ÑEXO)
//!   4. fs (FAT32 + BMO-FS + ramdisk)
//!   5. sandbox (capabilities)
//!   6. barex (compat + shaders)
//!   7. bmo_api (windowing + 256 syscalls)
//!   8. desktop (welcome + render)
//!
//! Después de init_bmo_core(), `enter()` arranca el desktop welcome
//! y se queda en el event loop.
//!
//! Punto de entrada: llamado desde `ring0::ring_0::dispatch_phase5()`.
//! Esta función NO retorna.

use crate::bmo_gpu;
use super::bmo_api;
use super::bef;
use super::desktop;
use super::diag;
use super::fs;
use super::gustos;

/// Inicializa todos los subsistemas de BMO Core.
///
/// Llamar desde `ring0::ring_0::dispatch_phase5()` antes de `enter()`.
pub fn init() {
    // 1) diag: logging + overlay + telemetry. Sin esto no se ven
    //    mensajes en pantalla.
    diag::init();

    // 2) ui: framebuffer + 8x16 font. Las primitivas draw_text, fill_rect
    //    etc. están aquí.
    //    (No requiere init explícito — es stateless.)

    // 3) lang: BMOasm (compiler) + ÑEXO (CLI + runtime).
    //    (No requiere init explícito — son compilers.)

    // 4) fs: FAT32 + BMO-FS + ramdisk.
    fs::init();

    // 5) sandbox: capabilities.
    //    (No requiere init explícito — son bitflags.)

    // 6) barex: compat + shader loader.
    bmo_gpu::init();

    // 7) bmo_abi: handle, status, type descriptors.
    //    (Stateless.)

    // 8) bmo_api: windowing — 256 syscalls + WM + paint compositor.
    bmo_api::init();

    // 9) desktop: init state + dock. Welcome se arranca desde enter().
    desktop::init();
}

/// Punto de entrada principal de BMO Core. Llamado desde
/// `ring0::ring_0::dispatch_phase5()`.
///
/// Muestra la pantalla de bienvenida v1.7.1, espera input, procesa
/// comandos (Run, Hello, Nexo, Test, Reboot). No retorna.
pub fn enter(_ctx: &crate::boot::BootContext, _t0: u64, _phase4_end: u64) -> ! {
    // Limpia el splash que dejó Ring 0.
    crate::boot::visual::clear();

    // Reproduce el logon sound (gustos).
    gustos::tracks::windows::logon();

    // Inicializa el BEF loader por si vienen BEFs embebidos.
    bef::init();

    // Lanza el welcome. Esta función NO retorna.
    desktop::welcome::run();
}
