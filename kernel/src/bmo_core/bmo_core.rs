//! FastOS/BMO v1.8.8
//!
//! Desarrolado por Salazar.
//!
//! v1.8.8 — BMO Core coordinator.
//!
//! Coordina la inicialización y entrada de todos los subsistemas de BMO Core.
//!
//! ## Orden de inicialización (v1.8.8)
//!
//! 1. **Cabina**    (cabina)         — El Ojo: log, telemetry, overlay
//! 2. **Defense**   (defense)        — El Escudo: ByteDefender (BEF scanner)
//! 3. **TimeBack**  (timeback)       — El Reloj: checkpoints + journal
//! 4. **UI**        (ui)             — framebuffer + 8x16 font
//! 5. **FS**        (fs)             — FAT32 + ramdisk + BMO-FS
//! 6. **GPU**       (bmo_gpu)        — GPU bridge (RDNA4 skeleton)
//! 7. **BEF**       (bef)            — BEF format + loaders
//! 8. **BMO API**   (bmo_api)        — 256 syscalls + WM + paint
//! 9. **Desktop**   (desktop)        — welcome + render + dock
//!
//! ## Punto de entrada
//!
//! `init()` se llama desde `ring0::coordinator::main` después de las 5 fases
//! de boot (p0..p4). `enter()` se llama al final del coordinator y NO retorna.
//!
//! ## Hand-offs
//!
//! ```text
//! Ring 0 boot
//!     │
//!     ▼
//! ring0::coordinator::main
//!     │
//!     ├─► vendor::amd::cpu::zen3::init_fastos_cpu
//!     ├─► boot::phases::run_all(p0..p4)
//!     ├─► bmo_core::init()              ← este archivo
//!     └─► bmo_core::enter()             ← welcome + event loop (no return)
//! ```

use super::bmo_api;
use super::bef;
use super::desktop;
use super::fs;
use super::desktop3;
use crate::bmo_gpu;

/// Inicializa todos los subsistemas de BMO Core.
///
/// Esta función **retorna** y debe llamarse una sola vez al boot,
/// después de las fases p0..p4 de Ring 0.
pub fn init() {
    crate::dev::console::serial_write("[bmo_core] init: starting\n");
    // ── Trilogía: los 3 mosqueteros (hermanos, no bmo_core) ────────
    // Cabina  = El Ojo   (observación)
    // Defense = El Escudo (protección)
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
    // 5) fs: FAT32 + BMO-FS + ramdisk.
    fs::init();
    crate::dev::console::serial_write("[bmo_core] init: bmo_gpu\n");

    // 6) GPU bridge: PE/ELF shims + BSF shaders.
    bmo_gpu::init();
    crate::dev::console::serial_write("[bmo_core] init: bef\n");

    // 7) BEF loader: format + loaders (PE/ELF/native).
    bef::init();
    crate::dev::console::serial_write("[bmo_core] init: bmo_api\n");

    // 8) BMO API v2.0: 256 syscalls + WM + paint compositor.
    bmo_api::init();
    crate::dev::console::serial_write("[bmo_core] init: desktop\n");

    // 9) Desktop: state + dock. (welcome se arranca desde enter().)
    desktop::init();
    crate::dev::console::serial_write("[bmo_core] init: desktop3\n");

    // 10) desktop3: la cúpula encima de Ring 3 (única puerta).
    desktop3::init();
    crate::dev::console::serial_write("[bmo_core] init: DONE\n");

    // ── Tests integrados de la trilogía ────────────────────────────
    // v1.8.8: tests DESHABILITADOS al boot por defecto.
    //
    // Los 38 tests (cabina+defense+timeback+desktop3+bef) son útiles
    // para validación, pero ejecutar `compile()` del BMO lang en cada
    // boot es demasiado costoso y puede colgarse.
    //
    // Para correr tests manualmente desde el welcome o el shell:
    //   `timeback::tests::run_all()` desde el welcome
    //   `cabina::tests::run_all()` desde el shell
    //
    // Si quieres re-habilitar los tests en boot, descomenta las 3
    // líneas siguientes. **Riesgo**: el BEF test compila programas
    // BMO reales, lo que requiere ~5 MB de heap y puede tardar 1-2s.
    //
    // run_trilogy_tests();
    // run_gateway_tests();
    // run_bef_tests();

    crate::cabina::info("bmo_core", "BMO Core initialized: cabina+defense+timeback+bmo_api+desktop+desktop3");
}

/// Ejecuta los tests del BEF loader (3 formatos: BEF, PE, ELF).
fn run_bef_tests() {
    use crate::cabina::{info, warn};
    for r in crate::bmo_core::bef::loader::tests::run_all() {
        if r.passed {
            info("test.bef", &r.name);
        } else {
            warn("test.bef", &r.name);
            warn("test.bef", &r.message);
        }
    }
}

/// Ejecuta los tests integrados de cabina, defense y timeback.
/// Los resultados se emiten como eventos a la cabina.
fn run_trilogy_tests() {
    use crate::cabina::{info, warn, fault};

    // Cabina tests
    for r in crate::cabina::tests::run_all() {
        if r.passed {
            info("test", &r.name);
        } else {
            fault("test", &r.name);
            fault("test", &r.message);
        }
    }

    // Defense tests
    for r in crate::defense::tests::run_all() {
        if r.passed {
            info("test.def", &r.name);
        } else {
            fault("test.def", &r.name);
        }
    }

    // TimeBack tests
    for r in crate::timeback::tests::run_all() {
        if r.passed {
            info("test.tb", &r.name);
        } else {
            warn("test.tb", &r.name);
        }
    }
}

/// Cabina se considera "ready" solo después de init() (FB GOP OK).
fn cabina_mark_ready() {
    crate::cabina::boot_ready();
}

/// Ejecuta los tests del desktop3 (la cúpula).
fn run_gateway_tests() {
    use crate::cabina::{info, warn};
    for r in desktop3::tests::run_all() {
        if r.passed {
            info("test.gw", &r.name);
        } else {
            warn("test.gw", &r.name);
            warn("test.gw", &r.message);
        }
    }
}

/// Punto de entrada principal de BMO Core. Llamado desde
/// `ring0::coordinator::main` al final del boot.
///
/// Esta función **NO retorna**. Arranca el welcome screen, espera
/// input, procesa comandos, y queda en el event loop de desktop.
pub fn enter(_ctx: &crate::boot::BootContext, _t0: u64, _phase4_end: u64) -> ! {
    crate::dev::console::serial_write("[bmo_core] enter: START\n");
    // Limpia el splash que dejó Ring 0.
    crate::dev::console::serial_write("[bmo_core] enter: clear splash\n");
    crate::boot::visual::clear();
    crate::dev::console::serial_write("[bmo_core] enter: splash cleared\n");

    // Inicializar el crate bmo_audio con la frecuencia de TSC calibrada.
    bmo_audio::init(crate::cpu::tsc_per_sec());

    // Reproduce el logon sound (Windows 10/11 chime).
    crate::dev::console::serial_write("[bmo_core] enter: logon sound (bmo_audio)\n");
    bmo_audio::play_logon_chime();
    crate::dev::console::serial_write("[bmo_core] enter: logon done\n");

    // Lanza el welcome. Esta función NO retorna.
    crate::dev::console::serial_write("[bmo_core] enter: welcome::run\n");
    desktop::welcome::run();
}
