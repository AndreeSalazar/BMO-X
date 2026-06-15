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
/// Kernel stack per thread (8 KB each).
const KERNEL_STACK_PER_THREAD: usize = 8192;

/// User virtual address space layout:
/// Code: 0x0000_0000_0040_0000 (4 MB)
/// Stack: 0x0000_0000_0080_0000 (8 MB) - grows down
const USER_CODE_VBASE: u64 = 0x0000_0000_0040_0000;
const USER_STACK_VBASE: u64 = 0x0000_0000_0080_0000;
const USER_STACK_VTOP: u64 = USER_STACK_VBASE + USER_STACK_SIZE as u64;

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
    proc.caps = caps;

    // Create dedicated user page table (clones kernel mappings)
    let kernel_cr3 = paging::read_cr3();
    let user_cr3 = unsafe { paging::create_user_page_table(kernel_cr3)? };
    proc.page_table_root = user_cr3;

    let code_pages = (code.len() + crate::arch::page_alloc::page_size() - 1) / crate::arch::page_alloc::page_size();
    let stack_pages = USER_STACK_SIZE / crate::arch::page_alloc::page_size();

    // Allocate physical pages for code and stack
    let code_phys = unsafe { crate::arch::page_alloc::alloc_pages_contiguous(code_pages.max(1))? };
    let stack_phys = unsafe { crate::arch::page_alloc::alloc_pages_contiguous(stack_pages)? };

    // Map into user virtual address space
    // Code: RX, USER, !NX
    let code_flags = paging::flags::PRESENT | paging::flags::USER | paging::flags::WRITABLE; // writable for copy, then make RX
    unsafe {
        paging::map_user_range(user_cr3, USER_CODE_VBASE, code_phys, code_pages, code_flags).ok()?;
    }

    // Stack: RW, USER, NX
    let stack_flags = paging::flags::PRESENT | paging::flags::USER | paging::flags::WRITABLE | paging::flags::NO_EXECUTE;
    unsafe {
        paging::map_user_range(user_cr3, USER_STACK_VBASE, stack_phys, stack_pages, stack_flags).ok()?;
    }

    // Copy code to physical pages
    unsafe {
        let dst = code_phys as *mut u8;
        core::ptr::copy_nonoverlapping(code.as_ptr(), dst, code.len());
        if code_pages * crate::arch::page_alloc::page_size() > code.len() {
            core::ptr::write_bytes(
                dst.add(code.len()),
                0x90, // NOP padding
                code_pages * crate::arch::page_alloc::page_size() - code.len(),
            );
        }
        // Zero stack
        core::ptr::write_bytes(stack_phys as *mut u8, 0, USER_STACK_SIZE);
    }

    // After copy, code pages are mapped RW+USER for simplicity.
    // A production kernel would re-walk PTEs here and clear WRITABLE to enforce RX-only.

    proc.entry_point = USER_CODE_VBASE;
    proc.user_code_base = code_phys;
    proc.user_code_size = code_pages * crate::arch::page_alloc::page_size();
    proc.user_stack_base = stack_phys;
    proc.user_stack_size = USER_STACK_SIZE;

    let user_stack_top = USER_STACK_VTOP;

    let kernel_stack = unsafe {
        let layout = core::alloc::Layout::from_size_align(KERNEL_STACK_PER_THREAD, 16).unwrap();
        let ptr = alloc::alloc::alloc_zeroed(layout);
        if ptr.is_null() { return None; }
        ptr as u64 + KERNEL_STACK_PER_THREAD as u64
    };

    let thr = thread::alloc_thread(proc.pid, Priority::Interactive)?;
    thr.regs = thread::SavedRegs::new_user(USER_CODE_VBASE, user_stack_top);
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

    crate::diag::info_u64("ring3", "user code entry", USER_CODE_VBASE);
    crate::diag::info_u64("ring3", "user stack top", user_stack_top);
    crate::diag::info_u64("ring3", "user CR3", user_cr3);
    Some((USER_CODE_VBASE, user_stack_top))
}

