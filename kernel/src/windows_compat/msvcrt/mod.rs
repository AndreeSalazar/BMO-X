//! msvcrt.dll compatibility — C Runtime (malloc, printf, string, math).

#![allow(dead_code)]

pub mod memory;
pub mod string;
pub mod stdio;
pub mod stdlib;
pub mod math;
pub mod init;
