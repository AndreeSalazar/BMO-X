#![allow(dead_code)]

//! Context switching for FastOS — saves/restores full register state across threads.
//!
//! Kernel stack layout (what the ISR stub pushes, from RSP upward):
//!
//!   For Ring0→Ring0 (APIC timer fires while in kernel):
//!     [rax] [rbx] [rcx] [rdx] [rsi] [rdi] [rbp] [r8]..[r15]    ← 15 GPRs (120 bytes)
//!     [RIP] [CS] [RFLAGS]                                         ← CPU frame (3 values, 24 bytes)
//!
//!   For Ring3→Ring0 (APIC timer fires while in user mode):
//!     [rax] [rbx] [rcx] [rdx] [rsi] [rdi] [rbp] [r8]..[r15]    ← 15 GPRs (120 bytes)
//!     [SS] [RSP] [RFLAGS] [CS] [RIP]                             ← CPU frame (5 values, 40 bytes)
//!
//! This module provides helpers to save/load this layout to/from a `SavedRegs` struct.

use crate::proc::task::SavedRegs;

/// Number of GPRs pushed by the ISR stub.
const GPR_COUNT: usize = 15;
/// Size in bytes of the GPR region on the kernel stack.
const GPR_SIZE: usize = GPR_COUNT * 8;

/// Save the full register ctx from the kernel stack into a `SavedRegs`.
///
/// `saved_state` points to the bottom of the pushed GPRs (where RAX is).
/// The CPU interrupt frame sits above the 15 GPRs.
///
/// Returns `true` if the interrupted code was in Ring 3 (user mode).
#[no_mangle]
pub unsafe extern "C" fn save_context_from_stack(saved_state: *mut u64) -> bool {
    let s = core::slice::from_raw_parts_mut(saved_state, GPR_COUNT + 5);

    let regs = &mut *crate::proc::task::current_ptr();
    let regs = &mut (*regs).regs;

    // GPRs (pushed by our ISR stub, order matches push sequence)
    regs.rax = s[0];
    regs.rbx = s[1];
    regs.rcx = s[2];
    regs.rdx = s[3];
    regs.rsi = s[4];
    regs.rdi = s[5];
    regs.rbp = s[6];
    regs.r8  = s[7];
    regs.r9  = s[8];
    regs.r10 = s[9];
    regs.r11 = s[10];
    regs.r12 = s[11];
    regs.r13 = s[12];
    regs.r14 = s[13];
    regs.r15 = s[14];

    // CPU interrupt frame (sits above the 15 GPRs)
    let cpu_frame = saved_state.add(GPR_COUNT);
    let cs = *cpu_frame.add(3); // CS is at offset 3 for both ring transitions

    let is_ring3 = (cs & 3) == 3;

    if is_ring3 {
        // CPU pushed: [SS] [RSP] [RFLAGS] [CS] [RIP] (5 values)
        regs.ss     = *cpu_frame.add(0);
        regs.rsp    = *cpu_frame.add(1);
        regs.rflags = *cpu_frame.add(2);
        regs.cs     = *cpu_frame.add(3);
        regs.rip    = *cpu_frame.add(4);
    } else {
        // CPU pushed: [RIP] [CS] [RFLAGS] (3 values)
        regs.rip    = *cpu_frame.add(0);
        regs.cs     = *cpu_frame.add(1);
        regs.rflags = *cpu_frame.add(2);
        regs.rsp    = 0; // Not saved by CPU for ring0→ring0
        regs.ss     = 0;
    }

    is_ring3
}

/// Build the kernel stack frame for a thread and return the RSP value for iretq.
///
/// `dst` is the top of the thread's kernel stack (high address).
/// The frame is written just below `dst`, and the returned pointer is where
/// RSP should be set to before the ISR stub does its `pop r15; ... pop rax; iretq`.
///
/// For Ring 0 threads: frame = 15 GPRs + 3 CPU frame values = 18 u64s
/// For Ring 3 threads: frame = 15 GPRs + 5 CPU frame values = 20 u64s
pub unsafe extern "C" fn build_context_on_stack(
    regs: *const SavedRegs,
    stack_top: u64,
) -> u64 {
    let r = &*regs;

    let is_ring3 = (r.cs & 3) == 3;

    let frame_size = if is_ring3 { 20 } else { 18 };
    let frame_bottom = stack_top - (frame_size as u64) * 8;
    let dst = frame_bottom as *mut u64;

    // GPRs (ISR stub will pop these in reverse order)
    *dst.add(0)  = r.rax;
    *dst.add(1)  = r.rbx;
    *dst.add(2)  = r.rcx;
    *dst.add(3)  = r.rdx;
    *dst.add(4)  = r.rsi;
    *dst.add(5)  = r.rdi;
    *dst.add(6)  = r.rbp;
    *dst.add(7)  = r.r8;
    *dst.add(8)  = r.r9;
    *dst.add(9)  = r.r10;
    *dst.add(10) = r.r11;
    *dst.add(11) = r.r12;
    *dst.add(12) = r.r13;
    *dst.add(13) = r.r14;
    *dst.add(14) = r.r15;

    // CPU interrupt frame (IRETQ will pop these)
    if is_ring3 {
        *dst.add(15) = r.ss;
        *dst.add(16) = r.rsp;
        *dst.add(17) = r.rflags;
        *dst.add(18) = r.cs;
        *dst.add(19) = r.rip;
    } else {
        *dst.add(15) = r.rip;
        *dst.add(16) = r.cs;
        *dst.add(17) = r.rflags;
    }

    frame_bottom
}
