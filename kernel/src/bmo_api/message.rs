//! BMO API — message types.
//!
//! Each `Message` describes something that happened to a window
//! (paint request, close, key press, mouse click). The window
//! manager dispatches messages to the focused window's
//! `wnd_proc` callback.

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum MessageKind {
    /// Repaint the window's client area.
    Paint,
    /// Window was closed (user clicked X or pressed ESC).
    Close,
    /// Key was pressed while the window had focus.
    KeyDown { scancode: u8, ascii: u8 },
    /// Mouse button pressed inside the window.
    MouseDown { x: i32, y: i32, button: u8 },
    /// Mouse moved over the window.
    MouseMove { x: i32, y: i32 },
    /// Window gained focus.
    FocusIn,
    /// Window lost focus.
    FocusOut,
    /// Periodic tick (for animations, clocks, etc.).
    Tick,
}

#[derive(Debug, Clone, Copy)]
pub struct Message {
    pub kind: MessageKind,
    /// Window this message targets (filled in by the manager).
    pub target_window_id: u32,
    /// Time the message was generated (TSC ticks since boot).
    pub timestamp: u64,
}

impl Message {
    pub const fn paint(target: u32) -> Self {
        Self { kind: MessageKind::Paint, target_window_id: target, timestamp: 0 }
    }
    pub const fn close(target: u32) -> Self {
        Self { kind: MessageKind::Close, target_window_id: target, timestamp: 0 }
    }
    pub const fn key(target: u32, scancode: u8, ascii: u8) -> Self {
        Self {
            kind: MessageKind::KeyDown { scancode, ascii },
            target_window_id: target,
            timestamp: 0,
        }
    }
    pub const fn tick(target: u32) -> Self {
        Self { kind: MessageKind::Tick, target_window_id: target, timestamp: 0 }
    }
}
