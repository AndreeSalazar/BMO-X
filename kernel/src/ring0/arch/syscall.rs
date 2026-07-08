//! Syscall — x86-64 syscall/sysret entry + jump-table dispatcher.
//!
//! ## Architecture
//!   - Naked entry saves regs → builds InterruptFrame
//!   - Rust handler reads RAX (syscall nr) → indexes into SYSCALL_TABLE
//!   - Jump table provides O(1) dispatch (branchless after bounds check)
//!   - Feature-gated syscall groups (net, gpu, ipc, audio) compile to
//!     `u64::MAX` when not enabled → LTO eliminates dead branches
//!
//! ## vDSO fast path
//!   - SYS_CLOCK_GET (0x50) reads directly from vDSO page when available
//!   - vDSO page is mapped read-only into every Ring 3 process
//!   - Timer IRQ updates vdso.monotonic_tsc and vdso.realtime_ns
//!
//! ## Syscall table layout
//!   0x00-0x0F: Process / Task
//!   0x10-0x1F: Memory
//!   0x20-0x2F: Filesystem (stubs)
//!   0x30-0x3F: IPC (stubs)
//!   0x40-0x4F: BareX bridges (stubs)
//!   0x50-0x5F: Time
//!   0x60-0x6F: Framebuffer
//!   0x70-0x7F: Input (stubs)
//!   0x80-0x8F: Audio (stubs)
//!   0x90-0x9F: Network (stubs)
//!   0xA0-0xAF: GPU (stubs)
//!   0xF0-0xFF: Debug

#![allow(dead_code, static_mut_refs)]

use core::arch::{asm, naked_asm};
use core::sync::atomic::{AtomicBool, Ordering};

// ── MSR addresses ──────────────────────────────────────────────────────

const IA32_STAR: u32           = 0xC000_0081;
const IA32_LSTAR: u32          = 0xC000_0082;
const IA32_FMASK: u32          = 0xC000_0084;
const IA32_EFER: u32           = 0xC000_0080;
const IA32_GS_BASE: u32        = 0xC000_0101;
const IA32_KERNEL_GS_BASE: u32 = 0xC000_0102;

const KERNEL_CS_SELECTOR: u64 = 0x08;
const KERNEL_DS_SELECTOR: u64 = 0x10;

// ── MSR helpers ────────────────────────────────────────────────────────

#[inline]
unsafe fn wrmsr(msr: u32, val: u64) {
    let lo = val as u32;
    let hi = (val >> 32) as u32;
    asm!("wrmsr", in("ecx") msr, in("eax") lo, in("edx") hi, options(nostack));
}

#[inline]
unsafe fn rdmsr(msr: u32) -> u64 {
    let (lo, hi): (u32, u32);
    asm!("rdmsr", in("ecx") msr, out("eax") lo, out("edx") hi, options(nostack));
    ((hi as u64) << 32) | (lo as u64)
}

// ── Syscall handler type ───────────────────────────────────────────────

/// A syscall handler receives: (nr, a0, a1, a2, a3, a4, a5) → result.
pub type SyscallFn = fn(u64, u64, u64, u64, u64, u64, u64) -> u64;

// ── Init ───────────────────────────────────────────────────────────────

static mut SYSCALL_KERNEL_RSP: u64 = 0;

pub fn init_syscall() {
    unsafe {
        let efer = rdmsr(IA32_EFER);
        wrmsr(IA32_EFER, efer | 1);

        let star = (KERNEL_DS_SELECTOR << 48) | (KERNEL_CS_SELECTOR << 32);
        wrmsr(IA32_STAR, star);
        wrmsr(IA32_LSTAR, syscall_entry_naked as *const () as u64);
        wrmsr(IA32_FMASK, (1 << 9) | (1 << 10));
        wrmsr(IA32_KERNEL_GS_BASE, 0);
        wrmsr(IA32_GS_BASE, 0);

        let stack_top = crate::arch::gdt::kernel_stack_top();
        SYSCALL_KERNEL_RSP = stack_top;
    }

    let real_entry = syscall_entry_naked as *const () as u64;
    crate::vendor::amd::cpu::zen3::init_msrs(real_entry);
}

pub fn set_syscall_kernel_stack(rsp: u64) {
    unsafe { SYSCALL_KERNEL_RSP = rsp; }
}

// ── InterruptFrame (saved user context) ────────────────────────────────

#[repr(C, packed)]
#[derive(Clone, Copy, Debug)]
pub struct InterruptFrame {
    pub r15: u64, pub r14: u64, pub r13: u64, pub r12: u64,
    pub rbp: u64, pub rbx: u64,
    pub r9: u64,  pub r8: u64,  pub r10: u64,
    pub rdx: u64, pub rsi: u64, pub rdi: u64,
    pub rax: u64,
    pub rip: u64,  pub cs: u64,  pub rflags: u64,
    pub rsp: u64,  pub ss: u64,
}

