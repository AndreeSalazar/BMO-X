//! BMO Sema — análisis semántico (type checking, scope, etc).
//!
//! v1.8.8: re-exporta el sema original. En la próxima fase se reescribirá
//! para operar sobre el common IR.

#![allow(dead_code)]

pub use crate::lang::bmo::sema::Sema;
