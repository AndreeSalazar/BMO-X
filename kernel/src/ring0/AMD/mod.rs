//! `AMD/` — Documentación técnica y referencias del Ryzen 5 5600X.
//!
//! v1.8.8 Phase 2: el código real se movió a `vendor/amd/cpu/zen3/`.
//! Este directorio conserva solo la documentación (archivos `.md`).
//!
//! ## Archivos
//!
//! - `ryzen_5_5600x.md` (75 KB) — Documento principal: CPUID, MSRs, APIC,
//!   paging, TSC, P-states, MTRR/PAT, erratas, comparativa Zen 1-5.
//! - `errata.md` — Tabla de erratas con workarounds.
//! - `boot_sequence.md` — Diagrama del startup del kernel.
//! - `glossary.md` — Glosario de términos.
//! - `README.md` — Índice y política de uso.
//!
//! ## Código
//!
//! El código real está en:
//!
//! ```
//! kernel/src/ring0/vendor/amd/cpu/zen3/
//! ```
//!
//! Acceso via `crate::vendor::amd::cpu::zen3::*` o via el alias
//! `crate::vendor::amd::cpu::zen3::*` (mantenido por compatibilidad).
//!
//! Si ves `pub mod` aquí, es un bug — el código vive en `vendor/`.
