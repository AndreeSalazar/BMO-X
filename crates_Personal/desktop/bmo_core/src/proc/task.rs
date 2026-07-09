use crate::hal;
use super::process::Pid;
use super::Priority;

pub const MAX_TASKS: usize = 256;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Tid(pub u32);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum State { Free, Ready, Running, Blocked, Dead }

#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct SavedRegs {
    pub rax: u64, pub rbx: u64, pub rcx: u64, pub rdx: u64,
    pub rsi: u64, pub rdi: u64, pub rbp: u64, pub rsp: u64,
    pub r8: u64, pub r9: u64, pub r10: u64, pub r11: u64,
    pub r12: u64, pub r13: u64, pub r14: u64, pub r15: u64,
    pub rip: u64, pub rflags: u64, pub cs: u64, pub ss: u64,
}

impl SavedRegs {
    pub const fn zero() -> Self {
        Self { rax: 0, rbx: 0, rcx: 0, rdx: 0, rsi: 0, rdi: 0, rbp: 0, rsp: 0,
               r8: 0, r9: 0, r10: 0, r11: 0, r12: 0, r13: 0, r14: 0, r15: 0,
               rip: 0, rflags: 0, cs: 0, ss: 0 }
    }
    pub fn new_user(entry: u64, user_stack: u64) -> Self {
        Self { rip: entry, rsp: user_stack, cs: crate::arch::gdt::USER_CS,
               ss: crate::arch::gdt::USER_DS, rflags: 0x202, ..Self::zero() }
    }
    pub fn new_kernel(entry: u64, kernel_stack: u64) -> Self {
        Self { rip: entry, rsp: kernel_stack, cs: crate::arch::gdt::KERNEL_CS,
               ss: crate::arch::gdt::KERNEL_DS, rflags: 0x202, ..Self::zero() }
    }
}

pub struct Task {
    pub tid: Tid, pub pid: Pid, pub state: State, pub priority: Priority,
    pub regs: SavedRegs, pub kernel_stack_top: u64, pub time_slice: u32,
    pub blocked_on: u64, pub robust_list_head: u64, pub tid_address: *mut i32,
}

impl Task {
    pub const fn empty() -> Self {
        Self { tid: Tid(0), pid: Pid(0), state: State::Free,
               priority: Priority::Interactive, regs: SavedRegs::zero(),
               kernel_stack_top: 0, time_slice: 0, blocked_on: 0,
               robust_list_head: 0, tid_address: core::ptr::null_mut() }
    }
}

pub fn current_index() -> usize {
    if let Some(h) = unsafe { hal::HAL.as_ref() } { (h.task_current_index)() } else { usize::MAX }
}

pub fn set_current(idx: usize) {
    if let Some(h) = unsafe { hal::HAL.as_ref() } { (h.task_set_current)(idx); }
}

pub fn get(idx: usize) -> Option<&'static mut Task> {
    if let Some(h) = unsafe { hal::HAL.as_ref() } {
        let ptr = (h.task_get)(idx);
        if ptr.is_null() { None } else { Some(unsafe { &mut *(ptr as *mut Task) }) }
    } else { None }
}

pub fn alloc(pid: Pid, priority: Priority) -> Option<&'static mut Task> {
    if let Some(h) = unsafe { hal::HAL.as_ref() } {
        let ptr = (h.task_alloc)(pid.0, priority as u32);
        if ptr.is_null() { None } else { Some(unsafe { &mut *(ptr as *mut Task) }) }
    } else { None }
}

pub fn free_task(t: &mut Task) {
    if let Some(h) = unsafe { hal::HAL.as_ref() } { (h.task_free)(t as *mut Task as *mut u8); }
}

pub fn current() -> Option<&'static mut Task> {
    if let Some(h) = unsafe { hal::HAL.as_ref() } {
        let ptr = (h.task_current)();
        if ptr.is_null() { None } else { Some(unsafe { &mut *(ptr as *mut Task) }) }
    } else { None }
}

pub fn current_ptr() -> *mut Task {
    if let Some(h) = unsafe { hal::HAL.as_ref() } { (h.task_current)() as *mut Task } else { core::ptr::null_mut() }
}

pub fn pick_next() -> Option<usize> {
    if let Some(h) = unsafe { hal::HAL.as_ref() } { let idx = (h.task_pick_next)(); if idx == usize::MAX { None } else { Some(idx) } } else { None }
}

pub fn block_on(uaddr: u64) {
    if let Some(h) = unsafe { hal::HAL.as_ref() } { (h.task_block_on)(uaddr); }
}

pub fn wake_on(uaddr: u64, max: usize) -> usize {
    if let Some(h) = unsafe { hal::HAL.as_ref() } { (h.task_wake_on)(uaddr, max) } else { 0 }
}
