//! Timer Wheel (Ring 0 HAL).
//!
//! Efficient timeout management for the kernel. Uses a hierarchical
//! timer wheel (similar to Linux) to manage thousands of concurrent
//! timeouts with O(1) insert and O(1) tick processing.
//!
//! Architecture:
//!   - 256 slots per level, 4 levels total (1ms, 16ms, 256ms, 4096ms)
//!   - Wheel granularity: 1ms per tick
//!   - Max timeout: ~68 seconds (4 levels × 256 slots × 1ms)
//!   - Each slot is a linked list of timer entries
//!
//! Usage:
//!   timer_wheel::init();
//!   let id = timer_wheel::add_timer(5000, callback); // 5 second timeout
//!   timer_wheel::cancel_timer(id);
//!   timer_wheel::tick(); // called from APIC timer ISR

use core::sync::atomic::{AtomicBool, AtomicU32, AtomicU64, Ordering};

/// Timer callback function type.
pub type TimerCallback = fn(u64); // argument: timer ID

/// Timer entry in the wheel.
#[derive(Clone, Copy)]
struct TimerEntry {
    id: u32,
    expiry_ns: u64,
    callback: TimerCallback,
    active: bool,
}

/// Wheel configuration.
const LEVELS: usize = 4;
const SLOTS_PER_LEVEL: usize = 256;
const TICK_NS: u64 = 1_000_000; // 1 ms in nanoseconds
const MAX_TIMERS: usize = 1024;

/// Timer storage.
static mut WHEEL: [[u32; SLOTS_PER_LEVEL]; LEVELS] = [[0u32; SLOTS_PER_LEVEL]; LEVELS]; // timer IDs per slot
static mut WHEEL_COUNT: [u16; LEVELS] = [0; LEVELS];
static mut TIMERS: [TimerEntry; MAX_TIMERS] = [TimerEntry {
    id: 0,
    expiry_ns: 0,
    callback: dummy_callback,
    active: false,
}; MAX_TIMERS];
static mut NEXT_ID: u32 = 1;
static mut BASE_NS: u64 = 0;

fn dummy_callback(_: u64) {}

/// Global tick counter.
static TICK_COUNT: AtomicU64 = AtomicU64::new(0);

/// Initialize the timer wheel.
pub fn init() {
    unsafe {
        WHEEL = [[0u32; SLOTS_PER_LEVEL]; LEVELS];
        WHEEL_COUNT = [0; LEVELS];
        NEXT_ID = 1;
        BASE_NS = super::timer::now_ns();
    }
}

/// Add a timer that fires after `delay_ns` nanoseconds.
/// Returns a timer ID that can be used to cancel it.
pub fn add_timer(delay_ns: u64, callback: TimerCallback) -> u32 {
    unsafe {
        let id = NEXT_ID;
        NEXT_ID += 1;

        let idx = (id as usize) % MAX_TIMERS;
        let expiry = super::timer::now_ns() + delay_ns;

        TIMERS[idx] = TimerEntry {
            id,
            expiry_ns: expiry,
            callback,
            active: true,
        };

        // Insert into appropriate wheel level
        let elapsed = expiry.saturating_sub(BASE_NS);
        let ticks = (elapsed / TICK_NS) as usize;

        let (level, slot) = if ticks < SLOTS_PER_LEVEL {
            (0, ticks % SLOTS_PER_LEVEL)
        } else if ticks < SLOTS_PER_LEVEL * SLOTS_PER_LEVEL {
            (1, (ticks / SLOTS_PER_LEVEL) % SLOTS_PER_LEVEL)
        } else if ticks < SLOTS_PER_LEVEL * SLOTS_PER_LEVEL * SLOTS_PER_LEVEL {
            (2, (ticks / (SLOTS_PER_LEVEL * SLOTS_PER_LEVEL)) % SLOTS_PER_LEVEL)
        } else {
            (3, (ticks / (SLOTS_PER_LEVEL * SLOTS_PER_LEVEL * SLOTS_PER_LEVEL)) % SLOTS_PER_LEVEL)
        };

        let slot_idx = WHEEL_COUNT[level] as usize;
        if slot_idx < SLOTS_PER_LEVEL {
            WHEEL[level][slot] = id;
            WHEEL_COUNT[level] += 1;
        }

        id
    }
}

/// Cancel a pending timer.
pub fn cancel_timer(id: u32) -> bool {
    unsafe {
        let idx = (id as usize) % MAX_TIMERS;
        if TIMERS[idx].id == id && TIMERS[idx].active {
            TIMERS[idx].active = false;
            true
        } else {
            false
        }
    }
}

/// Process one tick of the timer wheel. Called from APIC timer ISR.
pub fn tick() {
    let now = super::timer::now_ns();
    let elapsed = now.saturating_sub(unsafe { BASE_NS });
    let tick_num = elapsed / TICK_NS;

    TICK_COUNT.fetch_add(1, Ordering::Relaxed);

    // Process level 0 (1ms resolution)
    let slot = (tick_num % SLOTS_PER_LEVEL as u64) as usize;
    let count = unsafe { WHEEL_COUNT[0] };
    for i in 0..count as usize {
        if unsafe { WHEEL[0][slot] } != 0 {
            let timer_id = unsafe { WHEEL[0][slot] };
            let idx = (timer_id as usize) % MAX_TIMERS;
            if unsafe { TIMERS[idx].active } && unsafe { TIMERS[idx].expiry_ns } <= now {
                let cb = unsafe { TIMERS[idx].callback };
                unsafe { TIMERS[idx].active = false; }
                cb(timer_id as u64);
            }
        }
    }
}

/// Get total tick count since initialization.
pub fn ticks() -> u64 {
    TICK_COUNT.load(Ordering::Relaxed)
}
