//! Ring 3 — Primitivo de transición a CPL=3.
//!
//! Contiene el único punto de salida a Ring 3: `transition::ring3_transition()` (iretq).
//! No hay lógica de app ni desktop aquí — eso vive en `kernel/src/userland/` y en
//! `crates_Personal/userland_ring3/`.
//!
//! ## Regla
//!
//! - `ring3/` = SOLO transición CPU (el iretq)
//! - `userland/` = bridge kernel → Ring 3 (loader, procesos, dispatch)
//! - `bmo_api/` = API que las apps consumen (syscalls, ventanas, dibujo)
//! - `userland_ring3/` = runtime que las apps linkean (malloc, syscall wrappers)

pub mod transition;
pub mod desktop;
