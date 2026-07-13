//! Task — minimal stub for the Ring 0 base.

#[derive(Debug, Clone, Copy)]
pub struct Task {
    pub id: u32,
    pub state: TaskState,
    /// Saved general-purpose registers (rsp, rbp, rbx, r12-r15, rip).
    pub saved_regs: SavedRegs,
    /// FPU/SSE/AVX save area pointer (0 = lazy save).
    pub fpu_save: *mut u8,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TaskState {
    Free,
    Ready,
    Running,
    Blocked,
}

#[repr(C)]
#[derive(Debug, Clone, Copy, Default)]
pub struct SavedRegs {
    pub rsp: u64,
    pub rbp: u64,
    pub rbx: u64,
    pub r12: u64, pub r13: u64, pub r14: u64, pub r15: u64,
    pub rip: u64,
    pub rflags: u64,
}

static mut TASKS: [Task; super::MAX_TASKS] = [Task {
    id: 0, state: TaskState::Free, saved_regs: SavedRegs { rsp: 0, rbp: 0, rbx: 0, r12: 0, r13: 0, r14: 0, r15: 0, rip: 0, rflags: 0 }, fpu_save: core::ptr::null_mut()
}; super::MAX_TASKS];

pub fn current() -> Option<&'static mut Task> { Some(unsafe { &mut TASKS[0] }) }
pub fn current_index() -> usize { 0 }
pub fn get(_idx: usize) -> Option<&'static mut Task> {
    if _idx < super::MAX_TASKS { Some(unsafe { &mut TASKS[_idx] }) } else { None }
}
