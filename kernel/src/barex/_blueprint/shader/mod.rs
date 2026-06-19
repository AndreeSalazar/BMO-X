//! `_blueprint::shader` — esqueletos de los backends de shader.
//!
//! v1.2.0: Solo contiene stubs. Los módulos reales (`bsf`, `loader`)
//! viven en `crate::barex::shader::*` y son los que se usan en
//! producción. Este directorio es la documentación de los backends
//! que existirán cuando haya Ring 3 + GPU.
//!
//! Cada sub-módulo retorna `BxError::NotImplemented` y solo describe
//! las firmas que tendrá la API final.

#![allow(dead_code)]

pub mod cache;
pub mod dxbc;
pub mod dxil;
pub mod ir;
pub mod native;
pub mod spirv;
pub mod stage;
