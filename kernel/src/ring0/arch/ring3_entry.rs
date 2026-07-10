//! Ring 3 Entry — Transition from Ring 0 to Ring 3 (user mode).
//!
//! This module implements the critical transition from kernel mode (Ring 0)
//! to user mode (Ring 3) using either `iretq` or `sysret`.
//!
//! ## Architecture
//!
//! The transition sets up:
//! - User-mode code segment (CS) with RPL=3
//! - User-mode data segment (SS) with RPL=3
//! - User-mode stack pointer (RSP)
//! - User-mode instruction pointer (RIP)
//! - RFLAGS with interrupts enabled (IF=1)
//!
//! ## Two Methods
//!
//! 1. **iretq**: Traditional method, works with any x86-64 CPU.
//!    Used for the initial entry into Ring 3.
//!
//! 2. **sysret**: Fast path for returning from syscalls.
//!    Requires MSR setup (IA32_STAR, IA32_LSTAR, etc.)
//!    Already implemented in `arch/syscall.rs`

use core::arch::asm;

/// User-mode code segment selector (Ring 3)
/// In GDT: offset 0x18 (4th entry), RPL=3 → 0x18 | 3 = 0x1B
pub const USER_CS: u16 = 0x1B;

/// User-mode data segment selector (Ring 3)
/// In GDT: offset 0x20 (5th entry), RPL=3 → 0x20 | 3 = 0x23
pub const USER_DS: u16 = 0x23;

/// Initial RFLAGS for Ring 3 (IF=1, bit 9)
const USER_RFLAGS: u64 = 0x202; // IF=1, reserved bit 1=1

/// Transition to Ring 3 for the first time.
///
/// This function sets up the CPU state and executes `iretq` to jump
/// to user mode. It does NOT return.
///
/// # Arguments
///
/// * `entry_point` - Virtual address of the user-mode entry point (RIP)
/// * `user_stack` - Virtual address of the user-mode stack top (RSP)
///
/// # Safety
///
/// This function never returns. The entry_point and user_stack must be
/// valid user-mode virtual addresses with correct permissions.
pub fn enter_ring3(entry_point: u64, user_stack: u64) -> ! {
    crate::dev::console::serial_write("[ring3] transitioning to Ring 3...\n");
    crate::dev::console::serial_write("[ring3] entry=0x");
    crate::dev::console::serial_write_u64(entry_point, 16);
    crate::dev::console::serial_write(" stack=0x");
    crate::dev::console::serial_write_u64(user_stack, 16);
    crate::dev::console::serial_write("\n");

    unsafe {
        // Build the iretq frame on the stack:
        // SS     (offset 32)
        // RSP    (offset 24)
        // RFLAGS (offset 16)
        // CS     (offset 8)
        // RIP    (offset 0)
        
        asm!(
            // Disable interrupts during transition
            "cli",
            
            // Push SS (user data segment)
            "push {user_ds:r}",
            
            // Push RSP (user stack)
            "push {user_stack}",
            
            // Push RFLAGS (IF=1)
            "push {rflags}",
            
            // Push CS (user code segment)
            "push {user_cs:r}",
            
            // Push RIP (entry point)
            "push {entry}",
            
            // Set up segment registers for Ring 3
            "mov ax, {user_ds:r}",
            "mov ds, ax",
            "mov es, ax",
            "mov fs, ax",
            "mov gs, ax",
            
            // Execute iretq to jump to Ring 3
            // This pops: RIP, CS, RFLAGS, RSP, SS
            "iretq",
            
            user_ds = in(reg) USER_DS as u64,
            user_stack = in(reg) user_stack,
            rflags = in(reg) USER_RFLAGS,
            user_cs = in(reg) USER_CS as u64,
            entry = in(reg) entry_point,
            options(noreturn)
        );
    }
}

