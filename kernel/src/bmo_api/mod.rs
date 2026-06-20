//! BMO API — window manager for FastOS-BMO.
//!
//! Inspired by win32 / WinUI / X11 / Carbon. Provides a minimal
//! "desktop shell" with:
//!
//!   - `window`   : Window struct + paint + hit-test
//!   - `manager`  : Window manager (Z-order, focus, message routing)
//!   - `message`  : Message types (PAINT, CLOSE, KEY, MOUSE)
//!   - `widget`   : Pre-built widgets (label, button, panel)
//!   - `desktop`  : The desktop loop (paint all, route input, sleep)
//!
//! v1.6.22: this is the foundation. Real wm features (drag, resize,
//! double-click, focus traversal) come in v1.7.x once the loop
//! is verified on hardware.

#![allow(dead_code)]

pub mod message;
pub mod widget;
pub mod window;
pub mod manager;
pub mod desktop;

pub use message::{Message, MessageKind};
pub use window::{Window, WindowFlags, Rect};
pub use manager::WindowManager;
pub use desktop::run_desktop;
