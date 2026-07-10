//! Process table — native Ring 0 types (no bmo_core dependency).

use crate::mm::vmm::AddressSpace;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Pid(pub u32);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProcessState {
    Free,
    Active,
    Zombie,
}

pub const MAX_PROCESSES: usize = 256;

#[derive(Debug)]
pub struct Process {
    pub pid: Pid,
    pub state: ProcessState,
    pub page_table_root: u64,
    pub addr_space: AddressSpace,
    /// IPC message queue head pointer (linked list of MsgNode).
    pub msg_head: *mut MsgNode,
    pub msg_tail: *mut MsgNode,
    /// Sandbox capabilities bitmap (same format as crate::fs::Capabilities).
    pub capabilities: u32,
}

/// IPC message node (heap-allocated, linked list).
pub struct MsgNode {
    pub next: *mut MsgNode,
    pub data: *mut u8,
    pub len: usize,
}

impl Process {
    pub const fn empty() -> Self {
        Self {
            pid: Pid(0),
            state: ProcessState::Free,
            page_table_root: 0,
            addr_space: AddressSpace::empty(),
            msg_head: core::ptr::null_mut(),
            msg_tail: core::ptr::null_mut(),
            capabilities: 0,
        }
    }
}

static mut PROCESSES: [Process; MAX_PROCESSES] = {
    const E: Process = Process::empty();
    [E; MAX_PROCESSES]
};

static mut PROCESS_COUNT: usize = 0;

pub fn get_process(pid: Pid) -> Option<&'static mut Process> {
    unsafe {
        for i in 0..MAX_PROCESSES {
            if PROCESSES[i].state != ProcessState::Free && PROCESSES[i].pid == pid {
                return Some(&mut PROCESSES[i]);
            }
        }
    }
    None
}

pub fn process_count() -> usize {
    unsafe { PROCESS_COUNT }
}

/// Allocate a new process with the given PID. Returns None if the table is full.
pub fn alloc_process(pid: Pid) -> Option<&'static mut Process> {
    unsafe {
        for i in 0..MAX_PROCESSES {
            if PROCESSES[i].state == ProcessState::Free {
                PROCESSES[i].pid = pid;
                PROCESSES[i].state = ProcessState::Active;
                PROCESSES[i].page_table_root = 0;
                PROCESSES[i].addr_space = AddressSpace::empty();
                PROCESSES[i].msg_head = core::ptr::null_mut();
                PROCESSES[i].msg_tail = core::ptr::null_mut();
                PROCESSES[i].capabilities = 0;
                PROCESS_COUNT += 1;
                return Some(&mut PROCESSES[i]);
            }
        }
    }
    None
}

/// Free a process slot, transitioning it to Free state.
pub fn free_process(pid: Pid) {
    unsafe {
        for i in 0..MAX_PROCESSES {
            if PROCESSES[i].state != ProcessState::Free && PROCESSES[i].pid == pid {
                PROCESSES[i].state = ProcessState::Free;
                PROCESSES[i].pid = Pid(0);
                PROCESS_COUNT = PROCESS_COUNT.saturating_sub(1);
                return;
            }
        }
    }
}