/// Prepare a task for Ring 3 execution.
///
/// This sets up the saved registers for a task so that when it's
/// scheduled, it will enter Ring 3 via the context switch code.
///
/// # Arguments
///
/// * `task` - The task to prepare
/// * `entry_point` - User-mode entry point
/// * `user_stack` - User-mode stack top
pub fn prepare_task_for_ring3(
    task: &mut crate::proc::task::Task,
    entry_point: u64,
    user_stack: u64,
) {
    // Set up the saved registers for iretq return
    task.regs.rip = entry_point;
    task.regs.rsp = user_stack;
    task.regs.cs = USER_CS as u64;
    task.regs.ss = USER_DS as u64;
    task.regs.rflags = USER_RFLAGS;
    
    // Clear other registers (user mode starts with clean state)
    task.regs.rax = 0;
    task.regs.rbx = 0;
    task.regs.rcx = 0;
    task.regs.rdx = 0;
    task.regs.rsi = 0;
    task.regs.rdi = 0;
    task.regs.rbp = 0;
    task.regs.r8 = 0;
    task.regs.r9 = 0;
    task.regs.r10 = 0;
    task.regs.r11 = 0;
    task.regs.r12 = 0;
    task.regs.r13 = 0;
    task.regs.r14 = 0;
    task.regs.r15 = 0;
    
    crate::dev::console::serial_write("[ring3] task prepared for Ring 3 entry\n");
}

/// Context switch to Ring 3 via iretq.
///
/// This is called from the scheduler when switching to a user-mode task.
/// It restores the task's registers and executes iretq.
///
/// # Safety
///
/// The task must have valid Ring 3 state (CS, SS, RIP, RSP, RFLAGS).
pub unsafe fn context_switch_to_ring3(task: &crate::proc::task::Task) -> ! {
    let regs = &task.regs;
    
    asm!(
        // Restore general-purpose registers
        "mov r15, {r15}",
        "mov r14, {r14}",
        "mov r13, {r13}",
        "mov r12, {r12}",
        "mov rbp, {rbp}",
        "mov rbx, {rbx}",
        "mov r9,  {r9}",
        "mov r8,  {r8}",
        "mov rdx, {rdx}",
        "mov rsi, {rsi}",
        "mov rdi, {rdi}",
        
        // Build iretq frame
        "push {ss}",
        "push {rsp}",
        "push {rflags}",
        "push {cs}",
        "push {rip}",
        
        // Restore segment registers
        "mov ax, {ss}",
        "mov ds, ax",
        "mov es, ax",
        "mov fs, ax",
        "mov gs, ax",
        
        // Restore RAX
        "mov rax, {rax}",
        
        // Jump to Ring 3
        "iretq",
        
        r15 = in(reg) regs.r15,
        r14 = in(reg) regs.r14,
        r13 = in(reg) regs.r13,
        r12 = in(reg) regs.r12,
        rbp = in(reg) regs.rbp,
        rbx = in(reg) regs.rbx,
        r9  = in(reg) regs.r9,
        r8  = in(reg) regs.r8,
        rdx = in(reg) regs.rdx,
        rsi = in(reg) regs.rsi,
        rdi = in(reg) regs.rdi,
        rax = in(reg) regs.rax,
        ss = in(reg) regs.ss,
        rsp = in(reg) regs.rsp,
        rflags = in(reg) regs.rflags,
        cs = in(reg) regs.cs,
        rip = in(reg) regs.rip,
        options(noreturn)
    );
}

/// Check if the current execution context is in Ring 3.
///
/// Returns true if the CPU is currently in user mode (CPL=3).
pub fn is_in_ring3() -> bool {
    let cs: u16;
    unsafe {
        asm!(
            "mov {0:x}, cs",
            out(reg) cs,
            options(nostack, nomem)
        );
    }
    (cs & 0x3) == 3
}

/// Get the current privilege level (CPL).
///
/// Returns 0 for kernel mode, 3 for user mode.
pub fn current_privilege_level() -> u8 {
    let cs: u16;
    unsafe {
        asm!(
            "mov {0:x}, cs",
            out(reg) cs,
            options(nostack, nomem)
        );
    }
    (cs & 0x3) as u8
}
