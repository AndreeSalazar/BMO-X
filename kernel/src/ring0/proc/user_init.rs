//! v1.8.8: `spawn_hello` y `spawn_desktop` se llaman desde
//! `bmo_core::desktop::welcome` (comandos Hello / Run / Reboot).
//! `spawn_init_process` es interno. El resto de las funciones
//! públicas son los entry points que `jump_to_ring3` invoca al
//! hacer el iretq a Ring 3.

//! User-space initialization — first Ring 3 process.
//!
//! Creates a minimal "init" process that runs in Ring 3 and demonstrates
//! syscall functionality (DebugPrint, ClockGetTime).

#![allow(dead_code)]

use super::process;
use super::task;
use super::Priority;
// use crate::bmo_core::fs::Capabilities;  // TEMPORAL — moved to Temporal()
use crate::mm::virt;

/// Capabilities stub (bmo_core::fs::Capabilities moved to Temporal)
pub type Capabilities = u32;
const SYS_DEBUG: Capabilities = 1;

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
    // Machine code layout (x86-64):
    // [0..7)   lea rdi, [rip+disp32] → points to data at offset 45
    // [7..14)  mov rsi, 19
    // [14..21) mov rax, 0xF0
    // [21..23) syscall
    // [23..30) mov rax, 0x50
    // [30..32) syscall
    // [32..35) xor rdi, rdi
    // [35..42) mov rax, 0x00
    // [42..44) syscall
    // [44..45) hlt
    // [45..64) "Hello from Ring 3!\n" (19 bytes)
    static INIT_CODE: [u8; 64] = [
        // === Print "Hello from Ring 3!\n" via DebugPrint (syscall 0xF0) ===
        // lea rdi, [rip + 38]  ; a0 = pointer to string (45 - 7 = 38)
        0x48, 0x8D, 0x3D, 0x26, 0x00, 0x00, 0x00,
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

        // === Exit via ProcessExit(0) — syscall 0x00 ===
        // xor rdi, rdi  ; exit_code = 0
        0x48, 0x31, 0xFF,
        // mov rax, 0x00
        0x48, 0xC7, 0xC0, 0x00, 0x00, 0x00, 0x00,
        // syscall
        0x0F, 0x05,
        // hlt (should never reach here)
        0xF4,

        // === Data: "Hello from Ring 3!\n" (19 bytes) ===
        b'H', b'e', b'l', b'l', b'o', b' ', b'f', b'r', b'o', b'm',
        b' ', b'R', b'i', b'n', b'g', b' ', b'3', b'!', b'\n',
    ];
    &INIT_CODE
}

