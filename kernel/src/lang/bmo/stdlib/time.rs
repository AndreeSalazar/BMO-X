//! BMO std::time — Reloj y temporización.

#![allow(dead_code)]

use crate::lang::bmo::runtime::time as rt;

pub fn now_ns() -> u64 { rt::now_ns() }
pub fn now_ms() -> u64 { rt::now_ms() }
pub fn now_secs() -> u64 { rt::now_secs() }
pub fn sleep_ms(ms: u64) { rt::sleep_ms(ms) }
pub fn sleep_secs(s: u64) { rt::sleep_secs(s) }
