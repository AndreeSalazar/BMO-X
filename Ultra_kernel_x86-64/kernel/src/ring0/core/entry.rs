//! Kernel entry point ??? `_start` naked asm + `kernel_main_real`.
//!
//! Called from `stage3_dev` via `jmp` with `rdi = *const BootContext`.

use core::arch::naked_asm;

// ?????? _start: first code executed after the boot chain jumps here ???????????????

#[unsafe(no_mangle)]
#[link_section = ".text._start"]
#[unsafe(naked)]
unsafe extern "C" fn _start() -> ! {
    naked_asm!(
        // Save BootContext ptr in r12 (callee-saved).
        "mov r12, rdi",

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
    );
}

#[unsafe(no_mangle)]
#[inline(never)]
extern "C" fn kernel_main_real(ctx: *const boot_context::BootContext) -> ! {
    // Defensive: if a non-magic context arrived, halt.
    if ctx.is_null() {
        loop { unsafe { core::arch::asm!("hlt"); } }
    }
    let ctx_ref = unsafe { &*ctx };

    // ARM crash marker at low RAM (visible to host debugger).
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