fn allocate_user_process(name: &str, code: &[u8], caps: Capabilities) -> Option<(u64, u64)> {
    // crate::cabina::info("ring3", "=== Ring 3 process allocation START ===");  // TEMPORAL
    crate::dev::console::serial_write("[ring3] === alloc start ===\n");
    // crate::cabina::info("ring3", "allocating process struct");  // TEMPORAL
    let proc = process::alloc_process()?;
    proc.set_name(name);
    proc.caps = caps;
    // crate::cabina::info("ring3", "process struct allocated");  // TEMPORAL

    // Create dedicated user page table (clones kernel mappings)
    // crate::cabina::info("ring3", "reading kernel CR3");  // TEMPORAL
    let kernel_cr3 = virt::read_cr3();
    // crate::cabina::info_u64("ring3", "kernel CR3", kernel_cr3);  // TEMPORAL
    // crate::cabina::info("ring3", "creating user page table");  // TEMPORAL
    let user_cr3 = unsafe { virt::create_user_page_table(kernel_cr3)? };
    proc.page_table_root = user_cr3;
    // crate::cabina::info_u64("ring3", "user CR3", user_cr3);  // TEMPORAL

    let code_pages = (code.len() + crate::mm::phys::page_size() - 1) / crate::mm::phys::page_size();
    let stack_pages = USER_STACK_SIZE / crate::mm::phys::page_size();
    // crate::cabina::info_u64("ring3", "code pages", code_pages as u64);  // TEMPORAL
    // crate::cabina::info_u64("ring3", "stack pages", stack_pages as u64);  // TEMPORAL

    // Allocate physical pages for code and stack
    // crate::cabina::info("ring3", "allocating physical pages for code");  // TEMPORAL
    let code_phys = unsafe { crate::mm::phys::alloc_pages_contiguous(code_pages.max(1))? };
    // crate::cabina::info_u64("ring3", "code phys addr", code_phys);  // TEMPORAL
    // crate::cabina::info("ring3", "allocating physical pages for stack");  // TEMPORAL
    let stack_phys = unsafe { crate::mm::phys::alloc_pages_contiguous(stack_pages)? };
    // crate::cabina::info_u64("ring3", "stack phys addr", stack_phys);  // TEMPORAL

    // Map into user virtual address space
    // Code: RX, USER, !NX
    let code_flags = virt::flags::PRESENT | virt::flags::USER | virt::flags::WRITABLE;
    // crate::cabina::info("ring3", "mapping code into user page table");  // TEMPORAL
    unsafe {
        virt::map_user_range(user_cr3, USER_CODE_VBASE, code_phys, code_pages, code_flags).ok()?;
    }

    // Stack: RW, USER, NX
    let stack_flags = virt::flags::PRESENT | virt::flags::USER | virt::flags::WRITABLE | virt::flags::NO_EXECUTE;
    // crate::cabina::info("ring3", "mapping stack into user page table");  // TEMPORAL
    unsafe {
        virt::map_user_range(user_cr3, USER_STACK_VBASE, stack_phys, stack_pages, stack_flags).ok()?;
    }

    // Copy code to physical pages (via high-mem mapping)
    // crate::cabina::info("ring3", "copying code bytes to physical pages");  // TEMPORAL
    unsafe {
        let dst = crate::mm::virt::phys_to_virt(code_phys) as *mut u8;
        core::ptr::copy_nonoverlapping(code.as_ptr(), dst, code.len());
        if code_pages * crate::mm::phys::page_size() > code.len() {
            core::ptr::write_bytes(
                dst.add(code.len()),
                0x90, // NOP padding
                code_pages * crate::mm::phys::page_size() - code.len(),
            );
        }
        // Zero stack (via high-mem mapping)
        core::ptr::write_bytes(crate::mm::virt::phys_to_virt(stack_phys) as *mut u8, 0, USER_STACK_SIZE);
    }
    // crate::cabina::info("ring3", "code and stack zeroed/populated");  // TEMPORAL

    // After copy, code pages are mapped RW+USER for simplicity.
    proc.entry_point = USER_CODE_VBASE;
    proc.user_code_base = code_phys;
    proc.user_code_size = code_pages * crate::mm::phys::page_size();
    proc.user_stack_base = stack_phys;
    proc.user_stack_size = USER_STACK_SIZE;

    let user_stack_top = USER_STACK_VTOP;

    let kernel_stack = unsafe {
        let layout = core::alloc::Layout::from_size_align(KERNEL_STACK_PER_THREAD, 16).unwrap();
        let ptr = alloc::alloc::alloc_zeroed(layout);
        if ptr.is_null() { return None; }
        ptr as u64 + KERNEL_STACK_PER_THREAD as u64
    };
    // crate::cabina::info_u64("ring3", "kernel stack for this thread", kernel_stack);  // TEMPORAL

    let thr = task::alloc(proc.pid, Priority::Interactive)?;
    thr.regs = task::SavedRegs::new_user(USER_CODE_VBASE, user_stack_top);
    thr.kernel_stack_top = kernel_stack;
    thr.state = task::State::Ready;

    let tid = thr.tid;
    if let Some(idx) = task::find_index(tid) {
        task::set_current(idx);
        if let Some(t) = task::get(idx) {
            t.state = task::State::Running;
        }
    }
    // crate::cabina::info_u64("ring3", "thread TID", tid.0 as u64);  // TEMPORAL

    // Critical: set BOTH the TSS.rsp0 (for #GP/#DF exceptions) AND the
    // SYSCALL_KERNEL_RSP (for the syscall entry to switch to).
    // crate::cabina::info("ring3", "setting kernel stack for TSS.rsp0 and syscall entry");  // TEMPORAL
    crate::arch::gdt::set_kernel_stack(kernel_stack);
    crate::arch::syscall::set_syscall_kernel_stack(kernel_stack);

    // Sanity: read back the values to verify the writes took effect.
    // crate::cabina::info("ring3", "Ring 3 process allocation complete");  // TEMPORAL
    // crate::cabina::info_u64("ring3", "user code entry (Ring 3 RIP)", USER_CODE_VBASE);  // TEMPORAL
    // crate::cabina::info_u64("ring3", "user stack top (Ring 3 RSP)", user_stack_top);  // TEMPORAL
    // crate::cabina::info_u64("ring3", "user CR3 (page table root)", user_cr3);  // TEMPORAL
    // crate::cabina::info("ring3", "=== Ring 3 process allocation END ===");  // TEMPORAL
    Some((USER_CODE_VBASE, user_stack_top))
}

