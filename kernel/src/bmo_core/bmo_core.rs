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
use super::gustos;
use super::ring3_gateway;
use crate::bmo_gpu;

/// Inicializa todos los subsistemas de BMO Core.
///
/// Esta función **retorna** y debe llamarse una sola vez al boot,
/// después de las fases p0..p4 de Ring 0.
pub fn init() {
    // ── Trilogía: los 3 mosqueteros (hermanos, no bmo_core) ────────
    // Cabina  = El Ojo   (observación)
    // Defense = El Escudo (protección)
    // TimeBack= El Reloj (rollback)
    crate::cabina::init();
    crate::defense::init();
    crate::timeback::init();
    cabina_mark_ready();

    // 4) ui: framebuffer + 8x16 font. (stateless, no requiere init.)
    // 5) fs: FAT32 + BMO-FS + ramdisk.
    fs::init();

    // 6) GPU bridge: PE/ELF shims + BSF shaders.
    bmo_gpu::init();

    // 7) BEF loader: format + loaders (PE/ELF/native).
    bef::init();

    // 8) BMO API v2.0: 256 syscalls + WM + paint compositor.
    bmo_api::init();

    // 9) Desktop: state + dock. (welcome se arranca desde enter().)
    desktop::init();

    // 10) ring3_gateway: única puerta Ring 0 → BMO Core.
    ring3_gateway::init();

    // ── Tests integrados de la trilogía ────────────────────────────
    // Se ejecutan en cada boot para validar que los subsistemas
    // funcionan end-to-end. No fallan el boot (solo reportan).
    run_trilogy_tests();
    run_gateway_tests();

    crate::cabina::info("bmo_core", "BMO Core initialized: cabina+defense+timeback+bmo_api+desktop+ring3_gateway");
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

/// Ejecuta los tests del ring3_gateway.
fn run_gateway_tests() {
    use crate::cabina::{info, warn};
    for r in ring3_gateway::tests::run_all() {
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
    // Limpia el splash que dejó Ring 0.
    crate::boot::visual::clear();

    // Reproduce el logon sound (gustos).
    gustos::tracks::windows::logon();

    // Lanza el welcome. Esta función NO retorna.
    desktop::welcome::run();
}