// ── Naked entry point ──────────────────────────────────────────────────

#[unsafe(naked)]
unsafe extern "C" fn syscall_entry_naked() {
    naked_asm!(
        "swapgs",
        "mov r15, rsp",
        "mov rsp, [rip + {kstack}]",
        "push qword ptr 0x1B",
        "push r15",
        "push r11",
        "push qword ptr 0x23",
        "push rcx",
        "push rax",
        "push rdi",
        "push rsi",
        "push rdx",
        "push r10",
        "push r8",
        "push r9",
        "push rbx",
        "push rbp",
        "push r12",
        "push r13",
        "push r14",
        "push r15",
        "mov rdi, rsp",
        "call {handler}",
        "pop r15",
        "pop r14",
        "pop r13",
        "pop r12",
        "pop rbp",
        "pop rbx",
        "pop r9",
        "pop r8",
        "pop r10",
        "pop rdx",
        "pop rsi",
        "pop rdi",
        "pop rax",
        "swapgs",
        "iretq",
        kstack = sym SYSCALL_KERNEL_RSP,
        handler = sym syscall_handler_rust,
    );
}

// ── Ring 3 alive detection ─────────────────────────────────────────────

static RING3_ALIVE: AtomicBool = AtomicBool::new(false);

pub fn ring3_alive() -> bool {
    RING3_ALIVE.load(Ordering::Relaxed)
}

// ── Stub handler (returns u64::MAX = -1 = unsupported) ─────────────────

fn stub(_nr: u64, _a0: u64, _a1: u64, _a2: u64, _a3: u64, _a4: u64, _a5: u64) -> u64 {
    u64::MAX
}

// ── Syscall implementations ────────────────────────────────────────────

fn sys_exit(nr: u64, a0: u64, _a1: u64, _a2: u64, _a3: u64, _a4: u64, _a5: u64) -> u64 {
    u64::MAX // process exit — module handles this
}

fn sys_yield(_nr: u64, _a0: u64, _a1: u64, _a2: u64, _a3: u64, _a4: u64, _a5: u64) -> u64 {
    crate::proc::yield_now();
    0
}

fn sys_task_alloc(nr: u64, a0: u64, a1: u64, _a2: u64, _a3: u64, _a4: u64, _a5: u64) -> u64 {
    match crate::proc::task::alloc(
        crate::proc::process::Pid(a0 as u32),
        crate::proc::Priority::Interactive,
    ) {
        Some(thr) => {
            thr.regs = crate::proc::task::SavedRegs::new_user(a1, 0);
            thr.state = crate::proc::task::State::Ready;
            thr.tid.0 as u64
        }
        None => u64::MAX,
    }
}

fn sys_mmap(nr: u64, cr3: u64, virt: u64, phys: u64, pages: u64, flags: u64, _a5: u64) -> u64 {
    match unsafe { crate::mm::vmm::map_user_range(cr3, virt, phys, pages as usize, flags) } {
        Ok(()) => 0,
        Err(_) => u64::MAX,
    }
}

fn sys_clock_get(_nr: u64, _a0: u64, _a1: u64, _a2: u64, _a3: u64, _a4: u64, _a5: u64) -> u64 {
    crate::cpu::rdtsc()
}

fn sys_nanosleep(nr: u64, a0: u64, _a1: u64, _a2: u64, _a3: u64, _a4: u64, _a5: u64) -> u64 {
    let target_cycles = (a0 as u128 * 37) / 10;
    let start = crate::cpu::rdtsc();
    while (crate::cpu::rdtsc() - start) < target_cycles as u64 {
        core::hint::spin_loop();
    }
    0
}

fn sys_fb_info(_nr: u64, _a0: u64, _a1: u64, _a2: u64, _a3: u64, _a4: u64, _a5: u64) -> u64 {
    let w = unsafe { crate::info::FB_WIDTH as u64 };
    let h = unsafe { crate::info::FB_HEIGHT as u64 };
    let s = unsafe { crate::info::FB_STRIDE as u64 };
    let addr = unsafe { crate::info::FB_ADDR };
    addr | (w << 32) | ((h & 0xFFFF) << 16) | ((s & 0xFF) << 48)
}

fn sys_debug_print(nr: u64, a0: u64, a1: u64, _a2: u64, _a3: u64, _a4: u64, _a5: u64) -> u64 {
    if a1 > 0 && a1 < 4096 {
        let slice = unsafe { core::slice::from_raw_parts(a0 as *const u8, a1 as usize) };
        if let Ok(s) = core::str::from_utf8(slice) {
            crate::dev::console::serial_write(s);
        }
    }
    0
}

