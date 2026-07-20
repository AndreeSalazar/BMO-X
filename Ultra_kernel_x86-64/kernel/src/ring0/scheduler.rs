//! Fixed-capacity scheduler policy for the BSP-first kernel.
//!
//! This module owns task lifecycle, priorities and bounded quanta. Context
//! switching is intentionally separate: the timer/interrupt layer will use
//! `tick` and switch to the returned task once saved CPU contexts exist.

pub const MAX_TASKS: usize = 64;
pub const DEFAULT_QUANTUM_TICKS: u16 = 4;

#[repr(u8)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TaskState {
    Empty,
    Ready,
    Running,
    Blocked,
    Exited,
}

#[derive(Clone, Copy, Debug)]
pub struct Task {
    pub tid: u32,
    pub pid: u32,
    pub state: TaskState,
    pub priority: u8,
    pub remaining_ticks: u16,
}

impl Task {
    const EMPTY: Self = Self {
        tid: 0,
        pid: 0,
        state: TaskState::Empty,
        priority: 0,
        remaining_ticks: 0,
    };
}

struct Scheduler {
    tasks: [Task; MAX_TASKS],
    current: usize,
    next_tid: u32,
}

impl Scheduler {
    const fn new() -> Self {
        Self { tasks: [Task::EMPTY; MAX_TASKS], current: 0, next_tid: 1 }
    }

    fn choose_next(&self) -> usize {
        let mut best = None;
        let mut best_priority = 0;
        for offset in 1..=MAX_TASKS {
            let index = (self.current + offset) % MAX_TASKS;
            let task = self.tasks[index];
            if task.state == TaskState::Ready && (best.is_none() || task.priority > best_priority) {
                best = Some(index);
                best_priority = task.priority;
            }
        }
        best.unwrap_or(self.current)
    }

    fn schedule(&mut self) -> u32 {
        let next = self.choose_next();
        if next == self.current { return self.tasks[self.current].tid; }
        if self.tasks[self.current].state == TaskState::Running {
            self.tasks[self.current].state = TaskState::Ready;
        }
        self.current = next;
        self.tasks[next].state = TaskState::Running;
        self.tasks[next].remaining_ticks = DEFAULT_QUANTUM_TICKS;
        self.tasks[next].tid
    }
}

static mut SCHEDULER: Scheduler = Scheduler::new();

fn without_interrupts<T>(f: impl FnOnce(&mut Scheduler) -> T) -> T {
    let flags: u64;
    unsafe { core::arch::asm!("pushfq", "pop {}", "cli", out(reg) flags); }
    let result = unsafe { f(&mut *core::ptr::addr_of_mut!(SCHEDULER)) };
    if flags & (1 << 9) != 0 {
        unsafe { core::arch::asm!("sti", options(nomem, nostack)); }
    }
    result
}

pub fn init() {
    without_interrupts(|scheduler| {
        *scheduler = Scheduler::new();
        scheduler.tasks[0] = Task {
            tid: 1,
            pid: 0,
            state: TaskState::Running,
            priority: 0,
            remaining_ticks: DEFAULT_QUANTUM_TICKS,
        };
        scheduler.next_tid = 2;
    });
}

pub fn create(pid: u32, priority: u8) -> Option<u32> {
    without_interrupts(|scheduler| {
        let index = scheduler.tasks.iter().position(|task| task.state == TaskState::Empty)?;
        let tid = scheduler.next_tid;
        scheduler.next_tid = scheduler.next_tid.wrapping_add(1).max(2);
        scheduler.tasks[index] = Task {
            tid,
            pid,
            state: TaskState::Ready,
            priority: priority.min(31),
            remaining_ticks: DEFAULT_QUANTUM_TICKS,
        };
        Some(tid)
    })
}

pub fn current_tid() -> u32 {
    without_interrupts(|scheduler| scheduler.tasks[scheduler.current].tid)
}

pub fn current_pid() -> u32 {
    without_interrupts(|scheduler| scheduler.tasks[scheduler.current].pid)
}

/// Select another runnable task. This returns the selected TID; the context
/// switch layer is responsible for making that task execute.
pub fn yield_current() -> u32 {
    without_interrupts(Scheduler::schedule)
}

pub fn tick() -> Option<u32> {
    without_interrupts(|scheduler| {
        let current = &mut scheduler.tasks[scheduler.current];
        if current.remaining_ticks > 1 {
            current.remaining_ticks -= 1;
            return None;
        }
        Some(scheduler.schedule())
    })
}

pub fn snapshot(out: &mut [Task]) -> usize {
    without_interrupts(|scheduler| {
        let mut count = 0;
        for task in scheduler.tasks.iter().copied().filter(|task| task.state != TaskState::Empty) {
            if count == out.len() { break; }
            out[count] = task;
            count += 1;
        }
        count
    })
}

pub fn counts() -> (usize, usize) {
    without_interrupts(|scheduler| {
        let mut total = 0;
        let mut runnable = 0;
        for task in &scheduler.tasks {
            if task.state != TaskState::Empty { total += 1; }
            if matches!(task.state, TaskState::Ready | TaskState::Running) { runnable += 1; }
        }
        (total, runnable)
    })
}
