//! Fixed-capacity scheduler with real context switching at trap boundaries.
//!
//! Design rule: **a context switch only ever happens at a trap boundary**
//! (timer IRQ or SYSCALL). Voluntary operations from kernel tasks just mark
//! state and park in a `hlt` loop; the next trap commits the switch through
//! the unified frame. SYSCALLs from Ring 3 are themselves trap frames, so
//! YIELD/WAIT/EXIT switch immediately and correctly from the dispatcher.
//!
//! A running context is captured into its task by the trap stub writing
//! `percpu.trap_rsp`; `schedule_locked` stores that into the outgoing task
//! and publishes the next task's `context_rsp` back to `percpu.trap_rsp`,
//! which the trap epilogue restores.

use crate::ring0::mm::{self, phys};
use crate::ring0::percpu;
use crate::ring0::spin::SpinLock;
use crate::ring0::trap;

pub const MAX_TASKS: usize = 64;
pub const DEFAULT_QUANTUM_TICKS: u16 = 4;
/// 16 KiB, por el mismo motivo que en `proc.rs`: el contexto con XSAVE ocupa
/// ~3,3 KiB de pila en cada trap, contra los 720 bytes de cuando eran 8 KiB.
const TASK_STACK_PAGES: u64 = 4;

const _: () = assert!(
    crate::ring0::trap::MIN_TASK_STACK <= (TASK_STACK_PAGES * crate::ring0::mm::PAGE) as usize,
    "la pila de tarea de kernel no cubre un contexto con XSAVE"
);

#[repr(u8)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TaskState {
    Empty,
    Ready,
    Running,
    Blocked,
    Exited,
}

#[derive(Clone, Copy)]
pub struct Task {
    pub tid: u32,
    pub pid: u32,
    pub state: TaskState,
    pub priority: u8,
    pub remaining_ticks: u16,
    /// fxsave-base of the saved context (see trap.rs); 0 = never ran.
    pub context_rsp: u64,
    pub stack_phys: u64,
    pub stack_pages: u64,
    /// WAIT: channel/waitable identity the task sleeps on (0 = none).
    pub wait_key: u64,
    /// WAIT: absolute TSC deadline (0 = sleeps until `wait_key` fires).
    pub wait_deadline: u64,
    pub is_user: bool,
    pub cr3: u64,
    /// Top of the task's kernel stack (traps from Ring 3 land here via
    /// TSS.RSP0, and the SYSCALL entry reloads it from PerCpu).
    pub kernel_stack_top: u64,
}

impl Task {
    const EMPTY: Self = Self {
        tid: 0,
        pid: 0,
        state: TaskState::Empty,
        priority: 0,
        remaining_ticks: 0,
        context_rsp: 0,
        stack_phys: 0,
        stack_pages: 0,
        wait_key: 0,
        wait_deadline: 0,
        is_user: false,
        cr3: 0,
        kernel_stack_top: 0,
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

    /// Free kernel stacks of exited tasks and recycle their slots.
    /// Never reaps the running task.
    fn reap(&mut self) {
        let current_tid = self.tasks[self.current].tid;
        for task in &mut self.tasks {
            if task.state == TaskState::Exited && task.tid != current_tid {
                if task.stack_phys != 0 {
                    for p in 0..task.stack_pages {
                        phys::free_frame(task.stack_phys + p * mm::PAGE);
                    }
                }
                *task = Task::EMPTY;
            }
        }
    }
}

static SCHED_LOCK: SpinLock = SpinLock::new();
static mut SCHEDULER: Scheduler = Scheduler::new();
static mut TSC_FREQ: u64 = 0;

fn sched() -> &'static mut Scheduler {
    unsafe { &mut *core::ptr::addr_of_mut!(SCHEDULER) }
}

