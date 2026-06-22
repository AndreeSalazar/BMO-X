//! `AMD/zen3/` — Módulos Rust que implementan la lógica del CPU
//! documentada en `ryzen_5_5600x.md`.
//!
//! Cada archivo aquí corresponde a una sección del documento técnico.
//! RING 0 los invoca para obtener funcionalidad real (no stubs) sobre
//! el Ryzen 5 5600X (Vermeer, Zen 3, Family 19h Model 01h).
//!
//! ## Mapeo documento → código
//!
//! | Sección del documento AMD/ryzen_5_5600x.md  | Módulo Rust                |
//! |--------------------------------------------|----------------------------|
//! | §1 Identificación del CPU                   | `cpuid_detection.rs`       |
//! | §2 Topología física y SMT                  | `topology.rs`              |
//! | §3 Microarquitectura Zen 3                 | `cache_topology.rs`        |
//! | §4 CPUID leaves importantes                | `cpuid_detection.rs`       |
//! | §5 Ordenamiento de memoria (TSO débil)     | `memory_ordering.rs`       |
//! | §6 Cache, TLB y coherencia                 | `cache_topology.rs`        |
//! | §7 Paging y memoria virtual                | (en `memory_management/`)  |
//! | §8 Excepciones e IDT                       | (en `arch/`)               |
//! | §9 Local APIC                              | (en `arch/local_apic.rs`)  |
//! | §10 MSRs fundamentales                     | `msr_definitions.rs`       |
//! | §11 SYSCALL / SYSRET (ABI AMD64)           | (en `arch/`)               |
//! | §12 TSC y timers                           | `tsc_calibration.rs`       |
//! | §13 P-states, C-states y boost             | `power_management.rs`      |
//! | §14 MTRR y PAT                             | `mtrr_pat.rs`              |
//! | §15 Erratas relevantes                     | `errata_workarounds.rs`    |
//! | §16 Zen 3 vs Zen 2 vs Zen 4                | `model_comparison.rs`      |
//!
//! ## Estado de implementación
//!
//! Algunos módulos están completos y se usan desde RING 0; otros son
//! referencia para implementaciones futuras. Ver cada archivo para
//! su estado (`✅ completo`, `🚧 WIP`, `📋 stub`).
//!
//! ## Conexión con RING 0
//!
//! Los stubs actuales en `dev/acpi.rs`, `cpu/features.rs`, `cpu/tsc.rs`,
//! `cpu/cache.rs` deben migrar a invocar este módulo. Por ahora
//! coexisten (los stubs siguen siendo lo que RING 0 llama, y este
//! módulo expone la API "real" que se conectará cuando se elimine
//! el `#![allow(dead_code)]`).

#![allow(dead_code)] // v1.8.7: módulo aún no conectado al HAL

pub mod cpuid_detection;
pub mod topology;
pub mod cache_topology;
pub mod memory_ordering;
pub mod msr_definitions;
pub mod tsc_calibration;
pub mod power_management;
pub mod mtrr_pat;
pub mod errata_workarounds;
pub mod model_comparison;
pub mod acpi_real;
pub mod msr_init;
pub mod fastos_cpu;

// Re-exports convenientes (lo que el HAL realmente invoca)
pub use cpuid_detection::{
    detect_cpu, CpuVendor, CpuFamilyModel, CpuIdentity, CpuBrandString,
};
pub use topology::{Topology, CpuId, PerCpu, smp_init as topology_smp_init};
pub use tsc_calibration::{calibrate_tsc, TscSource};
pub use mtrr_pat::{init_mtrr, init_pat};
pub use acpi_real::{find_rsdp, parse_rsdp, parse_xsdt, parse_mcfg, RsdpHeader, McfgHeader, AcpiError, pm_timer_port};
pub use errata_workarounds::{apply_spectre_v2_mitigations, apply_spectre_v4_mitigations, apply_mds_mitigations, issue_ibpb};
pub use fastos_cpu::{init_fastos_cpu, init_msrs, init_acpi, identity, topology, cache, tsc_freq_hz, tsc_source, is_initialized, summary};
