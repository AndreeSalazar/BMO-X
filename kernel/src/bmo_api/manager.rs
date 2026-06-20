//! BMO API — window manager.
//!
//! Owns the list of windows, tracks focus (the topmost window is
//! focused by default), and provides the message loop. The desktop
//! calls `paint_all()` to render the frame, and `dispatch(msg)` to
//! route a message to the right window.

use super::message::{Message, MessageKind};
use super::window::{Rect, Window, WindowFlags};

const MAX_WINDOWS: usize = 16;

pub struct WindowManager {
    pub windows: [Option<Window>; MAX_WINDOWS],
    pub count: usize,
    /// Index into `windows` of the currently focused window, or None.
    pub focused: Option<usize>,
    /// Z-order is the same as array index for v1.6.22; the manager
    /// doesn't sort. Bring-to-front is implemented by swapping with the
    /// last element.
    next_id: u32,
    /// True if the WM needs a full repaint (a window was created /
    /// moved / closed).
    pub dirty: bool,
}

impl WindowManager {
    pub const fn new() -> Self {
        const NONE: Option<Window> = None;
        Self {
            windows: [NONE; MAX_WINDOWS],
            count: 0,
            focused: None,
            next_id: 1,
            dirty: true,
        }
    }

    /// Create a window and return its ID. Returns None if at capacity.
    pub fn create(&mut self, title: &'static str, rect: Rect) -> Option<u32> {
        if self.count >= MAX_WINDOWS { return None; }
        let id = self.next_id;
        self.next_id += 1;
        let w = Window::new(id, title, rect);
        // Insert at the end (highest z-order).
        let slot = self.count;
        self.windows[slot] = Some(w);
        self.count += 1;
        self.focused = Some(slot);
        self.dirty = true;
        Some(id)
    }

    /// Close (destroy) the window with the given ID. Returns true on success.
    pub fn close(&mut self, id: u32) -> bool {
        for i in 0..self.count {
            if let Some(w) = &self.windows[i] {
                if w.id == id {
                    // Shift remaining windows down to keep the array dense.
                    for j in i..(self.count - 1) {
                        self.windows[j] = self.windows[j + 1].take();
                    }
                    self.windows[self.count - 1] = None;
                    self.count -= 1;
                    if let Some(f) = self.focused {
                        if f == i {
                            self.focused = if self.count == 0 { None } else { Some(self.count - 1) };
                        } else if f > i {
                            self.focused = Some(f - 1);
                        }
                    }
                    self.dirty = true;
                    return true;
                }
            }
        }
        false
    }

    /// Bring the window with the given ID to the front (top z-order).
    pub fn bring_to_front(&mut self, id: u32) {
        for i in 0..self.count {
            if let Some(w) = &self.windows[i] {
                if w.id == id && i + 1 < self.count {
                    let w = self.windows[i].take().unwrap();
                    self.windows[i] = self.windows[i + 1].take();
                    self.windows[i + 1] = Some(w);
                    self.focused = Some(i + 1);
                    self.dirty = true;
                    return;
                }
            }
        }
    }

    /// Mark all windows as dirty (forces a full repaint).
    pub fn invalidate_all(&mut self) {
        self.dirty = true;
        for slot in self.windows.iter_mut().flatten() {
            slot.dirty = true;
        }
    }

    /// Iterate windows back-to-front (highest z-order first) for painting.
    /// Calls `f` for each window in render order.
    pub fn for_each_top_down<F: FnMut(&Window, bool)>(&self, mut f: F) {
        for i in (0..self.count).rev() {
            if let Some(w) = &self.windows[i] {
                let focused = self.focused == Some(i);
                f(w, focused);
            }
        }
    }

    /// Find a window at the given screen point (front-to-back).
    pub fn window_at(&self, x: i32, y: i32) -> Option<u32> {
        for i in (0..self.count).rev() {
            if let Some(w) = &self.windows[i] {
                if w.flags.contains(WindowFlags::VISIBLE) && w.rect.contains(x, y) {
                    return Some(w.id);
                }
            }
        }
        None
    }
}
