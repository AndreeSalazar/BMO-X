//! **El hilo de kernel que mantiene vivo el bus USB.**
//!
//! Salio de `dev/usb/mod.rs` el 2026-08-12 por la regla modular. Se puede sacar
//! solo porque **no toca ni una tecla**: bombea el bus y mira el rescate. Todo
//! lo que sabe de teclado y raton se lo pregunta al modulo padre.

use super::rescate::watch_rescue;
use super::{bombear_interno, PRESENT};

// -- ** THE BUS BELONGS TO THE KERNEL ----------------------------------------
//
// # The bug, told in full
//
// Until now the USB bus **only advanced when somebody asked for a key**. The
// only two callers of `bombear_interno` were `poll_ascii` and `evento_tecla`,
// that is:
//
//   * the Ring 0 shell -- but **only while `input::yielded()` is false**, which
//     is the exact opposite of when it is needed; and
//   * the `INPUT_OP_*` of whichever program holds the input.
//
// Put those together and you get this: **the moment a Ring 3 program takes the
// input, the only thing keeping the keyboard and mouse alive is that same
// program.** If it hangs, if it spins, or if it merely takes its time -- loading
// a 4 MB WAD, compositing a heavy frame -- the bus stops advancing, and from the
// outside that looks like a frozen machine. It wasn't frozen: it was waiting for
// the hijacker to ask for the time.
//
// And the rescue shortcut was built on top of that same pumping, so it fell with
// it.
//
// # What is done about it
//
// A **kernel thread** ([`bus_thread`]) pumps the bus on its own, with its own
// stack and its own scheduler slice. From here on:
//
//   * keyboard and mouse keep beating even when nobody asks;
//   * the rescue is watched by the thread ([`watch_rescue`]), so it **works even
//     when the input owner is hung**, which is the only case where it is truly
//     needed;
//   * the syscall paths can still pump -- that is not taken away, so a failure of
//     the thread does not leave the system mute -- but they are no longer the
//     only ones.
//
// # The guard, and why it is not optional
//
// `bombear_interno` touches dozens of `static mut` (queues, counters, xHCI
// state). With the thread there are, for the first time, **two** callers the
// timer can interleave mid-work. The guard is the same one CABINA uses: a flag,
// not a `SpinLock` -- a lock here would deadlock against itself if the one
// already inside is the one that got interrupted.
//
// [!] This is NOT SMP-safe and does not pretend to be: it holds because only the
// BSP runs. The day an AP touches the bus, this flag is a race. Written down on
// purpose instead of pretending otherwise.
static mut PUMPING: bool = false;

/// How many turns the bus thread has taken. If this stops rising the thread died
/// or never started -- and the keyboard depends on somebody asking again.
static mut BUS_TURNS: u64 = 0;
/// How many times the pump was found already running. A high number is not a
/// failure: it is the thread and a syscall asking at the same time.
static mut PUMP_OVERLAPS: u64 = 0;

/// `(thread turns, overlapped pumps)`. For the panel.
pub fn bus_stats() -> (u64, u64) {
    unsafe { (BUS_TURNS, PUMP_OVERLAPS) }
}

/// Pumps the bus with the kernel CR3 loaded and without letting two in at once.
/// **This is the only place that calls `bombear_interno`.**
pub(super) fn pump_bus() {
    use crate::ring0::mm::vmm;
    unsafe {
        if PUMPING {
            PUMP_OVERLAPS = PUMP_OVERLAPS.wrapping_add(1);
            return;
        }
        PUMPING = true;
    }
    // xHCI MMIO is only mapped in the kernel PML4. See the header of
    // [`poll_ascii`]: if we are already on the kernel one, this costs nothing.
    let kpml4 = vmm::kernel_pml4();
    let previous = vmm::read_cr3();
    let switched = kpml4 != 0 && previous != kpml4;
    if switched {
        vmm::switch_to(kpml4);
    }
    bombear_interno();
    if switched {
        vmm::switch_to(previous);
    }
    unsafe { PUMPING = false };
}

/// How often the bus beats, in milliseconds.
///
/// 4 ms = 250 Hz. A USB boot keyboard asks to be polled every 8-10 ms, so this
/// sits comfortably above that without becoming a busy loop. And the thread
/// **sleeps** between turns (`park_until`) instead of yielding hot: yielding in a
/// tight loop would eat everything as soon as there was nothing else to do.
const BUS_PERIOD_MS: u64 = 4;

/// **The kernel thread that keeps the bus alive.** Started once, at boot, and it
/// never returns.
///
/// See the header of [`PUMPING`] for the why. The proof that it is alive is
/// `bus_stats().0` rising.
pub extern "C" fn bus_thread(_arg: u64) -> ! {
    use crate::ring0::task::scheduler;
    loop {
        pump_bus();
        watch_rescue();
        unsafe { BUS_TURNS = BUS_TURNS.wrapping_add(1) };
        let hz = scheduler::tsc_freq();
        if hz == 0 {
            // With no measured TSC there is no way to sleep a concrete amount of
            // time, so yielding is the only honest thing. Should not happen: the
            // TSC is measured before this starts.
            scheduler::yield_current();
            continue;
        }
        let wake_at = scheduler::rdtsc() + hz / 1000 * BUS_PERIOD_MS;
        scheduler::park_until(wake_at);
    }
}

/// Starts [`bus_thread`]. Returns its tid, or `None` if there was no slot.
///
/// Priority 2: above idle and below anything doing real work. The thread runs 250
/// times a second and every turn is short, so what matters is not that it runs
/// soon but that it **always** runs.
pub fn start_bus_thread() -> Option<u32> {
    if !unsafe { PRESENT } {
        crate::ring0::cabina::warn("usb", "sin aparatos: el bus no tiene hilo propio", 0);
        return None;
    }
    let tid = crate::ring0::task::scheduler::spawn_kernel(
        bus_thread as *const () as usize as u64,
        0,
        2,
    );
    match tid {
        Some(t) => {
            crate::ring0::cabina::id("usb", "el bus tiene hilo propio, tid", t as u64);
            Some(t)
        }
        None => {
            // Said out loud, not swallowed: with no thread the system behaves
            // exactly as before -- that is, with the freeze bug.
            crate::ring0::cabina::warn("usb", "NO hubo ranura para el hilo del bus", 0);
            None
        }
    }
}