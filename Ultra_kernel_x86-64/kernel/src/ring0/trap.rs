//! Unified trap frame — the single context representation for every Ring 0
//! entry (SYSCALL, IRQ, first switch into a task).
//!
//! Both trap entries push 15 GPRs in the same order, then hand control to a
//! Rust dispatcher with a pointer to this frame. Below the frame sits a
//! 512-byte FXSAVE image (16-byte aligned) and a back-pointer slot:
//!
//! ```text
//! high ─►  [ss][rsp][rflags][cs][rip]   trap tail (5 u64)
//!          [rax]...[r15]                15 GPRs (pushed rax first)
//!          gpr_base = &frame
//!          [gpr_base back-ptr]          at fxsave_base + 512
//! low  ─►  [fxsave 512 B, 16-aligned]   fxsave_base = "context rsp"
//! ```
//!
//! A context is fully described by its `fxsave_base`: the shared epilogue
//! does `rsp = fxsave_base; fxrstor; rsp = [rsp+512]; pop 15; iretq`.
//! Context switch = choosing the next `fxsave_base`.

/// Frame passed to Rust dispatchers. Field order matches the push order:
/// `push rax, rcx, rdx, rbx, rbp, rsi, rdi, r8..r15` leaves `r15` lowest.
#[repr(C)]
pub struct TrapFrame {
    pub r15: u64,
    pub r14: u64,
    pub r13: u64,
    pub r12: u64,
    pub r11: u64,
    pub r10: u64,
    pub r9: u64,
    pub r8: u64,
    pub rdi: u64,
    pub rsi: u64,
    pub rbp: u64,
    pub rbx: u64,
    pub rdx: u64,
    pub rcx: u64,
    pub rax: u64,
    pub rip: u64,
    pub cs: u64,
    pub rflags: u64,
    /// Valid only when the trap came from Ring 3 (or was fabricated).
    pub rsp: u64,
    pub ss: u64,
}

pub const RFLAGS_IF: u64 = 1 << 9;

pub const KERNEL_CS: u64 = 0x08;
pub const KERNEL_SS: u64 = 0x08;
pub const USER_CS: u64 = 0x23;
pub const USER_SS: u64 = 0x1B;

const FXSAVE_SIZE: usize = 512;
/// GPR block (15*8) + back-pointer slot (8) + alignment slack (8).
const FRAME_BYTES_BELOW_TAIL: usize = 15 * 8 + 8 + 8;
/// Bytes consumed from the stack top: tail (40) + GPRs + back-ptr + fxsave
/// + alignment slack. 720 B fits comfortably in the 8 KiB task stack.
const CONTEXT_BYTES: usize = 40 + FRAME_BYTES_BELOW_TAIL + FXSAVE_SIZE + 8;

/// Fabricate the initial context of a task on its own kernel stack.
/// Returns the `fxsave_base` that becomes the task's `context_rsp`.
///
/// `user` selects a Ring 3 frame (iretq switches stack + CPL); kernel tasks
/// get a same-privilege frame whose post-iretq RSP lands 8 mod 16, matching
/// the SysV post-`call` alignment Rust code expects.
pub unsafe fn fabricate(
    stack_top: u64,
    entry: u64,
    arg: u64,
    user: bool,
    user_rsp: u64,
) -> u64 {
    let gpr_base = (stack_top - 168) & !0xF | 0x8; // ≡ 8 (mod 16)
    let fxsave_base = gpr_base - 552; // ≡ 0 (mod 16)

    core::ptr::write_bytes(fxsave_base as *mut u8, 0, FXSAVE_SIZE);
    // Valid legacy x87/SSE image: FXRSTOR faults on garbage, zeros are not
    // enough — FCW and MXCSR must hold architecturally sane values.
    (fxsave_base as *mut u16).write_volatile(0x037F); // FCW
    ((fxsave_base + 24) as *mut u32).write_volatile(0x1F80); // MXCSR
    ((fxsave_base + FXSAVE_SIZE as u64) as *mut u64).write_volatile(gpr_base); // back-ptr

    let frame = &mut *(gpr_base as *mut TrapFrame);
    core::ptr::write_bytes(frame as *mut TrapFrame as *mut u8, 0, core::mem::size_of::<TrapFrame>());
    frame.rdi = arg;
    frame.rip = entry;
    frame.rflags = RFLAGS_IF | 0x2;
    if user {
        frame.cs = USER_CS;
        frame.ss = USER_SS;
        frame.rsp = user_rsp;
    } else {
        frame.cs = KERNEL_CS;
        frame.ss = KERNEL_SS;
        frame.rsp = gpr_base + 144; // informational (same-CPL iretq ignores it)
    }
    fxsave_base
}

/// Minimum kernel stack size that fits one fabricated context plus working
/// room for the task's own frames.
pub const MIN_TASK_STACK: usize = CONTEXT_BYTES + 4096;
