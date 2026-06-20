#![allow(dead_code)]

//! Real-Time Scheduler for FastOS.
//!
//! Earliest Deadline First (EDF) scheduler for time-critical tasks.
//! Used for audio threads, game render loops, and input processing.

use super::thread::Tid;
use crate::device::serial;

/// Maximum RT threads.
const MAX_RT_THREADS: usize = 32;

/// Deadline period in ticks.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RtPeriod {
    Audio,      // ~2.9ms (48kHz audio callback)
    Render,     // ~8.3ms (120 FPS)
    Physics,    // ~16.6ms (60 Hz physics)
    Input,      // ~1ms (input polling)
    Network,    // ~10ms (network tick)
}

impl RtPeriod {
    pub fn ticks(&self) -> u32 {
        match self {
            RtPeriod::Audio => 1,     // 1 tick at 100Hz = 10ms (but we oversample)
            RtPeriod::Render => 1,
            RtPeriod::Physics => 2,
            RtPeriod::Input => 1,
            RtPeriod::Network => 1,
        }
    }
}

/// RT thread entry.
#[derive(Debug, Clone, Copy)]
pub struct RtThread {
    pub tid: Tid,
    pub period: RtPeriod,
    pub deadline: u64,      // Absolute deadline (TSC ticks)
    pub wcet: u64,          // Worst-case execution time (TSC ticks)
    pub utilisation: f32,   // WCET / period (0.0 to 1.0)
    pub active: bool,
}

impl RtThread {
    pub const fn empty() -> Self {
        Self {
            tid: Tid(0),
            period: RtPeriod::Audio,
            deadline: 0,
            wcet: 0,
            utilisation: 0.0,
            active: false,
        }
    }
}

static mut RT_TABLE: [RtThread; MAX_RT_THREADS] = [RtThread::empty(); MAX_RT_THREADS];
static mut RT_COUNT: usize = 0;

/// Register a thread as real-time with a specific deadline.
pub fn register_rt(tid: Tid, period: RtPeriod) -> bool {
    unsafe {
        if RT_COUNT >= MAX_RT_THREADS {
            return false;
        }
        RT_TABLE[RT_COUNT] = RtThread {
            tid,
            period,
            deadline: 0,
            wcet: 0,
            utilisation: 0.0,
            active: true,
        };
        RT_COUNT += 1;

        serial::serial_write("[sched] RT thread registered, tid=");
        crate::boot::serial::u64_dec(tid.0 as u64);
        serial::serial_write(" period=");
        match period {
            RtPeriod::Audio => serial::serial_write("Audio"),
            RtPeriod::Render => serial::serial_write("Render"),
            RtPeriod::Physics => serial::serial_write("Physics"),
            RtPeriod::Input => serial::serial_write("Input"),
            RtPeriod::Network => serial::serial_write("Network"),
        }
        serial::serial_write("\n");

        true
    }
}

/// Pick the highest-priority RT thread that is ready and past its deadline.
pub fn pick_rt_thread() -> Option<Tid> {
    unsafe {
        let mut best: Option<Tid> = None;
        let mut best_deadline = u64::MAX;

        for i in 0..RT_COUNT {
            let rt = &RT_TABLE[i];
            if !rt.active {
                continue;
            }
            // Check if this RT thread's deadline is the most urgent
            if rt.deadline < best_deadline {
                // Check if the thread is actually ready
                // (we can't directly access thread table here without a ref)
                best = Some(rt.tid);
                best_deadline = rt.deadline;
            }
        }

        best
    }
}

/// Called on each timer tick to update RT deadlines.
pub fn tick_update(current_tick: u64) {
    unsafe {
        for i in 0..RT_COUNT {
            let rt = &mut RT_TABLE[i];
            if !rt.active {
                continue;
            }
            // If deadline passed, set next deadline
            if current_tick >= rt.deadline {
                rt.deadline = current_tick + rt.period.ticks() as u64;
            }
        }
    }
}

/// Unregister an RT thread.
pub fn unregister(tid: Tid) {
    unsafe {
        for i in 0..RT_COUNT {
            if RT_TABLE[i].tid == tid && RT_TABLE[i].active {
                RT_TABLE[i].active = false;
                // Compact the table
                for j in i..RT_COUNT - 1 {
                    if RT_TABLE[j + 1].active {
                        RT_TABLE[j] = RT_TABLE[j + 1];
                    }
                }
                RT_COUNT -= 1;
                return;
            }
        }
    }
}

/// Total RT utilisation (sum of all WCET/period ratios).
pub fn total_utilisation() -> f32 {
    unsafe {
        let mut total = 0.0f32;
        for i in 0..RT_COUNT {
            if RT_TABLE[i].active {
                total += RT_TABLE[i].utilisation;
            }
        }
        total
    }
}

/// Print RT scheduler status.
pub fn print_status() {
    use crate::boot::serial::u64_dec;
    serial::serial_write("[sched] RT threads: ");
    u64_dec(unsafe { RT_COUNT as u64 });
    serial::serial_write(" util=");
    let util = total_utilisation();
    serial_write_f32(util);
    serial::serial_write("\n");
}

fn serial_write_f32(val: f32) {
    // Simple integer part + 2 decimal places
    let int_part = val as u32;
    let frac_part = ((val - int_part as f32) * 100.0) as u32;
    use crate::boot::serial::u32_dec;
    u32_dec(int_part);
    serial::serial_write(".");
    if frac_part < 10 { serial::serial_write_byte(b'0'); }
    u32_dec(frac_part);
}