pub fn rdtsc() -> u64 {
    let low: u32;
    let high: u32;
    unsafe { core::arch::asm!("rdtsc", out("eax") low, out("edx") high, options(nomem, nostack)); }
    ((high as u64) << 32) | low as u64
}

pub fn tsc_freq() -> u64 {
    unsafe { TSC_FREQ }
}

pub fn ns_to_tsc(ns: u64) -> u64 {
    let hz = unsafe { TSC_FREQ };
    if hz == 0 {
        return 0;
    }
    ((ns as u128 * hz as u128) / 1_000_000_000u128) as u64
}

/// Boot task 0 is the shell/boot context itself; its context is captured on
/// its first trap. `tsc_hz` feeds WAIT deadline conversion.
pub fn init(tsc_hz: u64) {
    let _g = SCHED_LOCK.lock();
    unsafe { TSC_FREQ = tsc_hz };
    let s = sched();
    *s = Scheduler::new();
    s.tasks[0] = Task {
        tid: 1,
        pid: 0,
        state: TaskState::Running,
        priority: 0,
        remaining_ticks: DEFAULT_QUANTUM_TICKS,
        ..Task::EMPTY
    };
    s.next_tid = 2;
}

/// Commit a context switch if a better task exists. Must run with the lock
/// held and from a trap boundary only.
fn schedule_locked(s: &mut Scheduler) {
    // Capture the outgoing context exactly as the trap stub left it.
    let outgoing = percpu::trap_rsp();
    if outgoing != 0 {
        s.tasks[s.current].context_rsp = outgoing;
    }
    if s.tasks[s.current].state == TaskState::Running {
        s.tasks[s.current].state = TaskState::Ready;
    }
    let next = s.choose_next();
    if next == s.current {
        if s.tasks[next].state == TaskState::Ready {
            s.tasks[next].state = TaskState::Running;
        }
        s.reap();
        return;
    }
    let next_rsp = s.tasks[next].context_rsp;
    if next_rsp == 0 {
        // Never switch into a context that does not exist.
        if s.tasks[s.current].state != TaskState::Running {
            s.tasks[s.current].state = TaskState::Running;
        }
        return;
    }
    s.tasks[next].state = TaskState::Running;
    s.tasks[next].remaining_ticks = DEFAULT_QUANTUM_TICKS;
    s.current = next;
    percpu::set_trap_rsp(next_rsp);
    // Address space + Ring 3 trap landing pads follow the selected task.
    // Kernel code keeps running through the switch because every user
    // address space shares the kernel half and the identity map.
    let next_task = s.tasks[next];
    let next_cr3 = if next_task.is_user { next_task.cr3 } else { mm::vmm::kernel_pml4() };
    if next_cr3 != mm::vmm::read_cr3() {
        mm::vmm::switch_to(next_cr3);
    }
    if next_task.is_user {
        crate::ring0::proc::set_tss_rsp0(next_task.kernel_stack_top);
        percpu::set_syscall_stack_top(next_task.kernel_stack_top);
        // Debug capture for the Ring 3 #GP hunt: the context pointer we just
        // published and what its back-pointer slot reads RIGHT NOW, under
        // the user CR3 that is already loaded. If this reads valid here but
        // zero in the trap epilogue an instant later, the content is being
        // clobbered in that window; if it is already zero here, the task
        // table entry itself is stale/corrupt. Painted by faults.rs.
        unsafe {
            SWITCH_SNAP[0] = next_rsp;
            SWITCH_SNAP[1] = ((next_rsp + crate::ring0::trap::XSAVE_AREA as u64) as *const u64).read_volatile();
            SWITCH_SNAP[2] = mm::vmm::read_cr3();
            SWITCH_SNAP[3] = SWITCH_SNAP[3].wrapping_add(1);
            // El PRIMER cruce a CPL3 de la vida del sistema. Momento histórico
            // y, sobre todo, el punto exacto donde antes moríamos: si CABINA lo
            // graba, el iretq a usuario ocurrió de verdad. Una sola vez — esto
            // corre dentro del IRQ del timer, no puede ser charlatán.
            if SWITCH_SNAP[3] == 1 {
                crate::ring0::cabina::info("sched", "primer switch a CPL3 (userspace corre)", next_task.tid as u64);
            }
        }
    }
    s.reap();
}

