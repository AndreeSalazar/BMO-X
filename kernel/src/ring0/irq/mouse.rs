//! PS/2 mouse packet parser (standard, IntelliMouse, and Explorer packets).

use core::sync::atomic::{AtomicBool, AtomicI32, AtomicU8, Ordering};

const OP_MOUSE_MOVE: u64 = crate::ring0::ipc_channel::OP_MOUSE_MOVE;
const OP_MOUSE_BUTTON: u64 = crate::ring0::ipc_channel::OP_MOUSE_BUTTON;
const OP_MOUSE_WHEEL: u64 = crate::ring0::ipc_channel::OP_MOUSE_WHEEL;

static PACKET_SIZE: AtomicU8 = AtomicU8::new(3);
static mut PACKET: [u8; 4] = [0; 4];
static mut INDEX: usize = 0;
static mut OLD_BUTTONS: u8 = 0;
static LEGACY_DX: AtomicI32 = AtomicI32::new(0);
static LEGACY_DY: AtomicI32 = AtomicI32::new(0);
static LEGACY_BUTTONS: AtomicU8 = AtomicU8::new(0);
static LEGACY_DIRTY: AtomicBool = AtomicBool::new(false);

pub(crate) fn set_packet_size(size: u8) {
    PACKET_SIZE.store(if size == 4 { 4 } else { 3 }, Ordering::Release);
}

pub(crate) fn reset_packet() {
    unsafe { INDEX = 0; }
}

pub(crate) fn handle_byte(byte: u8) {
    unsafe {
        // Bit 3 is always one in the first byte. Use it to recover cleanly
        // after dropped/corrupt bytes.
        if INDEX == 0 && byte & 0x08 == 0 { return; }
        PACKET[INDEX] = byte;
        INDEX += 1;
        if INDEX < PACKET_SIZE.load(Ordering::Acquire) as usize { return; }
        INDEX = 0;
        finish_packet();
    }
}

unsafe fn finish_packet() {
    let flags = PACKET[0];
    // Overflow bits mean the delta cannot be represented; buttons remain
    // valid, but reporting a wrapped movement would move the pointer wildly.
    let overflow = flags & 0xC0 != 0;
    let dx = (PACKET[1] as i8) as i64;
    let dy = -((PACKET[2] as i8) as i64);
    if !overflow && (dx != 0 || dy != 0) {
        crate::channel::sys_send(OP_MOUSE_MOVE, dx as u64, dy as u64, 0);
        LEGACY_DX.fetch_add(dx as i32, Ordering::Relaxed);
        LEGACY_DY.fetch_add(dy as i32, Ordering::Relaxed);
        LEGACY_DIRTY.store(true, Ordering::Release);
    }

    let mut buttons = flags & 0x07;
    if PACKET_SIZE.load(Ordering::Relaxed) == 4 {
        buttons |= ((PACKET[3] >> 4) & 0x03) << 3;
        let wheel = (((PACKET[3] & 0x0F) << 4) as i8 >> 4) as i64;
        if wheel != 0 {
            crate::channel::sys_send(OP_MOUSE_WHEEL, wheel as u64, 0, 0);
        }
    }
    if buttons != OLD_BUTTONS {
        crate::channel::sys_send(OP_MOUSE_BUTTON, buttons as u64, 0, 0);
        OLD_BUTTONS = buttons;
        LEGACY_BUTTONS.store(buttons, Ordering::Relaxed);
        LEGACY_DIRTY.store(true, Ordering::Release);
    }
}

pub fn take_legacy() -> Option<u64> {
    if !LEGACY_DIRTY.swap(false, Ordering::AcqRel) { return None; }
    let dx = LEGACY_DX.swap(0, Ordering::AcqRel) as i16 as u16 as u64;
    let dy = LEGACY_DY.swap(0, Ordering::AcqRel) as i16 as u16 as u64;
    let buttons = LEGACY_BUTTONS.load(Ordering::Acquire) as u64;
    Some((buttons << 32) | (dy << 16) | dx)
}

/// Kept for callers that previously polled keyboard and mouse separately.
/// The keyboard tick drains and routes both device streams.
pub fn tick() {}

pub fn init() -> bool {
    crate::irq::i8042::init()
}
