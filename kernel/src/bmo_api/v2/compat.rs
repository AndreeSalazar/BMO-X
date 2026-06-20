//! v2.0 — Compatibility shim para el viejo `bmo_api` Ring 0.
//!
//! El viejo código (`bmo_api::{window,manager,message,widget}`) sigue
//! funcionando: el shim reescribe las llamadas a usar las tablas v2
//! pero conserva la API existente. En próximas versiones el shim se
//! elimina y el código viejo se migra directamente a v2.

#![allow(dead_code)]

/// Migra las ventanas del viejo `WindowManager` a la tabla v2. Útil
/// durante el periodo de transición.
pub fn migrate_legacy_windows() {
    {
        let s = super::state();
        s.lock();
        s.windows.init();
        s.surfaces.init();
        s.timers.init();
        s.unlock();
    }
    // Re-registra las clases built-in.
    super::class::register_builtin_classes();
    // Crea la desktop window.
    super::wm::create_desktop_window();
}