/// Debug: last switch into a user task — `[context_rsp, backptr@switch,
/// cr3@switch, ordinal]`. See the capture in `schedule_locked`.
pub static mut SWITCH_SNAP: [u64; 4] = [0; 4];

/// Copy of `SWITCH_SNAP` for the fault reporter.
pub fn switch_snap() -> [u64; 4] {
    unsafe { SWITCH_SNAP }
}

/// Number of switches into user tasks since boot (SWITCH_SNAP ordinal).
pub fn user_switches() -> u64 {
    unsafe { SWITCH_SNAP[3] }
}

/// Fault isolation: the CURRENT task took a CPU fault it cannot survive.
/// Mark it Exited (never the shell at index 0), commit a switch to the next
/// runnable context, and return the context_rsp the fault stub must restore.
/// Called from the #UD/#GP/#PF stubs when the fault came from CPL3 — the
/// dying context is NOT saved (it is dead by definition), so unlike the
/// voluntary paths this never touches the outgoing frame.
pub fn kill_current_and_pick() -> u64 {
    let _g = SCHED_LOCK.lock();
    let s = sched();
    if s.current != 0 && s.tasks[s.current].state == TaskState::Running {
        s.tasks[s.current].state = TaskState::Exited;
    }
    schedule_locked(s);
    percpu::trap_rsp()
}

/// Lock-free diagnostic read of a task's state by TID. Racy by design —
/// telemetry only. 255 = no live task with that TID (never existed, or
/// exited and was reaped).
pub fn tid_state(tid: u32) -> u8 {
    let s = unsafe { &*core::ptr::addr_of!(SCHEDULER) };
    for t in &s.tasks {
        if t.tid == tid && t.state != TaskState::Empty {
            return t.state as u8;
        }
    }
    255
}

/// El contexto guardado de una tarea (su `xsave_base`), o 0 si no existe.
///
/// Lo necesita Endpoint RPC para escribir el resultado de una llamada **en el
/// frame guardado del llamante**. Un syscall que bloquea no puede calcular su
/// valor de retorno después de bloquearse: `wait_current_checked` vuelve en el
/// acto y el cambio de contexto se consuma en el epílogo, así que para cuando
/// hubiera respuesta ese código ya se ejecutó. La respuesta se deja donde el
/// epílogo la va a recoger.
/// ★ Solo devuelve el contexto de una tarea **bloqueada**.
///
/// `context_rsp` es donde quedó guardada la tarea la última vez que salió del
/// CPU. Para una tarea que está CORRIENDO ese valor es viejo: su estado real
/// vive en los registros, no en memoria. Escribir ahí no le llega — pisa lo
/// que haya ahora en esa dirección de pila, que es de otra cosa.
///
/// Devolver 0 salvo que esté `Blocked` convierte ese error en un no-op en vez
/// de en una corrupción silenciosa de otro contexto.
pub fn context_rsp_of(tid: u32) -> u64 {
    let s = unsafe { &*core::ptr::addr_of!(SCHEDULER) };
    for t in &s.tasks {
        if t.tid == tid && t.state == TaskState::Blocked {
            return t.context_rsp;
        }
    }
    0
}

/// Timer trap hook: sweep expired WAIT deadlines, account the quantum, and
/// reschedule when it expires.
pub fn on_timer() {
    let _g = SCHED_LOCK.lock();
    let s = sched();
    let now = rdtsc();
    for task in &mut s.tasks {
        if task.state == TaskState::Blocked && task.wait_deadline != 0 && now >= task.wait_deadline {
            task.wait_deadline = 0;
            task.wait_key = 0;
            task.state = TaskState::Ready;
        }
    }
    let current = &mut s.tasks[s.current];
    if current.remaining_ticks > 1 {
        current.remaining_ticks -= 1;
        return;
    }
    current.remaining_ticks = DEFAULT_QUANTUM_TICKS;
    schedule_locked(s);
}

