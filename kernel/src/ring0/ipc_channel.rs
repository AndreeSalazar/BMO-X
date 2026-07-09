//! BMO Channel — Ring 0 side: timer ISR processing + system event queue.
//!
//! ## System channel
//!
//! The kernel owns a dedicated "system channel" for hardware events that
//! Ring 3 can poll. Keyboard/mouse ISRs push events here via `sys_send()`.
//!
//! ## User channels
//!
//! Ring 3 can register additional channels via `SYS_CHANNEL_REGISTER`.
//! These are processed on each timer tick when the doorbell is set.

use bmo_channel::{Channel, ChannelEntry, CHANNEL_MAGIC};
use core::sync::atomic::{AtomicPtr, Ordering};

// ── System channel (kernel-owned, hardware events) ──────────────────────

/// Inline system channel — embedded in the kernel binary, no heap alloc.
/// Ring 3 maps this page via `SYS_CHANNEL_REGISTER` at any physical address.
#[repr(C, align(4096))]
struct SystemChannel {
    ch: Channel,
}

static mut SYS_CHANNEL: SystemChannel = SystemChannel {
    ch: Channel {
        magic: 0,
        doorbell: core::sync::atomic::AtomicU64::new(0),
        submit_head: core::sync::atomic::AtomicU64::new(0),
        submit_tail: core::sync::atomic::AtomicU64::new(0),
        complete_head: core::sync::atomic::AtomicU64::new(0),
        complete_tail: core::sync::atomic::AtomicU64::new(0),
        _pad: [0; 2],
        submit_ring: [ChannelEntry { opcode: 0, arg0: 0, arg1: 0, arg2: 0 }; 62],
        complete_ring: [ChannelEntry { opcode: 0, arg0: 0, arg1: 0, arg2: 0 }; 62],
    },
};

/// Get the physical address of the system channel (for Ring 3 mapping).
pub fn sys_channel_phys() -> u64 {
    unsafe { &raw const SYS_CHANNEL as *const SystemChannel as u64 }
}

/// Push an event into the system channel (called from IRQ handlers).
/// These go into the submit_ring; Ring 3 reads from complete_ring.
pub fn sys_send(opcode: u64, arg0: u64, arg1: u64, arg2: u64) {
    let ch = unsafe { &SYS_CHANNEL.ch };
    ch.ring3_submit(opcode, arg0, arg1, arg2);
    ch.ring3_doorbell();
}

// ── User channels ───────────────────────────────────────────────────────

const MAX_CHANNELS: usize = 8;
static CHANNELS: [AtomicPtr<Channel>; MAX_CHANNELS] = [
    AtomicPtr::new(core::ptr::null_mut()),
    AtomicPtr::new(core::ptr::null_mut()),
    AtomicPtr::new(core::ptr::null_mut()),
    AtomicPtr::new(core::ptr::null_mut()),
    AtomicPtr::new(core::ptr::null_mut()),
    AtomicPtr::new(core::ptr::null_mut()),
    AtomicPtr::new(core::ptr::null_mut()),
    AtomicPtr::new(core::ptr::null_mut()),
];

pub fn register(ch: *mut Channel) -> bool {
    for slot in &CHANNELS {
        if slot.compare_exchange(
            core::ptr::null_mut(), ch,
            Ordering::Relaxed, Ordering::Relaxed,
        ).is_ok() { return true; }
    }
    false
}

// ── Timer ISR processing ────────────────────────────────────────────────

/// Called from timer ISR each tick (~1ms at 1kHz).
/// Processes: PC speaker timeout, keyboard/mouse polling, user channels.
pub fn tick_all() {
    // Speaker timeout
    crate::dev::pc_speaker::tick();

    // Poll keyboard + mouse on each tick (IRQ-driven once IOAPIC is configured)
    crate::irq::keyboard::tick();
    crate::irq::mouse::tick();

    // Process Ring 0 → Ring 3 system channel
    let ch = unsafe { &SYS_CHANNEL.ch };
    if ch.ring0_has_work() {
        ch.ring0_process(|opcode, a0, a1, a2| {
            // Ring 0 → Ring 3: forward events with same opcode
            // Ring 3's ring3_poll() will receive these
            Some((opcode, a0, a1, a2))
        });
    }

    // Process user channels (Ring 3 → Ring 0)
    for slot in &CHANNELS {
        let ch = slot.load(Ordering::Acquire);
        if ch.is_null() { continue; }
        let channel = unsafe { &*ch };
        if channel.ring0_has_work() {
            channel.ring0_process(|opcode, a0, a1, a2| {
                Some((opcode + 100, a0, a1, a2))
            });
        }
    }
}

/// Process all channels immediately (called from SYS_CHANNEL_KICK).
pub fn process_now() -> usize {
    let ch = unsafe { &SYS_CHANNEL.ch };
    if ch.ring0_has_work() {
        ch.ring0_process(|opcode, a0, a1, a2| {
            Some((opcode, a0, a1, a2))
        })
    } else { 0 }
}

/// Initialize the IPC subsystem + hardware polling.
pub fn init() {
    unsafe {
        SYS_CHANNEL.ch.init();
    }
    // Write channel physical address to RAM marker so Ring 3 can find it
    let phys = sys_channel_phys();
    unsafe { core::ptr::write_volatile(0x9_0160 as *mut u64, phys); }
    crate::irq::keyboard::init();
    crate::irq::mouse::init();
}
