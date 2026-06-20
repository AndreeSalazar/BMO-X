//! Process model for FastOS.

#![allow(dead_code)]

use crate::bmo_core::sandbox::Capability;

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
    /// User code virtual base address.
    pub user_code_base: u64,
    /// User code size in bytes.
    pub user_code_size: usize,
    /// User stack virtual base address.
    pub user_stack_base: u64,
    /// User stack size in bytes.
    pub user_stack_size: usize,
    /// Virtual memory areas for demand paging / CoW.
    pub addr_space: crate::memory::paging::AddressSpace,
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
            user_code_base: 0,
            user_code_size: 0,
            user_stack_base: 0,
            user_stack_size: 0,
            addr_space: crate::memory::paging::AddressSpace::empty(),
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
#[allow(static_mut_refs)]
pub fn get_process(pid: Pid) -> Option<&'static mut Process> {
    unsafe {
        PROCESS_TABLE.iter_mut().find(|p| p.pid == pid && p.state != ProcessState::Free)
    }
}

/// Get process count (active + zombie).
#[allow(static_mut_refs)]
pub fn process_count() -> usize {
    unsafe {
        PROCESS_TABLE.iter().filter(|p| p.state != ProcessState::Free).count()
    }
}

/// Free a process: release user pages, page tables, mark slot free.
pub fn free_process(proc: &mut Process) {
    if proc.state == ProcessState::Free {
        return;
    }

    // Free user code pages
    if proc.user_code_size > 0 {
        let code_pages = (proc.user_code_size + crate::memory::page_alloc::page_size() - 1) / crate::memory::page_alloc::page_size();
        unsafe {
            crate::memory::page_alloc::free_pages(proc.user_code_base, code_pages);
        }
    }

    // Free user stack pages
    if proc.user_stack_size > 0 {
        let stack_pages = (proc.user_stack_size + crate::memory::page_alloc::page_size() - 1) / crate::memory::page_alloc::page_size();
        unsafe {
            crate::memory::page_alloc::free_pages(proc.user_stack_base, stack_pages);
        }
    }

    // Free user page tables (PDPTs, PDs, PTs)
    if proc.page_table_root != 0 {
        unsafe {
            crate::memory::paging::free_user_page_tables(proc.page_table_root);
            // Free the PML4 itself
            crate::memory::page_alloc::free_pages(proc.page_table_root, 1);
        }
        proc.page_table_root = 0;
    }

    // Mark process as free
    proc.state = ProcessState::Free;
    proc.pid = Pid(0);
    proc.caps = Capability::NONE;
    proc.name = [0u8; 32];
    proc.name_len = 0;
    proc.entry_point = 0;
    proc.exit_code = 0;
    proc.user_code_base = 0;
    proc.user_code_size = 0;
    proc.user_stack_base = 0;
    proc.user_stack_size = 0;
}

/// Kill the current process (called from syscall or exception handler).
/// Switches back to kernel page table, frees resources, marks as Zombie,
/// and calls schedule() to switch to next thread.
///
/// Safety: Does NOT free the current thread's kernel stack (we're executing on it).
/// After schedule(), loops with HLT — the next timer/interrupt will switch to
/// another thread using TSS.RSP0 (already updated by schedule).
pub fn kill_current_process(vector: u64, _error_code: u64, _cr2: u64) -> ! {
    crate::bmo_core::diag::fault_u64("process", "killing current process", vector);

    let current_idx = super::thread::current_index();
    if let Some(thread) = super::thread::get_thread(current_idx) {
        let pid = thread.pid;

        // Mark thread as Dead but DO NOT free kernel stack — we're executing on it.
        thread.state = super::thread::ThreadState::Dead;
        thread.tid = super::thread::Tid(0);
        thread.pid = super::process::Pid(0);
        thread.time_slice = 0;

        // Free user-space resources (code pages, stack pages, page tables)
        if let Some(proc) = get_process(pid) {
            proc.exit_code = -1;
            proc.state = ProcessState::Zombie;

            // Switch back to kernel page table before freeing user pages
            let kernel_cr3 = crate::memory::paging::read_cr3();
            if proc.page_table_root != 0 && proc.page_table_root != kernel_cr3 {
                unsafe { crate::memory::paging::write_cr3(kernel_cr3); }
            }

            // Free process resources (NOT kernel stack)
            free_process(proc);
        }

        // Clear current thread — no thread is "running" now
        super::thread::set_current(usize::MAX);
    }

    // Schedule next thread (updates TSS.RSP0 for next interrupt)
    crate::bmo_core::diag::trace("process", "scheduling after kill");
    super::schedule();

    // We're on the dead thread's kernel stack — can't do anything useful.
    // Loop until next timer/interrupt fires and switches to another thread
    // via TSS.RSP0 (already updated by schedule).
    loop {
        unsafe { core::arch::asm!("sti; hlt"); }
    }
}