/// Wake every task blocked on `key` (BMO Channel sequence change, F2+).
pub fn wake_by_key(key: u64) {
    let _g = SCHED_LOCK.lock();
    let s = sched();
    for task in &mut s.tasks {
        if task.state == TaskState::Blocked && task.wait_key == key {
            task.wait_key = 0;
            task.wait_deadline = 0;
            task.state = TaskState::Ready;
        }
    }
}

/// Spawn a kernel task running `entry(arg)` on its own 8 KiB stack.
/// Returns the new TID.
pub fn spawn_kernel(entry: u64, arg: u64, priority: u8) -> Option<u32> {
    // Contiguous: the stack is addressed linearly through the physmap, and
    // `reap` frees it as `stack_phys + p*PAGE` — both are only sound when
    // the frames really are physical neighbors.
    let stack_base = phys::alloc_frames_contig(TASK_STACK_PAGES)?;
    let stack_top = mm::phys_to_virt(stack_base) + TASK_STACK_PAGES * mm::PAGE;
    let context = unsafe { trap::fabricate(stack_top, entry, arg, false, 0) };

    let _g = SCHED_LOCK.lock();
    let s = sched();
    let index = s.tasks.iter().position(|t| t.state == TaskState::Empty)?;
    let tid = s.next_tid;
    s.next_tid = s.next_tid.wrapping_add(1).max(2);
    s.tasks[index] = Task {
        tid,
        pid: 0,
        state: TaskState::Ready,
        priority: priority.min(31),
        remaining_ticks: DEFAULT_QUANTUM_TICKS,
        context_rsp: context,
        stack_phys: stack_base,
        stack_pages: TASK_STACK_PAGES,
        wait_key: 0,
        wait_deadline: 0,
        is_user: false,
        cr3: 0,
        kernel_stack_top: stack_top,
    };
    Some(tid)
}

/// Register a Ring 3 task whose initial context was fabricated by the
/// process loader. `kernel_stack` = the trap/syscall landing stack,
/// `cr3` = the user address space. Returns the new TID.
pub fn spawn_user(
    pid: u32,
    context_rsp: u64,
    kernel_stack_phys: u64,
    kernel_stack_pages: u64,
    kernel_stack_top: u64,
    cr3: u64,
    priority: u8,
) -> Option<u32> {
    let _g = SCHED_LOCK.lock();
    let s = sched();
    let index = s.tasks.iter().position(|t| t.state == TaskState::Empty)?;
    let tid = s.next_tid;
    s.next_tid = s.next_tid.wrapping_add(1).max(2);
    s.tasks[index] = Task {
        tid,
        pid,
        state: TaskState::Ready,
        priority: priority.min(31),
        remaining_ticks: DEFAULT_QUANTUM_TICKS,
        context_rsp,
        stack_phys: kernel_stack_phys,
        stack_pages: kernel_stack_pages,
        wait_key: 0,
        wait_deadline: 0,
        is_user: true,
        cr3,
        kernel_stack_top,
    };
    Some(tid)
}

// ── Voluntary operations ─────────────────────────────────────────────
// Each has a trap variant (immediate switch, used by the SYSCALL
// dispatcher) and a mark-only variant (kernel tasks; the next trap
// commits the switch while the task parks in `hlt`).

fn mark_yield(s: &mut Scheduler) {
    if s.tasks[s.current].state == TaskState::Running {
        s.tasks[s.current].state = TaskState::Ready;
    }
}

