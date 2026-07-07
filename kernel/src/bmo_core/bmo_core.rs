//! BMO/BMO v1.8.8
//!
//! Desarrolado por Salazar.
//!
//! v1.8.8 â€” BMO Core coordinator.
//!
//! Coordina la inicializaciÃ³n y entrada de todos los subsistemas de BMO Core.
//!
//! ## Orden de inicializaciÃ³n (v1.8.8)
//!
//! 1. **Cabina**    (cabina)         â€” El Ojo: log, telemetry, overlay
//! 2. **Defense**   (defense)        â€” El Escudo: ByteDefender (BEF scanner)
//! 3. **TimeBack**  (timeback)       â€” El Reloj: checkpoints + journal
//! 4. **UI**        (ui)             â€” framebuffer + 8x16 font
//! 5. **FS**        (fs)             â€” ramdisk + exFAT + BMO-FS
//! 6. **GPU**       (bmo_gpu)        â€” GPU bridge (RDNA4 skeleton)
//! 7. **BEF**       (bef)            â€” BEF format + loaders
//! 8. **BMO API**   (bmo_api)        â€” 256 syscalls + WM + paint
//! 9. **Desktop**   (desktop)        â€” welcome + render + dock
//!
//! ## Punto de entrada
//!
//! `init()` se llama desde `ring0::coordinator::main` despuÃ©s de las 5 fases
//! de boot (p0..p4). `enter()` se llama al final del coordinator y NO retorna.
//!
//! ## Hand-offs
//!
//! ```text
//! Ring 0 boot
//!     â”‚
//!     â–¼
//! ring0::coordinator::main
//!     â”‚
//!     â”œâ”€â–º vendor::amd::cpu::zen3::init_bmo_cpu
//!     â”œâ”€â–º boot::phases::run_all(p0..p4)
//!     â”œâ”€â–º bmo_core::init()              â† este archivo
//!     â””â”€â–º bmo_core::enter()             â† welcome + event loop (no return)
//! ```

use super::bmo_api;
use super::bef;
use super::desktop;
use super::fs;
use super::desktop3;
// use crate::bmo_gpu;  // TODO: re-enable when bmo_gpu module exists

/// Inicializa todos los subsistemas de BMO Core.
///
/// Esta funciÃ³n **retorna** y debe llamarse una sola vez al boot,
/// despuÃ©s de las fases p0..p4 de Ring 0.
pub fn init() {
    crate::dev::console::serial_write("[bmo_core] init: starting\n");
    // â”€â”€ TrilogÃ­a: los 3 mosqueteros (hermanos, no bmo_core) â”€â”€â”€â”€â”€â”€â”€â”€
    // Cabina  = El Ojo   (observaciÃ³n)
    // Defense = El Escudo (protecciÃ³n)
    // TimeBack= El Reloj (rollback)
    crate::dev::console::serial_write("[bmo_core] init: cabina\n");
    crate::cabina::init();
    crate::dev::console::serial_write("[bmo_core] init: defensa\n");
    crate::defense::init();
    crate::dev::console::serial_write("[bmo_core] init: timeback\n");
    crate::timeback::init();
    crate::dev::console::serial_write("[bmo_core] init: mark ready\n");
    cabina_mark_ready();
    crate::dev::console::serial_write("[bmo_core] init: fs\n");

    // 4) ui: framebuffer + 8x16 font. (stateless, no requiere init.)
    // 5) fs: ramdisk + exFAT (data).
    fs::init();
    crate::dev::console::serial_write("[bmo_core] init: bmo_gpu\n");

    // 6) GPU bridge: ELF shims + BSF shaders.
    // TODO: bmo_gpu::init();  â€” re-enable when bmo_gpu exists
    crate::dev::console::serial_write("[bmo_core] init: bef\n");

    // 7) BEF loader: format + loaders (BEF native + ELF).
    bef::init();
    crate::dev::console::serial_write("[bmo_core] init: bmo_api\n");

    // 8) BMO API v2.0: 256 syscalls + WM + paint compositor.
    bmo_api::init();
    crate::dev::console::serial_write("[bmo_core] init: desktop\n");

    // 9) Desktop: state + dock. (welcome se arranca desde enter().)
    desktop::init();
    crate::dev::console::serial_write("[bmo_core] init: desktop3\n");

    // 10) desktop3: la cÃºpula encima de Ring 3 (Ãºnica puerta).
    desktop3::init();
    crate::dev::watchdog::pet_fch_watchdog();
    crate::dev::console::serial_write("[bmo_core] init: DONE\n");

    // â”€â”€ Tests integrados de la trilogÃ­a â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€
    // v1.8.8: tests DESHABILITADOS al boot por defecto.
    //
    // Los 38 tests (cabina+defense+timeback+desktop3+bef) son Ãºtiles
    // para validaciÃ³n, pero ejecutar `compile()` del BMO lang en cada
    // boot es demasiado costoso y puede colgarse.
    //
    // Para correr tests manualmente desde el welcome o el shell:
    //   `timeback::tests::run_all()` desde el welcome
    //   `cabina::tests::run_all()` desde el shell
    //
    // Si quieres re-habilitar los tests en boot, descomenta las 3
    // lÃ­neas siguientes. **Riesgo**: el BEF test compila programas
    // BMO reales, lo que requiere ~5 MB de heap y puede tardar 1-2s.
    //
    // run_trilogy_tests();
    // run_gateway_tests();
    // run_bef_tests();

    crate::cabina::info("bmo_core", "BMO Core initialized: cabina+defense+timeback+bmo_api+desktop+desktop3");
}

/// Cabina se considera "ready" solo despuÃ©s de init() (FB GOP OK).
fn cabina_mark_ready() {
    crate::cabina::boot_ready();
}

/// Punto de entrada principal de BMO Core. Llamado desde
/// `ring0::coordinator::main` al final del boot.
///
/// Esta funciÃ³n **NO retorna**. Arranca el welcome screen, espera
/// input, procesa comandos, y queda en el event loop de desktop.
pub fn enter(_ctx: &crate::context::BootContext, _t0: u64, _phase4_end: u64) -> ! {
    crate::dev::console::serial_write("[bmo_core] enter: START\n");
    // Stage 7: bmo_core::enter
    crate::phase_1_RING_0::write_crash_marker(7);
    crate::uefi_rt::write_boot_stage("bmo_enter");

    // Limpia el splash que dejÃ³ Ring 0.
    crate::dev::console::serial_write("[bmo_core] enter: clear splash\n");
    crate::visual::clear();
    crate::dev::console::serial_write("[bmo_core] enter: splash cleared\n");

    // Inicializar el crate bmo_audio con la frecuencia de TSC calibrada.
    bmo_audio::init(crate::cpu::tsc_per_sec());

    // Reproduce el logon sound (chime de bienvenida) ~1 second.
    crate::dev::console::serial_write("[bmo_core] enter: logon sound (bmo_audio)\n");
    bmo_audio::play_logon_chime();
    crate::dev::watchdog::pet_fch_watchdog();
    crate::dev::console::serial_write("[bmo_core] enter: logon done\n");

    // Lanza el welcome. Esta funciÃ³n NO retorna.
    crate::dev::console::serial_write("[bmo_core] enter: welcome::run\n");
    desktop::welcome::run();
}