// ── Port I/O (for Ring 3 drivers: PS/2 keyboard, ATA, etc.) ────────────

fn sys_port_in(_nr: u64, port: u64, _a1: u64, _a2: u64, _a3: u64, _a4: u64, _a5: u64) -> u64 {
    if port > 0xFFFF { return u64::MAX; }
    unsafe {
        let val: u8;
        core::arch::asm!("in al, dx", in("dx") port as u16, out("al") val, options(nostack, nomem));
        val as u64
    }
}

fn sys_port_out(_nr: u64, port: u64, val: u64, _a2: u64, _a3: u64, _a4: u64, _a5: u64) -> u64 {
    if port > 0xFFFF { return u64::MAX; }
    unsafe {
        core::arch::asm!("out dx, al", in("dx") port as u16, in("al") val as u8, options(nostack, nomem));
    }
    0
}

// crate::info is a module, not a struct. Access statics directly.

// ── Network syscalls (gated) ─────────────────────────────────────────

#[cfg(feature = "syscalls-net")]
fn sys_net_send(nr: u64, a0: u64, a1: u64, _a2: u64, _a3: u64, _a4: u64, _a5: u64) -> u64 {
    u64::MAX // NIC driver in kernel, TCP/IP in Ring 3
}

#[cfg(feature = "syscalls-gpu")]
fn sys_gpu_submit(nr: u64, a0: u64, a1: u64, _a2: u64, _a3: u64, _a4: u64, _a5: u64) -> u64 {
    u64::MAX // GPU command buffer submission
}

#[cfg(feature = "syscalls-ipc")]
fn sys_ipc_send(_nr: u64, dst_pid: u64, msg_ptr: u64, msg_len: u64, _a3: u64, _a4: u64, _a5: u64) -> u64 {
    if msg_len == 0 || msg_len > 65536 || msg_ptr == 0 { return u64::MAX; }
    let dst = match crate::proc::process::get_process(crate::proc::process::Pid(dst_pid as u32)) {
        Some(p) => p as *mut crate::proc::process::Process,
        None => return u64::MAX,
    };
    unsafe {
        // Allocate message node on heap
        let layout = core::alloc::Layout::new::<crate::proc::process::MsgNode>();
        let node = crate::mm::slab::heap_alloc(layout.size(), layout.align())
            as *mut crate::proc::process::MsgNode;
        if node.is_null() { return u64::MAX; }

        // Allocate data buffer
        let data = crate::mm::slab::heap_alloc(msg_len as usize, 8);
        if data.is_null() {
            crate::mm::slab::heap_free(node as *mut u8, layout.size(), layout.align());
            return u64::MAX;
        }

        core::ptr::copy_nonoverlapping(msg_ptr as *const u8, data, msg_len as usize);
        (*node).next = core::ptr::null_mut();
        (*node).data = data;
        (*node).len = msg_len as usize;

        let dst_ref = &mut *dst;
        if dst_ref.msg_tail.is_null() {
            dst_ref.msg_head = node;
            dst_ref.msg_tail = node;
        } else {
            (*dst_ref.msg_tail).next = node;
            dst_ref.msg_tail = node;
        }
    }
    0
}

#[cfg(feature = "syscalls-ipc")]
fn sys_ipc_recv(_nr: u64, buf_ptr: u64, buf_len: u64, _a2: u64, _a3: u64, _a4: u64, _a5: u64) -> u64 {
    if buf_ptr == 0 || buf_len == 0 { return u64::MAX; }
    let cur = match crate::proc::task::current() {
        Some(t) => t,
        None => return u64::MAX,
    };
    let proc = match crate::proc::process::get_process(cur.pid) {
        Some(p) => p,
        None => return u64::MAX,
    };
    if proc.msg_head.is_null() { return 0; } // no messages (non-blocking for now)

    unsafe {
        let node = proc.msg_head;
        proc.msg_head = (*node).next;
        if proc.msg_head.is_null() { proc.msg_tail = core::ptr::null_mut(); }

        let len = (*node).len.min(buf_len as usize);
        core::ptr::copy_nonoverlapping((*node).data, buf_ptr as *mut u8, len);

        // Free
        let data_layout = core::alloc::Layout::from_size_align_unchecked((*node).len, 8);
        crate::mm::slab::heap_free((*node).data, data_layout.size(), data_layout.align());
        let node_layout = core::alloc::Layout::new::<crate::proc::process::MsgNode>();
        crate::mm::slab::heap_free(node as *mut u8, node_layout.size(), node_layout.align());

        len as u64
    }
}

