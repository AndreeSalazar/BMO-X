//! Kernel entry point — `_start` naked asm + `kernel_main_real`.
//!
//! Called from `s12_devices` via `jmp` with `rdi = *const BootContext`.
//!
//! ## Stack switch
//!
//! When s12_devices jumps here, RSP is still the UEFI stack which
//! may live at a high physical address (e.g. 0x7FFF...) that
//! s9_paging did not identity-map. The very first push/call would
//! cause a #PF on a not-present page and triple-fault. We therefore
//! switch to an internal stack (defined in .bss, identity-mapped by
//! s9_paging) before doing anything that uses the stack.

use core::arch::{asm, naked_asm};

// The kernel's 64 KiB stack is allocated by the linker script
// (see linker.ld, .bss section, 65536 bytes). KERNEL_STACK_END_MARKER
// is the address of the top of that stack. We do not declare the
// array in Rust to avoid double-allocation; the linker creates the
// backing memory and KERNEL_STACK_END_MARKER is the symbol just past
// the end.

// _start: first code executed after the boot chain jumps here.
#[unsafe(no_mangle)]
#[link_section = ".text._start"]
#[unsafe(naked)]
unsafe extern "C" fn _start() -> ! {
    naked_asm!(
        // Save BootContext ptr in r12 (callee-saved).
        "mov r12, rdi",

        // Switch to a known-good internal stack IMMEDIATELY.
        // The UEFI stack may be unmapped after s9_paging.
        // The stack lives in the kernel's .bss at KERNEL_STACK + 65536,
        // which s9_paging identity-maps. The next block zeros BSS
        // (including the stack), which is fine — we're not using
        // any old stack contents.
        "lea rsp, [rip + {stack_end}]",

        // Zero BSS (stosq + stosb).
        "lea rax, [rip + __bss_start]",
        "lea rcx, [rip + __bss_end]",
        "sub rcx, rax",
        "jz 2f",
        "mov rdi, rax",
        "xor eax, eax",
        "mov rdx, rcx",
        "shr rcx, 3",
        "jz 1f",
        "rep stosq",
        "1: and rdx, 7",
        "mov rcx, rdx",
        "jz 2f",
        "rep stosb",

        // Enter kernel.
        "2: mov rdi, r12",
        "call kernel_main_real",

        // Halt if returned.
        "3: hlt",
        "jmp 3b",

        // Linker-resolved address: KERNEL_STACK + 65536 (end of stack).
        // The asm! block substitutes the address of KERNEL_STACK
        // and adds the size constant at expansion time.
        stack_end = sym KERNEL_STACK_END_MARKER,
    );
}

// Marker at the top of the kernel stack. This is at the same
// address as KERNEL_STACK + 65536. We declare it as a zero-sized
// extern so the linker resolves its address; the actual stack
// memory is KERNEL_STACK.
extern "C" {
    static KERNEL_STACK_END_MARKER: u8;
}

#[unsafe(no_mangle)]
#[inline(never)]
extern "C" fn kernel_main_real(ctx: *const boot_context::BootContext) -> ! {
    // Defensive: if a non-magic context arrived, halt.
    if ctx.is_null() {
        loop { unsafe { asm!("hlt"); } }
    }
    let ctx_ref = unsafe { &*ctx };

    // Crash marker at low RAM (visible to host debugger).
    unsafe {
        core::ptr::write_volatile(0x9_0000 as *mut u32, 0x464F_5343u32); // "FOSC"
        core::ptr::write_volatile(0x9_0004 as *mut u32, 1u32);
    }

    crate::ring0::dev::console::serial_write("[entry] kernel_main_real entered\n");

    // Drain any scancodes left in the i8042 PS/2 buffer by s12_devices.
    // The keyboard controller accumulates up to 16 bytes; if the user
    // pressed keys during boot (NumLock, CapsLock), those scancodes
    // would otherwise stay in the buffer. Since the shell is serial-
    // driven (COM1), we don't want stray PS/2 scancodes interfering
    // with anything else later.
    unsafe {
        const KBD_STATUS: u16 = 0x64;
        const KBD_DATA:   u16 = 0x60;
        for _ in 0..32 {
            // Bit 0 of status = output buffer full (data ready to read).
            let status: u8;
            core::arch::asm!("in al, dx", in("dx") KBD_STATUS, out("al") status);
            if status & 1 == 0 { break; }
            let _: u8;
            core::arch::asm!("in al, dx", in("dx") KBD_DATA, out("al") _);
        }
    }

    crate::ring0::core::phase::main(ctx_ref);

    // Once all phases complete, idle forever (single-CPU Ring 0 base).
    crate::ring0::dev::console::serial_write("[entry] idle (hlt loop)\n");
    loop {
        unsafe { core::arch::asm!("sti; hlt"); }
    }
}

#[panic_handler]
fn panic(_info: &core::panic::PanicInfo) -> ! {
    loop { unsafe { core::arch::asm!("hlt"); } }
}
