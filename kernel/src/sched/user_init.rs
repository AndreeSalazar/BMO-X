//! User-space initialization — first Ring 3 process.
//!
//! Creates a minimal "init" process that runs in Ring 3 and demonstrates
//! syscall functionality (DebugPrint, ClockGetTime).

#![allow(dead_code)]

use super::process;
use super::thread;
use super::Priority;
use crate::sandbox::Capability;
use crate::arch::paging;

/// Size of user stack (64 KB).
const USER_STACK_SIZE: usize = 65536;
/// Base virtual address for user code (low half of address space).
const USER_CODE_BASE: u64 = 0x0040_0000;
/// Base virtual address for user stack.
const USER_STACK_BASE: u64 = 0x007F_0000;

/// Kernel stack per thread (8 KB each).
const KERNEL_STACK_PER_THREAD: usize = 8192;

/// A small user-mode program (x86-64 machine code) that:
///   1. Calls syscall DebugPrint to print "Hello from Ring 3!\n"
///   2. Calls syscall ClockGetTime  
///   3. Calls syscall ProcessExit(0)
///
/// BMO ABI syscall: RAX=nr, RDI=a0, RSI=a1 → `syscall`
fn build_init_program() -> &'static [u8] {
    static INIT_CODE: [u8; 79] = [
        // === Print "Hello from Ring 3!\n" via DebugPrint (syscall 0xF0) ===
        // lea rdi, [rip + message]  ; a0 = pointer to string
        0x48, 0x8D, 0x3D, 0x2E, 0x00, 0x00, 0x00,   // lea rdi, [rip+46]
        // mov rsi, 19              ; a1 = string length
        0x48, 0xC7, 0xC6, 0x13, 0x00, 0x00, 0x00,
        // mov rax, 0xF0            ; syscall number = DebugPrint
        0x48, 0xC7, 0xC0, 0xF0, 0x00, 0x00, 0x00,
        // syscall
        0x0F, 0x05,

        // === Get time via ClockGetTime (syscall 0x50) ===
        // mov rax, 0x50
        0x48, 0xC7, 0xC0, 0x50, 0x00, 0x00, 0x00,
        // syscall
        0x0F, 0x05,

        // === Infinite loop with yield (syscall 0x03) ===
        0x48, 0xC7, 0xC0, 0x03, 0x00, 0x00, 0x00,   // mov rax, 0x03
        0x0F, 0x05,                                     // syscall
        0xEB, 0xF5,                                     // jmp -11

        // === Exit via ProcessExit (syscall 0x00) — unreachable ===
        0x48, 0xC7, 0xC7, 0x00, 0x00, 0x00, 0x00,
        0x48, 0xC7, 0xC0, 0x00, 0x00, 0x00, 0x00,
        0x0F, 0x05,
        0xF4,

        // === Data: "Hello from Ring 3!\n" (19 bytes) ===
        b'H', b'e', b'l', b'l', b'o', b' ', b'f', b'r', b'o', b'm',
        b' ', b'R', b'i', b'n', b'g', b' ', b'3', b'!', b'\n',
    ];
    &INIT_CODE
}

fn allocate_user_process(name: &str, code: &[u8], caps: Capability) -> Option<(u64, u64)> {
    let proc = process::alloc_process()?;
    proc.set_name(name);
    proc.entry_point = USER_CODE_BASE;
    proc.caps = caps;

    proc.page_table_root = paging::read_cr3();

    unsafe {
        let dst = USER_CODE_BASE as *mut u8;
        core::ptr::copy_nonoverlapping(code.as_ptr(), dst, code.len());
    }

    unsafe {
        let stack_base = USER_STACK_BASE as *mut u8;
        core::ptr::write_bytes(stack_base, 0, USER_STACK_SIZE);
    }
    let user_stack_top = USER_STACK_BASE + USER_STACK_SIZE as u64;

    let kernel_stack = unsafe {
        let layout = core::alloc::Layout::from_size_align(KERNEL_STACK_PER_THREAD, 16).unwrap();
        let ptr = alloc::alloc::alloc_zeroed(layout);
        if ptr.is_null() { return None; }
        ptr as u64 + KERNEL_STACK_PER_THREAD as u64
    };

    let thr = thread::alloc_thread(proc.pid, Priority::Interactive)?;
    thr.regs = thread::SavedRegs::new_user(USER_CODE_BASE, user_stack_top);
    thr.kernel_stack_top = kernel_stack;
    thr.state = thread::ThreadState::Ready;

    let tid = thr.tid;
    if let Some(idx) = thread::find_thread_index(tid) {
        thread::set_current(idx);
        if let Some(t) = thread::get_thread(idx) {
            t.state = thread::ThreadState::Running;
        }
    }

    crate::arch::gdt::set_kernel_stack(kernel_stack);
    crate::arch::syscall_entry::set_syscall_kernel_stack(kernel_stack);

    Some((USER_CODE_BASE, user_stack_top))
}

/// Spawn the first user-mode process ("init").
pub fn spawn_init_process() -> Option<(u64, u64)> {
    allocate_user_process("init", build_init_program(), Capability::SYS_DEBUG)
}

/// Prepare the future Ring 3 compositor contract without jumping to it yet.
///
/// Today the reliable desktop path stays in Ring 0/GOP. This function validates
/// that the compositor payload can be generated against the syscall ABI, but it
/// intentionally does not copy code into user memory or create a runnable
/// process from the `Run` path. That keeps desktop boot stable until paging and
/// scheduler return paths are complete.
pub fn prepare_desktop_compositor() -> bool {
    let mut code_buf = [0u8; 256];
    let (_entry_off, total) = crate::desktop::compositor::build_compositor(&mut code_buf, USER_CODE_BASE);
    if total == 0 || total > code_buf.len() {
        crate::drivers::serial::serial_write("[user_init] Ring 3 compositor build failed.\n");
        return false;
    }

    crate::drivers::serial::serial_write("[user_init] Ring 3 compositor ABI validated; Ring 0 remains supervisor.\n");
    true
}

/// Jump to Ring 3 — execute the init process. Does NOT return.
pub unsafe fn jump_to_ring3(entry_point: u64, user_stack: u64) -> ! {
    core::arch::asm!(
        "mov rcx, {entry}",
        "mov r11, 0x202",
        "mov rsp, {stack}",
        "sysretq",
        entry = in(reg) entry_point,
        stack = in(reg) user_stack,
        options(noreturn),
    );
}

/// Shell command: spawn Ring 3 hello process.
pub fn spawn_hello() {
    crate::drivers::serial::serial_write("[user_init] Spawning hello Ring 3 process...\n");
    if let Some((_entry, _stack)) = spawn_init_process() {
        crate::drivers::serial::serial_write("[user_init] Process created, jumping to Ring 3\n");
        // NOTE: jump_to_ring3 does NOT return. The shell will not resume.
        // In a full OS, we'd schedule it and return to the shell.
        // For now, we just log that it's ready.
        crate::drivers::serial::serial_write("[user_init] Ring 3 process ready (not jumping yet — needs scheduler)\n");
    } else {
        crate::drivers::serial::serial_write("[user_init] ERROR: failed to spawn process\n");
    }
}

/// Shell command: launch the desktop path that is stable today.
pub fn spawn_desktop() -> ! {
    crate::drivers::serial::serial_write("[user_init] Launching Ring 0 desktop via GOP; preparing Ring 3 contract.\n");
    prepare_desktop_compositor();
    crate::desktop::run_ring0();
}