fn mark_exit(s: &mut Scheduler) {
    // Task 1 is the shell/boot context; it may not exit.
    if s.current != 0 && s.tasks[s.current].state == TaskState::Running {
        s.tasks[s.current].state = TaskState::Exited;
    }
}

fn mark_wait(s: &mut Scheduler, key: u64, deadline: u64) {
    if s.tasks[s.current].state == TaskState::Running {
        s.tasks[s.current].state = TaskState::Blocked;
        s.tasks[s.current].wait_key = key;
        s.tasks[s.current].wait_deadline = deadline;
    }
}

pub fn yield_current() {
    let _g = SCHED_LOCK.lock();
    let s = sched();
    mark_yield(s);
    schedule_locked(s);
}

pub fn exit_current() {
    let _g = SCHED_LOCK.lock();
    let s = sched();
    let tid = s.tasks[s.current].tid;
    let user = s.tasks[s.current].is_user;
    // Salida VOLUNTARIA (INVOKE EXIT). La distinción con la muerte por fault
    // (faults.rs) importa en la bitácora: una terminó su trabajo, la otra la
    // mataron. Antes ambas se veían igual: un contador que bajaba.
    crate::ring0::cabina::info(
        if user { "ring3" } else { "sched" },
        "proceso termino por su cuenta (EXIT)",
        tid as u64,
    );
    mark_exit(s);
    schedule_locked(s);
}

pub fn wait_current(key: u64, deadline_tsc: u64) {
    let _g = SCHED_LOCK.lock();
    let s = sched();
    mark_wait(s, key, deadline_tsc);
    schedule_locked(s);
}

/// WAIT with a lost-wakeup guard: `seq()` is sampled *under the scheduler
/// lock* and compared against `observed`. Because `wake_by_key` also takes
/// the lock, a kick can never slip between the compare and the block. If
/// the sequence already moved, returns it immediately without blocking.
pub fn wait_current_checked(
    key: u64,
    deadline_tsc: u64,
    observed: u64,
    seq: impl Fn() -> u64,
) -> u64 {
    let _g = SCHED_LOCK.lock();
    let current = seq();
    if current != observed {
        return current;
    }
    let s = sched();
    mark_wait(s, key, deadline_tsc);
    schedule_locked(s);
    // Still pre-switch on this stack: the context switch commits at the
    // trap epilogue. The value returned here is what the caller sees when
    // resumed, so it is advisory — userland re-reads the shared page.
    observed
}

/// Kernel-task parking: mark blocked, then `hlt` until scheduled again.
pub fn park_until(deadline_tsc: u64) {
    {
        let _g = SCHED_LOCK.lock();
        let s = sched();
        mark_wait(s, 0, deadline_tsc);
    }
    while current_state() != TaskState::Running {
        unsafe { core::arch::asm!("hlt"); }
    }
}

/// Kernel-task exit: mark exited, then park forever (never resumed).
pub fn exit_and_park() -> ! {
    {
        let _g = SCHED_LOCK.lock();
        let s = sched();
        mark_exit(s);
    }
    loop {
        unsafe { core::arch::asm!("hlt"); }
    }
}

pub fn current_state() -> TaskState {
    let _g = SCHED_LOCK.lock();
    sched().tasks[sched().current].state
}

pub fn current_tid() -> u32 {
    let _g = SCHED_LOCK.lock();
    let s = sched();
    s.tasks[s.current].tid
}

pub fn current_pid() -> u32 {
    let _g = SCHED_LOCK.lock();
    let s = sched();
    s.tasks[s.current].pid
}

pub fn counts() -> (usize, usize) {
    let _g = SCHED_LOCK.lock();
    let s = sched();
    let mut total = 0;
    let mut runnable = 0;
    for task in &s.tasks {
        if task.state != TaskState::Empty {
            total += 1;
        }
        if matches!(task.state, TaskState::Ready | TaskState::Running) {
            runnable += 1;
        }
    }
    (total, runnable)
}