/// Spawn the first user-mode process ("init").
pub fn spawn_init_process() -> Option<(u64, u64)> {
    crate::diag::info("sched", "allocating init Ring 3 test process");
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
    crate::diag::info("sched", "validating Ring 3 compositor payload ABI");
    let mut code_buf = [0u8; 256];
    let (_entry_off, total) = crate::desktop::compositor::build_compositor(&mut code_buf, 0);
    if total == 0 || total > code_buf.len() {
        crate::diag::fault("sched", "Ring 3 compositor build failed");
        crate::drivers::serial::serial_write("[user_init] Ring 3 compositor build failed.\n");
        return false;
    }

    crate::diag::info_u64("sched", "Ring 3 compositor payload bytes", total as u64);
    crate::drivers::serial::serial_write("[user_init] Ring 3 compositor ABI validated; Ring 0 remains supervisor.\n");
    true
}

/// Jump to Ring 3 — execute the init process. Does NOT return.
/// Uses iretq for safe return from Ring 0 to Ring 3.
pub unsafe fn jump_to_ring3(entry_point: u64, user_stack: u64) -> ! {
    // Build interrupt frame for iretq return to Ring 3
    // Layout (from low to high):
    //   SS, RSP, RFLAGS, CS, RIP
    core::arch::asm!(
        // Build iretq frame on kernel stack
        "push qword ptr {user_ss}",     // SS (user data segment)
        "push {stack}",                  // RSP (user stack pointer)
        "push qword ptr 0x202",         // RFLAGS (IF=1, reserved bit 1)
        "push qword ptr {user_cs}",     // CS (user code segment)
        "push {entry}",                  // RIP (user entry point)

        // Return to Ring 3 via iretq
        "iretq",

        user_cs = const 0x23_u64,        // USER_CS | RPL=3
        user_ss = const 0x1B_u64,        // USER_DS | RPL=3
        entry = in(reg) entry_point,
        stack = in(reg) user_stack,
        options(noreturn),
    );
}

fn launch_desktop_compositor_ring3() -> bool {
    crate::diag::info("ring3", "building desktop compositor process");
    let mut code_buf = [0u8; 256];
    let (_entry_off, total) = crate::desktop::compositor::build_compositor(&mut code_buf, 0);
    if total == 0 || total > code_buf.len() {
        crate::diag::fault("ring3", "desktop compositor payload invalid");
        return false;
    }

    let Some((entry, stack)) = allocate_user_process(
        "desktop3",
        &code_buf[..total],
        Capability::SYS_DEBUG,
    ) else {
        crate::diag::fault("ring3", "desktop compositor allocation failed");
        return false;
    };

    crate::diag::info_u64("ring3", "sysret desktop entry", entry);
    crate::drivers::serial::serial_write("[user_init] Jumping to Ring 3 desktop compositor.\n");
    unsafe { jump_to_ring3(entry, stack); }
}

/// Shell command: spawn Ring 3 hello process.
pub fn spawn_hello() {
    crate::diag::info("sched", "spawn_hello requested");
    crate::drivers::serial::serial_write("[user_init] Spawning hello Ring 3 process...\n");
    if let Some((entry, stack)) = spawn_init_process() {
        crate::diag::info_u64("sched", "Ring 3 hello sysret entry", entry);
        crate::drivers::serial::serial_write("[user_init] Process created, jumping to Ring 3\n");
        unsafe { jump_to_ring3(entry, stack); }
    } else {
        crate::diag::fault("sched", "failed to spawn Ring 3 hello process");
        crate::drivers::serial::serial_write("[user_init] ERROR: failed to spawn process\n");
    }
}

/// Shell command: launch the desktop path that is stable today.
pub fn spawn_desktop() -> ! {
    crate::diag::info("sched", "spawn_desktop: Ring 3 compositor + Ring 0 services");
    crate::drivers::serial::serial_write("[user_init] Launching Ring 3 desktop compositor over Ring 0 GOP services.\n");
    if launch_desktop_compositor_ring3() {
        loop { unsafe { core::arch::asm!("hlt"); } }
    }

    crate::diag::warn("sched", "Ring 3 desktop unavailable; falling back to Ring 0");
    crate::desktop::run_ring0();
}
