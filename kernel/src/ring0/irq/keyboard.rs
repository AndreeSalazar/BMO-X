//! PS/2 keyboard byte parser.
//!
//! The shared i8042 driver owns ports 0x60/0x64 and routes keyboard bytes
//! here. Events preserve make/break state and set-1 scancode prefixes.

use core::sync::atomic::{AtomicU8, AtomicUsize, Ordering};

const OP_KEY: u64 = crate::ring0::ipc_channel::OP_KEY_SCANCODE;
const QUEUE_SIZE: usize = 64;
static PREFIX_STATE: AtomicU8 = AtomicU8::new(0);
static QUEUE_HEAD: AtomicUsize = AtomicUsize::new(0);
static QUEUE_TAIL: AtomicUsize = AtomicUsize::new(0);
static mut QUEUE: [u16; QUEUE_SIZE] = [0; QUEUE_SIZE];

pub(crate) fn reset_prefix() {
    PREFIX_STATE.store(0, Ordering::Relaxed);
}

pub(crate) fn handle_byte(byte: u8) {
    let state = PREFIX_STATE.load(Ordering::Relaxed);
    if state >= 2 {
        PREFIX_STATE.store(if state == 2 { 0 } else { state - 1 }, Ordering::Relaxed);
        return;
    }
    match byte {
        0xE0 => {
            PREFIX_STATE.store(1, Ordering::Relaxed);
            return;
        }
        0xE1 => {
            // Pause/Break has five trailing bytes and no break code. Consume
            // it until the ABI defines a dedicated logical key.
            PREFIX_STATE.store(6, Ordering::Relaxed);
            return;
        }
        0xFA | 0xFE => return, // Command responses, never key events.
        _ => {}
    }

    let pressed = byte & 0x80 == 0;
    let code = byte & 0x7F;
    let extended = PREFIX_STATE.swap(0, Ordering::Relaxed) == 1;
    crate::channel::sys_send(OP_KEY, code as u64, pressed as u64, extended as u64);

    let head = QUEUE_HEAD.load(Ordering::Relaxed);
    let tail = QUEUE_TAIL.load(Ordering::Acquire);
    if head.wrapping_sub(tail) < QUEUE_SIZE {
        let event = code as u16 | ((!pressed as u16) << 7) | ((extended as u16) << 8);
        unsafe { QUEUE[head % QUEUE_SIZE] = event; }
        QUEUE_HEAD.store(head.wrapping_add(1), Ordering::Release);
    }
}

pub fn pop_scancode() -> Option<u16> {
    let tail = QUEUE_TAIL.load(Ordering::Relaxed);
    if tail == QUEUE_HEAD.load(Ordering::Acquire) { return None; }
    let event = unsafe { QUEUE[tail % QUEUE_SIZE] };
    QUEUE_TAIL.store(tail.wrapping_add(1), Ordering::Release);
    Some(event)
}

pub fn tick() {
    crate::irq::i8042::poll();
}

pub fn init() -> bool {
    crate::irq::i8042::init()
}
