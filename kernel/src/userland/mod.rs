//! `userland/` — Aplicaciones de Ring 3 (futuro).
//!
//! v1.8.8: stub. Esta capa NO existe todavía — es donde correrán las
//! apps de usuario en el futuro (juegos, herramientas, tests).
//!
//! ## Relación con BMO CORE
//!
//! En la arquitectura Opus, BMO CORE es el "kernel del Ring 3":
//! - Recibe control de RING 0 al final del boot (después de phase 4).
//! - Inicializa la windowing API, el desktop, el FS.
//! - Cuando una app de userland está lista, BMO CORE le transfiere
//!   control vía `iretq` o `sysret` a un proceso de Ring 3.
//! - Las apps usan los syscalls de BMO API (0x100..0x1FF) a través de
//!   `bmo_core::desktop3` (la cúpula).
//!
//! ## Estado actual
//!
//! v1.8.8: hay un `userland_impl.rs` mínimo en este directorio. La fase
//! completa de Ring 3 con BMO CORE handoff queda para sesiones futuras.
//!
//! Ver `Rutas.md` §7 (USERLAND) para la arquitectura ideal.

#![allow(dead_code)]

#[path = "ring_3.rs"]
pub mod userland_impl;
