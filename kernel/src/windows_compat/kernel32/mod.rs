//! kernel32.dll compatibility — Process, Memory, Thread, File, Module, String, Time.
//!
//! Maps Win32 kernel32 functions to BMO syscalls and barex functions.

#![allow(dead_code)]

pub mod process;
pub mod memory;
pub mod thread;
pub mod file;
pub mod module;
pub mod string;
pub mod env;
pub mod time;
