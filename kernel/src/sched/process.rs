//! Process model for FastOS.

#![allow(dead_code)]

use crate::sandbox::Capability;

/// Maximum number of processes.
pub const MAX_PROCESSES: usize = 64;

/// Process identifier.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Pid(pub u32);

/// Process state machine.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProcessState {
    /// Slot is free.
    Free,
    /// Process is running or ready to run.
    Active,
    /// Process has exited, waiting to be reaped.
    Zombie,
}

/// A process — owns an address space and one or more threads.
#[derive(Debug)]
pub struct Process {
    pub pid: Pid,
    pub state: ProcessState,
    /// CR3 value — physical address of PML4 page table root.
    pub page_table_root: u64,
    /// Security capabilities granted to this process.
    pub caps: Capability,
    /// Name for debugging.
    pub name: [u8; 32],
    pub name_len: usize,
    /// Entry point virtual address.
    pub entry_point: u64,
    /// Exit code (set when state = Zombie).
    pub exit_code: i32,
}

impl Process {
    pub const fn empty() -> Self {
        Self {
            pid: Pid(0),
            state: ProcessState::Free,
            page_table_root: 0,
            caps: Capability::NONE,
            name: [0u8; 32],
            name_len: 0,
            entry_point: 0,
            exit_code: 0,
        }
    }

    pub fn set_name(&mut self, n: &str) {
        let bytes = n.as_bytes();
        let len = bytes.len().min(31);
        self.name[..len].copy_from_slice(&bytes[..len]);
        self.name_len = len;
    }

    pub fn name_str(&self) -> &str {
        core::str::from_utf8(&self.name[..self.name_len]).unwrap_or("???")
    }
}

/// Global process table.
static mut PROCESS_TABLE: [Process; MAX_PROCESSES] = {
    const EMPTY: Process = Process::empty();
    [EMPTY; MAX_PROCESSES]
};

/// Next PID counter.
static mut NEXT_PID: u32 = 1;

/// Allocate a new process slot. Returns None if table is full.
pub fn alloc_process() -> Option<&'static mut Process> {
    unsafe {
        for i in 0..MAX_PROCESSES {
            if PROCESS_TABLE[i].state == ProcessState::Free {
                PROCESS_TABLE[i].pid = Pid(NEXT_PID);
                PROCESS_TABLE[i].state = ProcessState::Active;
                NEXT_PID += 1;
                return Some(&mut PROCESS_TABLE[i]);
            }
        }
        None
    }
}

/// Get a process by PID.
pub fn get_process(pid: Pid) -> Option<&'static mut Process> {
    unsafe {
        PROCESS_TABLE.iter_mut().find(|p| p.pid == pid && p.state != ProcessState::Free)
    }
}

/// Get process count (active + zombie).
pub fn process_count() -> usize {
    unsafe {
        PROCESS_TABLE.iter().filter(|p| p.state != ProcessState::Free).count()
    }
}