/// Spawn the first user-mode process ("init").
pub fn spawn_init_process() -> Option<(u64, u64)> {
    // crate::cabina::info("sched", "allocating init Ring 3 test process");  // TEMPORAL
    allocate_user_process("init", build_init_program(), SYS_DEBUG)
}

/// Prepare the future Ring 3 compositor contract without jumping to it yet.
///
/// Today the reliable desktop path stays in Ring 0/GOP. This function validates
/// that the compositor payload can be generated against the syscall ABI, but it
/// intentionally does not copy code into user memory or create a runnable
/// process from the `Run` path. That keeps desktop boot stable until paging and
/// scheduler return paths are complete.
pub fn prepare_desktop_compositor() -> bool {
    // TEMPORAL: bmo_core::desktop::compositor moved out — stubbed
    crate::dev::console::serial_write("[user_init] Ring 3 compositor stubbed (TEMPORAL)\n");
    false
}

/// Jump to Ring 3 — execute the init process. Does NOT return.
/// Uses iretq for safe return from Ring 0 to Ring 3.
pub unsafe fn jump_to_ring3(entry_point: u64, user_stack: u64) -> ! {
    // CRITICAL: This is the moment we leave Ring 0. Any error here
    // becomes a #GP / #DF / triple-fault and we lose the CPU.
    //
    // Sanity check 1: stack pointer must be 16-byte aligned.
    if user_stack & 0xF != 0 {
        crate::dev::console::serial_write("[ring3] user_stack NOT 16-byte aligned — #GP imminent\n");
        loop { core::arch::asm!("hlt"); }
    }
    // Sanity check 2: entry point must be canonical (high bit 47 == high bit 48-63).
    if (entry_point >> 47) != ((entry_point >> 48) & 1) {
        crate::dev::console::serial_write("[ring3] entry_point NOT canonical — #GP imminent\n");
        loop { core::arch::asm!("hlt"); }
    }
    // Sanity check 3: user_stack must be canonical.
    if (user_stack >> 47) != ((user_stack >> 48) & 1) {
        crate::dev::console::serial_write("[ring3] user_stack NOT canonical — #GP imminent\n");
        loop { core::arch::asm!("hlt"); }
    }
    // Sanity check 4: user_stack must be in lower half of user address space.
    if user_stack >= 0x0000_8000_0000_0000 {
        crate::dev::console::serial_write("[ring3] user_stack in kernel range — would overwrite kernel\n");
        loop { core::arch::asm!("hlt"); }
    }

    // crate::cabina::info("ring3", "=== Ring 3 JUMP START ===");  // TEMPORAL
    crate::dev::console::serial_write("[ring3] === Ring 3 JUMP START ===\n");
    crate::dev::console::serial_write("  entry (RIP)=0x");
    crate::dev::console::serial_write_u64(entry_point, 16);
    crate::dev::console::serial_write(" stack (RSP)=0x");
    crate::dev::console::serial_write_u64(user_stack, 16);
    crate::dev::console::serial_write("\n");
    crate::dev::console::serial_write("[ring3] jumping: RIP=");
    crate::dev::console::serial_write_u64(entry_point, 16);
    crate::dev::console::serial_write(" RSP=");
    crate::dev::console::serial_write_u64(user_stack, 16);
    crate::dev::console::serial_write(" CR3=");
    crate::dev::console::serial_write("...\n");
    // crate::cabina::read_cr3_into_serial();  // TEMPORAL

    // Build interrupt frame for iretq return to Ring 3
    // Layout (low to high on kernel stack):
    //   SS, RSP, RFLAGS, CS, RIP
    core::arch::asm!(
        // Build iretq frame on kernel stack
        "push qword ptr {user_ss}",     // SS (user data segment) 0x1B
        "push {stack}",                  // RSP (user stack pointer)
        "push qword ptr 0x202",         // RFLAGS (IF=1, reserved bit 1 set)
        "push qword ptr {user_cs}",     // CS (user code segment) 0x23
        "push {entry}",                  // RIP (user entry point)

        // Return to Ring 3 via iretq
        // iretq pops 5 values: RIP, CS, RFLAGS, RSP, SS
        // and switches to Ring 3 with the new RIP/RSP/CS/SS
        "iretq",

        user_cs = const 0x23_u64,        // USER_CS | RPL=3
        user_ss = const 0x1B_u64,        // USER_DS | RPL=3
        entry = in(reg) entry_point,
        stack = in(reg) user_stack,
        options(noreturn),
    );
}

