//! Window manager / compositor.
//!
//! In the Ring 3 base, this is a stub. Future phases will:
//! - Track window handles per-process
//! - Composite via shared framebuffer region
//! - Send expose/move/resize events to client processes
//! - Drive the desktop shell

#![no_std]

pub fn init() {}
pub fn expose(_hwnd: u32) {}
pub fn move_window(_hwnd: u32, _x: i32, _y: i32) {}
