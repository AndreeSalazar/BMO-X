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
}

impl Process {
    pub const fn empty() -> Self {
        Self {
            pid: Pid(0),
            state: ProcessState::Free,
            page_table_root: 0,
            addr_space: AddressSpace::empty(),
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