fn launch_desktop_compositor_ring3() -> bool {
    // TEMPORAL: bmo_core::desktop::compositor moved out — stubbed
    crate::dev::console::serial_write("[ring3] desktop compositor stubbed (TEMPORAL)\n");
    false
}

/// Shell command: spawn Ring 3 hello process.
pub fn spawn_hello() {
    // TEMPORAL: cabina/bmo_core moved out — stubbed
    crate::dev::console::serial_write("[user_init] spawn_hello: stubbed (TEMPORAL)\n");
}

/// Shell command: launch the desktop path that is stable today.
///
/// BUG WORKAROUND: instead of `-> !`, return from this function so
/// the welcome screen can recover if anything fails. The desktop
/// itself is `-> !` so if everything works, it runs forever.
pub fn spawn_desktop() {
    // TEMPORAL: bmo_core::desktop moved out — stubbed
    crate::dev::console::serial_write("[user_init] spawn_desktop: stubbed (TEMPORAL)\n");
}

/// Build a minimal Ring 3 program that executes `ud2` (undefined opcode).
/// This triggers #UD → exception_kill_handler → kill_current_process → schedule.
/// Used to verify crash recovery: after the process dies, the scheduler
/// should return to the welcome/shell screen.
fn build_crash_program() -> &'static [u8] {
    static CRASH_CODE: [u8; 15] = [
        // ud2 — triggers #UD exception (vector 6)
        0x0F, 0x0B,
        // Should never reach here
        0xF4, // hlt
        // Padding to 16 bytes for alignment
        0x90, 0x90, 0x90, 0x90, 0x90, 0x90, 0x90, 0x90, 0x90, 0x90, 0x90, 0x90,
    ];
    &CRASH_CODE
}

/// Shell command: spawn a Ring 3 process that crashes with ud2.
/// Verifies exception → kill → scheduler → welcome returns.
pub fn spawn_crash() {
    // TEMPORAL: cabina/bmo_core moved out — stubbed
    crate::dev::console::serial_write("[user_init] spawn_crash: stubbed (TEMPORAL)\n");
}