#[cfg(not(feature = "syscalls-ipc"))]
fn sys_ipc_send(_nr: u64, _a0: u64, _a1: u64, _a2: u64, _a3: u64, _a4: u64, _a5: u64) -> u64 { stub(_nr, _a0, _a1, _a2, _a3, _a4, _a5) }

#[cfg(not(feature = "syscalls-ipc"))]
fn sys_ipc_recv(_nr: u64, _a0: u64, _a1: u64, _a2: u64, _a3: u64, _a4: u64, _a5: u64) -> u64 { stub(_nr, _a0, _a1, _a2, _a3, _a4, _a5) }

// ── Jump table (256 entries, lazy-initialized once) ────────────────────

static mut SYSCALL_TABLE: [SyscallFn; 256] = [stub as SyscallFn; 256];
static mut TABLE_INIT: bool = false;

unsafe fn init_table() {
    if TABLE_INIT { return; }

    // Base syscalls (always present)
    SYSCALL_TABLE[0x00] = sys_exit;
    SYSCALL_TABLE[0x03] = sys_yield;
    SYSCALL_TABLE[0x04] = sys_task_alloc;
    SYSCALL_TABLE[0x10] = sys_mmap;
    SYSCALL_TABLE[0x11] = stub; // munmap
    SYSCALL_TABLE[0x12] = stub; // mprotect
    SYSCALL_TABLE[0x50] = sys_clock_get;
    SYSCALL_TABLE[0x51] = sys_nanosleep;
    SYSCALL_TABLE[0x60] = sys_fb_info;
    SYSCALL_TABLE[0x61] = stub; // fb_map
    SYSCALL_TABLE[0x62] = stub; // fb_flush
    SYSCALL_TABLE[0x70] = sys_port_in;
    SYSCALL_TABLE[0x71] = sys_port_out;
    SYSCALL_TABLE[0xF0] = sys_debug_print;

    // Network (gated)
    #[cfg(feature = "syscalls-net")] {
        SYSCALL_TABLE[0x90] = sys_net_send;
    }

    // GPU (gated)
    #[cfg(feature = "syscalls-gpu")] {
        SYSCALL_TABLE[0xA0] = sys_gpu_submit;
    }

    // IPC (gated)
    #[cfg(feature = "syscalls-ipc")] {
        SYSCALL_TABLE[0x30] = sys_ipc_send;
        SYSCALL_TABLE[0x31] = sys_ipc_recv;
    }

    TABLE_INIT = true;
}

// ── Rust handler (dispatches via jump table) ─────────────────────────

#[unsafe(no_mangle)]
extern "C" fn syscall_handler_rust(frame: *mut InterruptFrame) {
    unsafe {
        if !RING3_ALIVE.load(Ordering::Relaxed) {
            RING3_ALIVE.store(true, Ordering::Relaxed);
            crate::dev::console::serial_write("[syscall] first syscall received; Ring 3 is alive\n");
        }

        // Lazy init the jump table (first syscall)
        init_table();

        let f = &mut *frame;
        let nr = f.rax as usize;
        let a0 = f.rdi;
        let a1 = f.rsi;
        let a2 = f.rdx;
        let a3 = f.r10;
        let a4 = f.r8;
        let a5 = f.r9;

        // O(1) dispatch via jump table (bounds-checked)
        let handler = if nr < 256 {
            SYSCALL_TABLE[nr]
        } else {
            stub
        };

        f.rax = handler(nr as u64, a0, a1, a2, a3, a4, a5);
    }
}

// ── Syscall number constants (public API for userland) ─────────────────

pub const SYS_EXIT: u64         = 0x00;
pub const SYS_YIELD: u64        = 0x03;
pub const SYS_TASK_ALLOC: u64   = 0x04;
pub const SYS_MMAP: u64         = 0x10;
pub const SYS_MUNMAP: u64       = 0x11;
pub const SYS_MPROTECT: u64     = 0x12;
pub const SYS_CLOCK_GET: u64    = 0x50;
pub const SYS_NANOSLEEP: u64    = 0x51;
pub const SYS_FB_INFO: u64      = 0x60;
pub const SYS_FB_MAP: u64       = 0x61;
pub const SYS_FB_FLUSH: u64     = 0x62;
pub const SYS_PORT_IN: u64      = 0x70;
pub const SYS_PORT_OUT: u64     = 0x71;
pub const SYS_NET_SEND: u64     = 0x90;
pub const SYS_GPU_SUBMIT: u64   = 0xA0;
pub const SYS_IPC_SEND: u64     = 0x30;
pub const SYS_IPC_RECV: u64     = 0x31;
pub const SYS_DEBUG_PRINT: u64  = 0xF0;
