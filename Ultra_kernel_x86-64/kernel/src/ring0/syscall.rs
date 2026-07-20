//! x86-64 SYSCALL entry and minimal BMO ABI dispatcher.
//!
//! BSP-only for now: the dedicated stack and saved user RSP become per-CPU
//! storage before application processors are enabled.

use core::arch::{asm, naked_asm};

// Minimal no-alloc view of the canonical bmo-abi syscall contract. Keeping
// these values here avoids linking the full alloc-using BEF/ABI implementation
// into Ring 0; build.ps1 rejects values that drift from bmo-abi.
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
// SYSRET derives SS=base+8 and CS=base+16. A base of 0x10 selects
// user DS=0x1b and user CS=0x23 after the CPU adds RPL 3.
const SYSRET_SELECTOR_BASE: u64 = 0x10;
const SYSCALL_STACK_SIZE: usize = 32 * 1024;

#[repr(C, align(64))]
struct SyscallStack([u8; SYSCALL_STACK_SIZE]);

static mut SYSCALL_STACK: SyscallStack = SyscallStack([0; SYSCALL_STACK_SIZE]);
static mut SYSCALL_USER_RSP: u64 = 0;

#[repr(C)]
struct SyscallFrame {
    number: u64,
    arg0: u64,
    arg1: u64,
    arg2: u64,
    arg3: u64,
    arg4: u64,
    arg5: u64,
    user_rip: u64,
    user_rflags: u64,
}

#[unsafe(naked)]
unsafe extern "C" fn syscall_entry() -> ! {
    naked_asm!(
        // SYSCALL leaves the user stack active. Save it without touching it,
        // then move to memory owned by Ring 0 before the first push.
        "mov [rip + {user_rsp}], rsp",
        "lea rsp, [rip + {stack}]",
        "add rsp, {stack_size}",

        // Build SyscallFrame in reverse order. RCX and R11 contain the return
        // RIP and RFLAGS by architectural definition.
        "push r11",
        "push rcx",
        "push r9",
        "push r8",
        "push r10",
        "push rdx",
        "push rsi",
        "push rdi",
        "push rax",

        // Nine pushes leave RSP at 8 mod 16; reserve eight bytes so the Rust
        // dispatcher is called with the SysV pre-call alignment.
        "sub rsp, 8",
        "lea rdi, [rsp + 8]",
        "call {dispatch}",
        "add rsp, 8",

        // BmoStatus returns in RAX:RDX. Restore argument registers but skip
        // the saved RAX and RDX so the status is not overwritten.
        "add rsp, 8",
        "pop rdi",
        "pop rsi",
        "add rsp, 8",
        "pop r10",
        "pop r8",
        "pop r9",
        "pop rcx",
        "pop r11",
        "mov rsp, [rip + {user_rsp}]",
        "sysretq",
        user_rsp = sym SYSCALL_USER_RSP,
        stack = sym SYSCALL_STACK,
        stack_size = const SYSCALL_STACK_SIZE,
        dispatch = sym dispatch,
    );
}

unsafe fn wrmsr(msr: u32, value: u64) {
    asm!(
        "wrmsr",
        in("ecx") msr,
        in("eax") value as u32,
        in("edx") (value >> 32) as u32,
        options(nostack),
    );
}

pub fn init() {
    let star = (SYSRET_SELECTOR_BASE << 48) | (KERNEL_CS << 32);
    unsafe {
        wrmsr(MSR_STAR, star);
        wrmsr(MSR_LSTAR, syscall_entry as *const () as u64);
        // Do not let hostile user flags trigger #DB/#AC before the entry stub
        // has switched away from the user stack. Interrupts remain disabled
        // for this BSP-only, non-reentrant dispatcher.
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

#[unsafe(no_mangle)]
extern "C" fn dispatch(frame: &SyscallFrame) -> BmoStatus {
    match frame.number as u32 {
        NR_PROC_GET_PID => BmoStatus::ok_value(crate::ring0::scheduler::current_pid() as u64),
        NR_PROC_GET_TID | NR_THREAD_SELF => {
            BmoStatus::ok_value(crate::ring0::scheduler::current_tid() as u64)
        }
        // The scheduler can select a next task, but yielding cannot claim
        // success until the context-switch layer actually installs it.
        NR_PROC_YIELD => unsupported(),
        NR_BEFCORE_POLL => {
            BmoStatus::ok_value(crate::ring0::channel::service_all() as u64)
        }
        _ => unsupported(),
    }
}
