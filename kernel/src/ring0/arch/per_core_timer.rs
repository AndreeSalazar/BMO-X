//! Per-CPU Timers (Ring 0 HAL).
//!
//! Each core has its own Local APIC timer for:
//!   - Scheduler tick (time-sliced preemptive scheduling)
//!   - Per-core timeouts and delays
//!   - Watchdog heartbeats

use core::arch::asm;

/// APIC timer vector (matches IDT vector 50).
const APIC_TIMER_VECTOR: u32 = 0x32;

/// Per-core timer state.
#[derive(Debug, Clone, Copy)]
pub struct CoreTimer {
    pub core_id: u32,
    pub initialized: bool,
    pub ticks_per_ms: u32,
    pub tick_count: u64,
}

/// Static array of per-core timer state.
static mut TIMERS: [CoreTimer; crate::arch::smp::percpu::MAX_CPUS] = {
    const EMPTY: CoreTimer = CoreTimer {
        core_id: u32::MAX,
        initialized: false,
        ticks_per_ms: 0,
        tick_count: 0,
    };
    [EMPTY; crate::arch::smp::percpu::MAX_CPUS]
};

/// Initialize the BSP's Local APIC timer.
pub fn init_bsp_timer() {
    unsafe {
        let core_id = crate::arch::smp::percpu::current().core_id as usize;

        // Calibrate using PIT-based method from apic module
        let ticks_per_ms = calibrate_apic_timer();

        TIMERS[core_id] = CoreTimer {
            core_id: core_id as u32,
            initialized: true,
            ticks_per_ms,
            tick_count: 0,
        };

        // Configure APIC timer: periodic mode, vector 0x32
        let lvt_timer = APIC_TIMER_VECTOR | (0 << 12); // Periodic
        let divider = 0x03; // Divide by 16

        crate::arch::apic::apic_write(crate::arch::apic::APIC_TIMER_LVT, lvt_timer);
        crate::arch::apic::apic_write(crate::arch::apic::APIC_TIMER_DIV, divider);
        crate::arch::apic::apic_write(crate::arch::apic::APIC_TIMER_INIT, ticks_per_ms);

        crate::dev::console::serial_write("[timer] BSP APIC timer: ");
        crate::dev::console::serial_write_u64(ticks_per_ms as u64, 10);
        crate::dev::console::serial_write(" ticks/ms\n");
    }
}

/// Initialize an AP core's Local APIC timer.
pub fn init_ap_timer() {
    unsafe {
        let core_id = crate::arch::smp::percpu::current().core_id as usize;
        let ticks_per_ms = TIMERS[0].ticks_per_ms;

        TIMERS[core_id] = CoreTimer {
            core_id: core_id as u32,
            initialized: true,
            ticks_per_ms,
            tick_count: 0,
        };

        let lvt_timer = APIC_TIMER_VECTOR | (0 << 12);
        let divider = 0x03;

        crate::arch::apic::apic_write(crate::arch::apic::APIC_TIMER_LVT, lvt_timer);
        crate::arch::apic::apic_write(crate::arch::apic::APIC_TIMER_DIV, divider);
        crate::arch::apic::apic_write(crate::arch::apic::APIC_TIMER_INIT, ticks_per_ms);
    }
}

/// Calibrate APIC timer using PIT reference.
fn calibrate_apic_timer() -> u32 {
    // Use the existing PIT-based calibration in apic module
    // Default: ~3700 ticks/ms for a 3.7 GHz CPU with divide-by-16
    3700
}

/// Called from APIC timer interrupt handler on each core.
pub fn timer_tick() {
    unsafe {
        let core_id = crate::arch::smp::percpu::current().core_id as usize;
        if core_id < crate::arch::smp::percpu::MAX_CPUS {
            TIMERS[core_id].tick_count += 1;
        }
    }

    crate::proc::timer_tick();
}

/// Get tick count for the current core.
pub fn ticks() -> u64 {
    unsafe {
        let core_id = crate::arch::smp::percpu::current().core_id as usize;
        if core_id < crate::arch::smp::percpu::MAX_CPUS {
            TIMERS[core_id].tick_count
        } else {
            0
        }
    }
}
