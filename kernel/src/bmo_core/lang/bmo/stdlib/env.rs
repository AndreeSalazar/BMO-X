//! BMO std::env — Entorno y argumentos.

#![allow(dead_code)]

pub fn args() -> &'static [&'static str] { &[] }
pub fn get_env(_name: &str) -> Option<&'static str> { None }
pub fn set_env(_name: &str, _value: &str) -> bool { false }
pub fn current_dir() -> &'static str { "/" }
pub fn set_dir(_path: &str) -> bool { false }
