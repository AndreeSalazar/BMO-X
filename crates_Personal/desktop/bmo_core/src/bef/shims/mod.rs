//! `bmo_core::bef::shims` - Linux compat layer.
//!
//! v1.8.8: Linux syscall shims implementados (fs, mem, proc, sync, time).
//! Win32 shims eliminados - Linux es el unico target de compatibilidad.
//!
//! ## Roadmap
//!
//! v1.9: minimum viable (hello world ELF - 6 funciones).
//! v2.0: windowed app ELF.
//! v3.0: games (Vulkan/DXVK via Linux).

#![allow(dead_code)]

pub mod linux;
