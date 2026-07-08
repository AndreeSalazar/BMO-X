//! Input event types and lock-free ring buffer queue.
//!
//! `InputEventQueue` is a single-producer, single-consumer (SPSC) lock-free
//! ring buffer. Safe for use from IRQ context (producer) + main loop (consumer).

use core::sync::atomic::{AtomicUsize, Ordering};
use core::cell::UnsafeCell;

/// Maximum events in the queue.
pub const QUEUE_CAPACITY: usize = 256;

/// Device-agnostic input event.
#[derive(Debug, Clone, Copy)]
#[repr(C)]
pub struct InputEvent {
    pub timestamp: u64,
    pub device_id: u16,
    pub kind: InputEventKind,
    pub _pad: u8,
    pub code: u8,       // scancode, HID usage, or VK code
    pub value: u64,     // dx:dy packed, or button bitmap, or key state
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum InputEventKind {
    KeyDown     = 0x01,
    KeyUp       = 0x02,
    MouseMove   = 0x03,
    MouseButton = 0x04,
    MouseWheel  = 0x05,
}

impl InputEvent {
    pub const fn empty() -> Self {
        Self { timestamp: 0, device_id: 0, kind: InputEventKind::KeyDown, _pad: 0, code: 0, value: 0 }
    }

    pub fn key(scancode: u8, pressed: bool) -> Self {
        Self {
            timestamp: 0, device_id: 0,
            kind: if pressed { InputEventKind::KeyDown } else { InputEventKind::KeyUp },
            _pad: 0, code: scancode, value: if pressed { 1 } else { 0 },
        }
    }

    pub fn mouse_move(dx: i16, dy: i16) -> Self {
        Self {
            timestamp: 0, device_id: 1,
            kind: InputEventKind::MouseMove, _pad: 0, code: 0,
            value: ((dx as u16) as u64) | (((dy as u16) as u64) << 16),
        }
    }

    pub fn mouse_button(buttons: u8) -> Self {
        Self {
            timestamp: 0, device_id: 1,
            kind: InputEventKind::MouseButton, _pad: 0, code: 0,
            value: buttons as u64,
        }
    }

    pub fn mouse_wheel(delta: i8) -> Self {
        Self {
            timestamp: 0, device_id: 1,
            kind: InputEventKind::MouseWheel, _pad: 0, code: 0,
            value: (delta as i8 as i16 as u16) as u64,
        }
    }

    pub fn mouse_dx(&self) -> i16 {
        (self.value & 0xFFFF) as i16
    }

    pub fn mouse_dy(&self) -> i16 {
        ((self.value >> 16) & 0xFFFF) as i16
    }

    pub fn mouse_buttons(&self) -> u8 {
        (self.value & 0xFF) as u8
    }
}

/// Lock-free SPSC ring buffer for InputEvents.
pub struct InputEventQueue {
    buf: [UnsafeCell<InputEvent>; QUEUE_CAPACITY],
    write_idx: AtomicUsize,
    read_idx: AtomicUsize,
}

// Safety: SPSC ring buffer where only one thread writes and one thread reads.
unsafe impl Sync for InputEventQueue {}

impl InputEventQueue {
    pub fn new() -> Self {
        // Safe: we initialize each UnsafeCell with valid InputEvent values
        let buf: [UnsafeCell<InputEvent>; QUEUE_CAPACITY] = unsafe {
            core::mem::transmute_copy(&[InputEvent::empty(); QUEUE_CAPACITY])
        };
        Self {
            buf,
            write_idx: AtomicUsize::new(0),
            read_idx: AtomicUsize::new(0),
        }
    }

    /// Push an event. Returns false if queue is full.
    pub fn push(&self, ev: InputEvent) -> bool {
        let w = self.write_idx.load(Ordering::Relaxed);
        let r = self.read_idx.load(Ordering::Acquire);
        let next_w = (w + 1) % QUEUE_CAPACITY;
        if next_w == r { return false; } // full
        unsafe {
            (*self.buf[w].get()) = ev;
        }
        self.write_idx.store(next_w, Ordering::Release);
        true
    }

    /// Pop an event. Returns None if queue is empty.
    pub fn pop(&self) -> Option<InputEvent> {
        let r = self.read_idx.load(Ordering::Relaxed);
        let w = self.write_idx.load(Ordering::Acquire);
        if r == w { return None; } // empty
        let ev = unsafe { *self.buf[r].get() };
        self.read_idx.store((r + 1) % QUEUE_CAPACITY, Ordering::Release);
        Some(ev)
    }

    /// Number of events available.
    pub fn available(&self) -> usize {
        let w = self.write_idx.load(Ordering::Acquire);
        let r = self.read_idx.load(Ordering::Relaxed);
        if w >= r { w - r } else { QUEUE_CAPACITY - r + w }
    }

    /// Drain all keyboard events, returning the last scancode (backward compat with poll_key).
    pub fn drain_key_scancode(&self) -> Option<u8> {
        let mut last = None;
        while let Some(ev) = self.pop() {
            match ev.kind {
                InputEventKind::KeyDown | InputEventKind::KeyUp => {
                    last = Some(ev.code);
                }
                _ => {}
            }
        }
        last
    }

    /// Drain all mouse events, returning packed (x|y<<16|buttons<<32).
    pub fn drain_mouse_packed(&self) -> u64 {
        let mut x: i32 = 0;
        let mut y: i32 = 0;
        let mut btns: u64 = 0;
        while let Some(ev) = self.pop() {
            match ev.kind {
                InputEventKind::MouseMove => {
                    x = x.saturating_add(ev.mouse_dx() as i32);
                    y = y.saturating_add(ev.mouse_dy() as i32);
                }
                InputEventKind::MouseButton => {
                    btns = ev.mouse_buttons() as u64;
                }
                _ => {}
            }
        }
        let xi = (x.clamp(-32768, 32767) as i16) as u16 as u64;
        let yi = (y.clamp(-32768, 32767) as i16) as u16 as u64;
        xi | (yi << 16) | (btns << 32)
    }
}
