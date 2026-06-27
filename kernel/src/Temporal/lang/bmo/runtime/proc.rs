//! BMO Runtime — Process and thread management.
//!
//! Wraps kernel process/thread management into safe API.

#![allow(dead_code)]

use super::error::{Error, Result};

/// Process identifier.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Pid(pub u32);

/// Thread identifier.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Tid(pub u32);

/// Thread priority levels.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Priority {
    Realtime,
    High,
    Normal,
    Low,
    Idle,
}

impl Priority {
    /// Map to kernel Priority enum.
    fn to_kernel(self) -> crate::proc::Priority {
        match self {
            Priority::Realtime => crate::proc::Priority::Realtime,
            Priority::High => crate::proc::Priority::HighGame,
            Priority::Normal => crate::proc::Priority::Game,
            Priority::Low => crate::proc::Priority::Interactive,
            Priority::Idle => crate::proc::Priority::Idle,
        }
    }
}

/// Current process information.
#[derive(Debug, Clone)]
pub struct ProcessInfo {
    pub pid: Pid,
    pub name: alloc::string::String,
    pub exit_code: i32,
}

/// Spawn a new process with given name and entry point.
pub fn spawn(name: &str, entry: u64) -> Result<Pid> {
    use crate::proc::process::alloc_process;
    use crate::proc::task::alloc;

    let proc = alloc_process().ok_or(Error::ProcessLimit)?;
    proc.set_name(name);
    let pid = Pid(proc.pid.0);

    let thread = alloc(proc.pid, crate::proc::Priority::Interactive).ok_or(Error::ThreadLimit)?;
    let _tid = Tid(thread.tid.0);

    // Set thread entry to user-mode entry point
    thread.regs = crate::proc::task::SavedRegs::new_user(entry, 0x80_0000 + 64 * 1024);

    crate::cabina::info("nexo_proc", "Process spawned");
    Ok(pid)
}

/// Exit the current process with given exit code.
pub fn exit(code: i32) -> ! {
    // Use syscall: ProcessExit (0x00)
    unsafe {
        core::arch::asm!(
            "mov rax, 0x00",
            "syscall",
            in("rdi") code as u64,
            options(noreturn)
        );
    }
}

/// Yield the current thread's time slice.
pub fn yield_now() {
    unsafe {
        core::arch::asm!(
            "mov rax, 0x03",
            "syscall",
            options(nomem, nostack)
        );
    }
}

/// Get current process ID.
pub fn current_pid() -> Pid {
    // Kernel tracks this via thread's pid field
    if let Some(thread) = crate::proc::task::current() {
        Pid(thread.pid.0)
    } else {
        Pid(0)
    }
}

/// Create a new thread in the current process.
pub fn spawn_thread(entry: u64, priority: Priority) -> Result<Tid> {
    use crate::proc::task::alloc;

    let pid = current_pid();
    let thread = alloc(
        crate::proc::process::Pid(pid.0),
        priority.to_kernel(),
    ).ok_or(Error::ThreadLimit)?;

    thread.regs = crate::proc::task::SavedRegs::new_user(entry, 0x80_0000 + 64 * 1024);
    let tid = Tid(thread.tid.0);
    Ok(tid)
}

/// Sleep for given nanoseconds.
pub fn sleep_ns(ns: u64) {
    unsafe {
        core::arch::asm!(
            "mov rax, 0x51",
            "syscall",
            in("rdi") ns,
            options(nomem, nostack)
        );
    }
}

/// Get current time in nanoseconds since boot.
pub fn clock_ns() -> u64 {
    let mut ns: u64;
    unsafe {
        core::arch::asm!(
            "mov rax, 0x50",
            "syscall",
            out("rax") ns,
            options(nomem, nostack)
        );
    }
    ns
}

/// Initialize process subsystem.
pub fn init() {
    crate::cabina::info("bmo_proc", "Process subsystem initialized");
}

