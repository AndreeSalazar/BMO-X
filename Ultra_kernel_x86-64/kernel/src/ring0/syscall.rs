//! x86-64 SYSCALL entry and minimal BMO ABI dispatcher.
//!
//! The entry builds the unified trap frame (see trap.rs): swapgs, switch to
//! the per-CPU syscall stack, synthesize the 5-word trap tail (user SS/RSP/
//! RFLAGS/CS/RIP from the SYSCALL contract), push 15 GPRs, FXSAVE, then call
//! the Rust dispatcher with the frame pointer.
//!
//! Return is via `iretq`, never `sysretq`: one return path for traps and
//! syscalls, no non-canonical-RCX #GP hazard in Ring 0, and — critically —
//! the dispatcher may answer with a *different* context than the one that
//! entered (YIELD/WAIT/EXIT switch right at the syscall boundary).

use core::arch::{asm, naked_asm};

use crate::ring0::percpu;
use crate::ring0::scheduler;
use crate::ring0::trap::TrapFrame;

// Minimal no-alloc view of the canonical bmo-abi syscall contract. Keeping
// these values here avoids linking the full alloc-using BEF/ABI implementation
// into Ring 0; build.ps1 rejects values that drift from bmo-abi.
const NR_INVOKE: u32 = 0x00;
const NR_CHANNEL_KICK: u32 = 0x01;
const NR_WAIT: u32 = 0x02;
const CURRENT_TASK: u64 = 0xFFFF_FFFF_FFFF_FFFE;
const TASK_OP_GET_PID: u64 = 0x01;
const TASK_OP_GET_TID: u64 = 0x02;
const TASK_OP_YIELD: u64 = 0x03;
const TASK_OP_EXIT: u64 = 0x04;
const NR_PROC_GET_PID: u32 = 0x182;
const NR_PROC_GET_TID: u32 = 0x183;
const NR_PROC_YIELD: u32 = 0x184;
const NR_THREAD_SELF: u32 = 0x188;
const NR_BEFCORE_POLL: u32 = 0x196;
const ERROR_UNSUPPORTED: u32 = 10;

#[repr(C)]
struct BmoStatus {
    code: u32,
    flags: u32,
    value: u64,
}

impl BmoStatus {
    const fn ok_value(value: u64) -> Self { Self { code: 0, flags: 0, value } }
    const fn err(code: u32) -> Self { Self { code, flags: 0, value: 0 } }
}

const _: () = assert!(core::mem::size_of::<BmoStatus>() == 16);

const MSR_STAR: u32 = 0xC000_0081;
const MSR_LSTAR: u32 = 0xC000_0082;
const MSR_SFMASK: u32 = 0xC000_0084;
const RFLAGS_TF: u64 = 1 << 8;
const RFLAGS_IF: u64 = 1 << 9;
const RFLAGS_DF: u64 = 1 << 10;
const RFLAGS_NT: u64 = 1 << 14;
const RFLAGS_AC: u64 = 1 << 18;
const KERNEL_CS: u64 = 0x08;
// Legacy STAR layout; kept armed although the exit path is iretq-only.
const SYSRET_SELECTOR_BASE: u64 = 0x10;

#[unsafe(naked)]
unsafe extern "C" fn syscall_entry() -> ! {
    naked_asm!(
        "swapgs",
        "mov gs:[0x08], rsp",          // stash user RSP
        "mov rsp, gs:[0x00]",          // per-CPU syscall stack
        // Trap tail: ss, rsp, rflags, cs, rip (SYSCALL contract values).
        "push 0x1B",                   // user SS
        "push qword ptr gs:[0x08]",    // user RSP
        "push r11",                    // user RFLAGS
        "push 0x23",                   // user CS
        "push rcx",                    // user RIP
        // 15 GPRs (push order; pops restore the reverse).
        "push rax", "push rcx", "push rdx", "push rbx", "push rbp",
        "push rsi", "push rdi", "push r8", "push r9", "push r10",
        "push r11", "push r12", "push r13", "push r14", "push r15",
        "mov rbp, rsp",
        "sub rsp, 544",
        "and rsp, -16",
        "mov [rsp+512], rbp",          // back-pointer to the GPR block
        "fxsave64 [rsp]",
        "mov gs:[0x10], rsp",          // publish this context
        "cld",
        "mov rdi, rbp",
        "call {dispatch}",
        // Shared trap epilogue: rax = fxsave-base of the context to run.
        "mov rsp, rax",
        "fxrstor64 [rsp]",
        "mov rsp, [rsp+512]",
        "pop r15", "pop r14", "pop r13", "pop r12", "pop r11",
        "pop r10", "pop r9", "pop r8", "pop rdi", "pop rsi",
        "pop rbp", "pop rbx", "pop rdx", "pop rcx", "pop rax",
        "cmp qword ptr [rsp+8], 0x08", // returning to kernel CS?
        "je 1f",
        "swapgs",
        "1: iretq",
        dispatch = sym dispatch,
    );
}

