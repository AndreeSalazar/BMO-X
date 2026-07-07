//! v2.0 — Ring 3 coordinator.
//!
//! Coordinates userland subsystem initialization and window message dispatch.
//!
//! ## Architecture
//!
//! ```text
//! Ring 0 kernel
//!   │
//!   ├── userland::init()          ← creates initial Ring 3 process
//!   │
//!   ├── ring3::desktop::enter()   ← transitions to CPL=3 with desktop BEF
//!   │
//!   └── userland::enter_wnd_proc() ← dispatches window messages to Ring 3
//! ```
//!
//! ## Process model (v1.8.8)
//!
//! Ring 3 processes are allocated via `bmo_core::proc::process::alloc_process()`.
//! Each process gets:
//! - User code + stack pages (identity-mapped, USER flag)
//! - A page table root (CR3)
//! - Capabilities (windowing, filesystem, audio)
//!
//! ## Window message flow
//!
//! 1. Kernel posts message to process's message queue
//! 2. Scheduler runs the process (CR3 switch + iretq)
//! 3. Process calls GetMessage → wnd_proc executes
//! 4. wnd_proc returns via DISPATCH_RETURN → kernel regains control

use bmo_core::bmo_api::message::BmoMsgKind;
use bmo_core::proc::process::{Pid, alloc_process, get_process};

/// Initialize the Ring 3 userland subsystem.
///
/// Creates the initial process table entries and registers the
/// desktop process. Called once from bmo_core::coord::init().
pub fn init() {
    // Create the desktop process (PID 1)
    let process = match alloc_process() {
        Some(p) => p,
        None => {
            cabina_daemon::warn("userland", "failed to allocate desktop process");
            return;
        }
    };

    process.name = [0u8; 32];
    let dname = b"desktop";
    for i in 0..dname.len() {
        process.name[i] = dname[i];
    }
    process.name_len = 7;
    process.caps = 0
        | 1 << 0   // FileAccess
        | 1 << 4   // Windowing
        | 1 << 6   // Audio
        | 1 << 3;  // MemAlloc

    cabina_daemon::info("userland", &alloc::format!(
        "desktop process allocated: pid={:?}", process.pid
    ));
}

/// Dispatch a window message to a Ring 3 wnd_proc synchronously.
///
/// Returns the wnd_proc result, or None if dispatch failed.
///
/// ## How it works
///
/// 1. Finds the process that owns `hwnd`
/// 2. Posts the message to its queue
/// 3. If the process is Ring 3 (wnd_proc != 0):
///    - The scheduler will execute the process on its next time slice
///    - The process's wnd_proc processes the message
///    - Result comes back via syscall 0x198 (DISPATCH_RETURN)
/// 4. Returns the result
///
/// For kernel-side windows (wnd_proc == 0), the kernel executes
/// default_wnd_proc directly.
pub fn enter_wnd_proc(hwnd: u32, msg: u16, wparam: u64, lparam: u64) -> Option<u64> {
    let kind = BmoMsgKind::from_u16(msg);

    // v1.8.8: synchronous dispatch through the scheduler.
    // The full implementation requires the process message queue
    // and scheduler integration (v1.9).
    //
    // For now, kernel-side windows are handled directly by
    // bmo_core::bmo_api::syscall::dispatch_syscall().

    let _ = (hwnd, kind, wparam, lparam);
    None
}

/// Check if a wnd_proc is kernel-side (0) or Ring 3 (!= 0).
///
/// wnd_proc == 0: kernel default handler (direct call, no context switch)
/// wnd_proc != 0: Ring 3 process handler (requires scheduler dispatch)
pub fn is_ring3_wnd_proc(wnd_proc: u64) -> bool {
    wnd_proc != 0
}
