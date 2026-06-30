//! Task model + ctx switching for FastOS.

#![allow(dead_code)]

use super::process::Pid;

/// Maximum tasks system-wide.
pub const MAX_TASKS: usize = 256;

/// Task identifier.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Tid(pub u32);

/// Task state.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum State {
    Free,
    Ready,
    Running,
    Blocked,
    Dead,
}

/// Saved CPU register state for ctx switching.
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

    /// Create register state for a Ring 3 Task.
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

    /// Create register state for a Ring 0 (kernel) Task.
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

/// A Task — unit of scheduling.
pub struct Task {
    pub tid: Tid,
    pub pid: Pid,
    pub state: State,
    pub priority: super::Priority,
    pub regs: SavedRegs,
    /// Kernel stack for this Task (used during syscalls/interrupts).
    pub kernel_stack_top: u64,
    /// Time slice remaining (in timer ticks).
    pub time_slice: u32,
    /// Futex address we are blocked on (0 = not blocked).
    pub blocked_on: u64,
    /// Per-thread robust list head pointer (set_robust_list).
    pub robust_list_head: u64,
    /// Per-thread tid address (set_tid_address).
    pub tid_address: *mut i32,
}

impl Task {
    pub const fn empty() -> Self {
        Self {
            tid: Tid(0),
            pid: Pid(0),
            state: State::Free,
            priority: super::Priority::Interactive,
            regs: SavedRegs::zero(),
            kernel_stack_top: 0,
            time_slice: 0,
            blocked_on: 0,
            robust_list_head: 0,
            tid_address: core::ptr::null_mut(),
        }
    }
}

/// Global Task table.
static mut TASK_TABLE: [Task; MAX_TASKS] = {
    const EMPTY: Task = Task::empty();
    [EMPTY; MAX_TASKS]
};

static mut NEXT_TID: u32 = 1;

/// Index of the currently running Task (or usize::MAX if none).
static mut CURRENT_IDX: usize = usize::MAX;

/// Allocate a new Task for a process.
pub fn alloc(pid: Pid, priority: super::Priority) -> Option<&'static mut Task> {
    unsafe {
        for i in 0..MAX_TASKS {
            if TASK_TABLE[i].state == State::Free {
                TASK_TABLE[i].tid = Tid(NEXT_TID);
                TASK_TABLE[i].pid = pid;
                TASK_TABLE[i].state = State::Ready;
                TASK_TABLE[i].priority = priority;
                TASK_TABLE[i].time_slice = priority_to_slice(priority);
                NEXT_TID += 1;
                return Some(&mut TASK_TABLE[i]);
            }
        }
        None
    }
}

/// Get the currently running Task.
pub fn current() -> Option<&'static mut Task> {
    unsafe {
        if CURRENT_IDX < MAX_TASKS {
            Some(&mut TASK_TABLE[CURRENT_IDX])
        } else {
            None
        }
    }
}

/// Raw pointer to the current Task (for use in interrupt/asm ctx where
/// we cannot hold &mut references safely).
pub fn current_ptr() -> *mut Task {
    unsafe {
        if CURRENT_IDX < MAX_TASKS {
            core::ptr::addr_of_mut!(TASK_TABLE[CURRENT_IDX])
        } else {
            core::ptr::null_mut()
        }
    }
}

/// Set the current Task index.
pub fn set_current(idx: usize) { unsafe { CURRENT_IDX = idx; } }

/// Get the current Task index.
pub fn current_index() -> usize {
    unsafe { CURRENT_IDX }
}

/// Find Task table index by TID.
#[allow(static_mut_refs)]
pub fn find_index(tid: Tid) -> Option<usize> {
    unsafe {
        TASK_TABLE.iter().position(|t| t.tid == tid && t.state != State::Free)
    }
}

/// Get Task by index.
pub fn get(idx: usize) -> Option<&'static mut Task> {
    unsafe {
        if idx < MAX_TASKS && TASK_TABLE[idx].state != State::Free {
            Some(&mut TASK_TABLE[idx])
        } else {
            None
        }
    }
}

/// Count of ready/running threads.
#[allow(static_mut_refs)]
pub fn ready_count() -> usize {
    unsafe {
        TASK_TABLE.iter().filter(|t| {
            t.state == State::Ready || t.state == State::Running
        }).count()
    }
}

/// Free a Task: release kernel stack, mark slot free.
pub fn free_task(t: &mut Task) {
    if t.state == State::Free {
        return;
    }

    // Free kernel stack
    if t.kernel_stack_top != 0 {
        let kernel_stack_size = 8192; // KERNEL_STACK_PER_THREAD
        unsafe {
            let layout = core::alloc::Layout::from_size_align(kernel_stack_size, 16).unwrap();
            let ptr = (t.kernel_stack_top - kernel_stack_size as u64) as *mut u8;
            alloc::alloc::dealloc(ptr, layout);
        }
        t.kernel_stack_top = 0;
    }

    t.state = State::Dead;
    t.tid = Tid(0);
    t.pid = super::process::Pid(0);
    t.regs = SavedRegs::zero();
    t.time_slice = 0;
}

/// Pick next ready Task (round-robin from current).
pub fn pick_next() -> Option<usize> {
    unsafe {
        let start = if CURRENT_IDX < MAX_TASKS { CURRENT_IDX + 1 } else { 0 };
        // Round-robin: search from current+1, wrap around
        for offset in 0..MAX_TASKS {
            let idx = (start + offset) % MAX_TASKS;
            if TASK_TABLE[idx].state == State::Ready {
                return Some(idx);
            }
        }
        None
    }
}

/// Block the current task on a futex address.
/// The task will not be scheduled again until woken by `wake_on`.
pub fn block_on(uaddr: u64) {
    if let Some(t) = current() {
        t.state = State::Blocked;
        t.blocked_on = uaddr;
    }
    super::schedule();
}

/// Wake up to `max` tasks blocked on a futex address.
/// Returns the number of tasks woken.
pub fn wake_on(uaddr: u64, max: usize) -> usize {
    let mut woken = 0;
    unsafe {
        for task in TASK_TABLE.iter_mut() {
            if woken >= max { break; }
            if task.state == State::Blocked && task.blocked_on == uaddr {
                task.state = State::Ready;
                task.blocked_on = 0;
                woken += 1;
            }
        }
    }
    woken
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
