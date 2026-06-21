//! BMO std::proc — Gestión de procesos.

#![allow(dead_code)]

use crate::bmo_core::lang::bmo::runtime::proc as rt;

pub fn exit(code: i32) -> ! { rt::exit(code) }
pub fn yield_now() { rt::yield_now() }
pub fn pid() -> u32 { rt::current_pid().0 }
