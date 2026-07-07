//! Ring 3 — Transición y entry points de CPL=3.
//!
//! Contiene el primitivo compartido `ring3_transition()` (iretq) y el
//! desktop entry point que reemplaza el viejo demo de bordes morados.
//! El app loader (`bmo_core::bef::parsers::run_entry_point`) también
//! usa `ring3_transition()` — no hay duplicación del iretq.

pub mod transition;
pub mod demo_entry;
pub mod desktop;
