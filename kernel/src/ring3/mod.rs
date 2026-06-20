//! Ring 3 — Userland applications (work in progress).
//!
//! Ring 3 de x86-64 es donde corren las apps de usuario. En FastOS/BMO,
//! las apps Ring 3 aún no se cargan dinámicamente (eso es v2.1 del
//! BMO API spec). Lo que existe hoy son:
//!
//!   - Tests del path Ring 0→3→0 vía `ring0::arch::ring3_test` (en el kernel).
//!   - El crate externo `nexo_ring3/` con stubs de BSF loader y ABI.
//!   - El BMO API v2.0 (`bmo_core::bmo_api`) que define los 256 syscalls
//!     que las apps Ring 3 usarán cuando el loader dinámico esté listo.
//!
//! Submódulos (a medida que se implementen):
//!   apps/        — Apps de ejemplo (terminal, file manager, etc.)
//!   libbmo/      — Ring 3 library (en libbmo.a) con wrappers Rust para syscalls
//!   elf_loader/  — ELF loader para cargar apps en Ring 3 desde BMO-FS
//!
//! Estado actual: preparación estructural. La wnd_proc real a Ring 3
//! (syscall 0x198 BMO_DISPATCH_RETURN) está documentada en
//! `docs/BMO_API_V2_SPEC.md` §6.2 pero la implementación del
//! trampoline + per-thread kernel stack queda para v2.1.
//!
//! Contrato con BMO Core (ver ../bmo_core/mod.rs):
//!   - Ring 3 sólo accede a BMO Core vía syscalls 0x100..0x1FF.
//!   - Cada syscall valida el origen (Ring 3, no Ring 0) y el destino
//!     (handle válido con generation counter coincidente).

#![allow(dead_code)]
#![allow(static_mut_refs)]

// ── Coordinator (orquesta init + wnd_proc dispatch) ─────────────────
// El módulo `coord` apunta a `ring_3.rs` al lado de este archivo.
#[path = "ring_3.rs"]
pub mod coord;