unsafe fn wrmsr(msr: u32, value: u64) {
    asm!(
        "wrmsr",
        in("ecx") msr,
        in("eax") value as u32,
        in("edx") (value >> 32) as u64,
        options(nostack),
    );
}

pub fn init() {
    let star = (SYSRET_SELECTOR_BASE << 48) | (KERNEL_CS << 32);
    unsafe {
        wrmsr(MSR_STAR, star);
        wrmsr(MSR_LSTAR, syscall_entry as *const () as u64);
        // Do not let hostile user flags trigger #DB/#AC before the entry stub
        // has switched away from the user stack. Interrupts stay masked for
        // the whole dispatch; the iretq restores the user IF.
        wrmsr(
            MSR_SFMASK,
            RFLAGS_TF | RFLAGS_IF | RFLAGS_DF | RFLAGS_NT | RFLAGS_AC,
        );
    }
}

#[inline]
fn unsupported() -> BmoStatus {
    BmoStatus::err(ERROR_UNSUPPORTED)
}

fn invoke_current_task(operation: u64, arg0: u64) -> BmoStatus {
    match operation {
        TASK_OP_GET_PID => BmoStatus::ok_value(scheduler::current_pid() as u64),
        TASK_OP_GET_TID => BmoStatus::ok_value(scheduler::current_tid() as u64),
        // These switch at the syscall boundary; when (if) this context runs
        // again it resumes here and reports success.
        TASK_OP_YIELD => {
            scheduler::yield_current();
            BmoStatus::ok_value(0)
        }
        TASK_OP_EXIT => {
            let _ = arg0;
            scheduler::exit_current();
            BmoStatus::ok_value(0)
        }
        _ => unsupported(),
    }
}

fn invoke(frame: &TrapFrame) -> BmoStatus {
    if frame.rdi != CURRENT_TASK {
        return unsupported();
    }
    invoke_current_task(frame.rsi, frame.rdx)
}

#[unsafe(no_mangle)]
extern "C" fn dispatch(frame: &mut TrapFrame) -> u64 {
    let status = match frame.rax as u32 {
        NR_INVOKE => invoke(frame),
        // Recognized V2 boundary. It becomes operational with
        // capability-backed channels (F2).
        NR_CHANNEL_KICK => unsupported(),
        // WAIT(waitable, observed_seq, absolute_deadline_ns): block until the
        // waitable's sequence moves past observed or the deadline expires.
        NR_WAIT => {
            let deadline_ns = frame.rdx;
            let deadline = if deadline_ns == 0 {
                0
            } else {
                scheduler::rdtsc() + scheduler::ns_to_tsc(deadline_ns)
            };
            scheduler::wait_current(frame.rdi, deadline);
            BmoStatus::ok_value(0)
        }
        // Temporary ABI v1 adapter. All task behavior still flows through
        // the canonical v2 operation dispatcher above.
        NR_PROC_GET_PID => invoke_current_task(TASK_OP_GET_PID, 0),
        NR_PROC_GET_TID | NR_THREAD_SELF => invoke_current_task(TASK_OP_GET_TID, 0),
        NR_PROC_YIELD => invoke_current_task(TASK_OP_YIELD, 0),
        NR_BEFCORE_POLL => {
            BmoStatus::ok_value(crate::ring0::channel::service_all() as u64)
        }
        _ => unsupported(),
    };
    frame.rax = (status.code as u64) | ((status.flags as u64) << 32);
    frame.rdx = status.value;
    percpu::trap_rsp()
}
