//! Thread model + context switching for FastOS.

#![allow(dead_code)]

use super::process::Pid;

/// Maximum threads system-wide.
pub const MAX_THREADS: usize = 256;

/// Thread identifier.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Tid(pub u32);

/// Thread state.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ThreadState {
    Free,
    Ready,
    Running,
    Blocked,
    Dead,
}

/// Saved CPU register state for context switching.
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct SavedRegs {
    // General purpose registers
    pub rax: u64,
    pub rbx: u64,
    pub rcx: u64,
    pub rdx: u64,
    pub rsi: u64,
    pub rdi: u64,
    pub rbp: u64,
    pub rsp: u64,
    pub r8:  u64,
    pub r9:  u64,
    pub r10: u64,
    pub r11: u64,
    pub r12: u64,
    pub r13: u64,
    pub r14: u64,
    pub r15: u64,
    // Instruction pointer and flags
    pub rip: u64,
    pub rflags: u64,
    // Segment selectors (for Ring 3 transitions)
    pub cs: u64,
    pub ss: u64,
}

impl SavedRegs {
    pub const fn zero() -> Self {
        Self {
            rax: 0, rbx: 0, rcx: 0, rdx: 0,
            rsi: 0, rdi: 0, rbp: 0, rsp: 0,
            r8: 0, r9: 0, r10: 0, r11: 0,
            r12: 0, r13: 0, r14: 0, r15: 0,
            rip: 0, rflags: 0, cs: 0, ss: 0,
        }
    }

    /// Create register state for a Ring 3 thread.
    pub fn new_user(entry: u64, user_stack: u64) -> Self {
        Self {
            rip: entry,
            rsp: user_stack,
            cs: crate::arch::gdt::USER_CS as u64,
            ss: crate::arch::gdt::USER_DS as u64,
            rflags: 0x202, // IF=1 (interrupts enabled) + reserved bit 1
            ..Self::zero()
        }
    }

    /// Create register state for a Ring 0 (kernel) thread.
    pub fn new_kernel(entry: u64, kernel_stack: u64) -> Self {
        Self {
            rip: entry,
            rsp: kernel_stack,
            cs: crate::arch::gdt::KERNEL_CS as u64,
            ss: crate::arch::gdt::KERNEL_DS as u64,
            rflags: 0x202,
            ..Self::zero()
        }
    }
}

/// A thread — unit of scheduling.
pub struct Thread {
    pub tid: Tid,
    pub pid: Pid,
    pub state: ThreadState,
    pub priority: super::Priority,
    pub regs: SavedRegs,
    /// Kernel stack for this thread (used during syscalls/interrupts).
    pub kernel_stack_top: u64,
    /// Time slice remaining (in timer ticks).
    pub time_slice: u32,
}

impl Thread {
    pub const fn empty() -> Self {
        Self {
            tid: Tid(0),
            pid: Pid(0),
            state: ThreadState::Free,
            priority: super::Priority::Interactive,
            regs: SavedRegs::zero(),
            kernel_stack_top: 0,
            time_slice: 0,
        }
    }
}

/// Global thread table.
static mut THREAD_TABLE: [Thread; MAX_THREADS] = {
    const EMPTY: Thread = Thread::empty();
    [EMPTY; MAX_THREADS]
};

static mut NEXT_TID: u32 = 1;

/// Index of the currently running thread (or usize::MAX if none).
static mut CURRENT_THREAD: usize = usize::MAX;

/// Allocate a new thread for a process.
pub fn alloc_thread(pid: Pid, priority: super::Priority) -> Option<&'static mut Thread> {
    unsafe {
        for i in 0..MAX_THREADS {
            if THREAD_TABLE[i].state == ThreadState::Free {
                THREAD_TABLE[i].tid = Tid(NEXT_TID);
                THREAD_TABLE[i].pid = pid;
                THREAD_TABLE[i].state = ThreadState::Ready;
                THREAD_TABLE[i].priority = priority;
                THREAD_TABLE[i].time_slice = priority_to_slice(priority);
                NEXT_TID += 1;
                return Some(&mut THREAD_TABLE[i]);
            }
        }
        None
    }
}

/// Get the currently running thread.
pub fn current_thread() -> Option<&'static mut Thread> {
    unsafe {
        if CURRENT_THREAD < MAX_THREADS {
            Some(&mut THREAD_TABLE[CURRENT_THREAD])
        } else {
            None
        }
    }
}

/// Set the current thread index.
pub fn set_current(idx: usize) {
    unsafe { CURRENT_THREAD = idx; }
}

/// Get the current thread index.
pub fn current_index() -> usize {
    unsafe { CURRENT_THREAD }
}

/// Find thread table index by TID.
#[allow(static_mut_refs)]
pub fn find_thread_index(tid: Tid) -> Option<usize> {
    unsafe {
        THREAD_TABLE.iter().position(|t| t.tid == tid && t.state != ThreadState::Free)
    }
}

/// Get thread by index.
pub fn get_thread(idx: usize) -> Option<&'static mut Thread> {
    unsafe {
        if idx < MAX_THREADS && THREAD_TABLE[idx].state != ThreadState::Free {
            Some(&mut THREAD_TABLE[idx])
        } else {
            None
        }
    }
}

/// Count of ready/running threads.
#[allow(static_mut_refs)]
pub fn ready_count() -> usize {
    unsafe {
        THREAD_TABLE.iter().filter(|t| {
            t.state == ThreadState::Ready || t.state == ThreadState::Running
        }).count()
    }
}

/// Free a thread: release kernel stack, mark slot free.
pub fn free_thread(thread: &mut Thread) {
    if thread.state == ThreadState::Free {
        return;
    }

    // Free kernel stack
    if thread.kernel_stack_top != 0 {
        let kernel_stack_size = 8192; // KERNEL_STACK_PER_THREAD
        unsafe {
            let layout = core::alloc::Layout::from_size_align(kernel_stack_size, 16).unwrap();
            let ptr = (thread.kernel_stack_top - kernel_stack_size as u64) as *mut u8;
            alloc::alloc::dealloc(ptr, layout);
        }
        thread.kernel_stack_top = 0;
    }

    thread.state = ThreadState::Dead;
    thread.tid = Tid(0);
    thread.pid = super::process::Pid(0);
    thread.regs = SavedRegs::zero();
    thread.time_slice = 0;
}

/// Pick next ready thread (round-robin from current).
pub fn pick_next() -> Option<usize> {
    unsafe {
        let start = if CURRENT_THREAD < MAX_THREADS { CURRENT_THREAD + 1 } else { 0 };
        // Round-robin: search from current+1, wrap around
        for offset in 0..MAX_THREADS {
            let idx = (start + offset) % MAX_THREADS;
            if THREAD_TABLE[idx].state == ThreadState::Ready {
                return Some(idx);
            }
        }
        None
    }
}

/// Time slice in ticks per priority level.
fn priority_to_slice(p: super::Priority) -> u32 {
    match p {
        super::Priority::Realtime    => 1,   // switch ASAP after work done
        super::Priority::HighGame    => 5,
        super::Priority::Game        => 10,
        super::Priority::Interactive => 20,
        super::Priority::Idle        => 50,
    }
}
