#![allow(dead_code)]

//! IDT — Interrupt Descriptor Table for x86-64 Long Mode.
//! 256 entries, 16 bytes each. Ring 0, no_std.
//!
//! CRITICAL: All ISR stubs MUST be `#[naked]` so the compiler does NOT
//! generate a function prologue (push rbp / mov rbp,rsp). A prologue
//! would corrupt the interrupt stack frame and cause a triple-fault on
//! `iretq`. This was the root cause of the reboot-on-real-hardware bug.
//!
//! CPU exceptions 8, 10-14, 17, 21, 29, 30 push an error code onto the
//! stack. Our handlers must pop it before `iretq`.

use core::arch::{asm, naked_asm};

/// IDT entry (16 bytes in Long Mode).
#[repr(C, packed)]
#[derive(Clone, Copy)]
pub struct IdtEntry {
    offset_low: u16,
    selector: u16,
    ist: u8,
    type_attr: u8,
    offset_mid: u16,
    offset_high: u32,
    reserved: u32,
}

impl IdtEntry {
    const fn empty() -> Self {
        Self {
            offset_low: 0, selector: 0, ist: 0, type_attr: 0,
            offset_mid: 0, offset_high: 0, reserved: 0,
        }
    }

    fn set_handler(&mut self, handler: u64) {
        self.offset_low = handler as u16;
        self.offset_mid = (handler >> 16) as u16;
        self.offset_high = (handler >> 32) as u32;
        self.selector = 0x08; // Kernel code segment
        self.ist = 0;
        self.type_attr = 0x8E; // Present, DPL=0, 64-bit interrupt gate
        self.reserved = 0;
    }
}

/// IDTR register value for `lidt`.
#[repr(C, packed)]
struct Idtr {
    limit: u16,
    base: u64,
}

static mut IDT: [IdtEntry; 256] = [IdtEntry::empty(); 256];

/// IRQ handler function pointers (set by drivers).
static mut IRQ_HANDLERS: [Option<fn()>; 16] = [None; 16];

/// Initialize the IDT and load it via LIDT.
pub fn init_idt() {
    unsafe {
        // CPU exceptions WITHOUT error code (0-7, 9, 15, 16, 18-20, 22-31)
        for i in [0,1,2,3,4,5,6,7,9,15,16,18,19,20,22,23,24,25,26,27,28,31] {
            IDT[i].set_handler(isr_stub_exception_no_err as *const () as u64);
        }

        // CPU exceptions WITH error code (8, 10, 11, 12, 13, 14, 17, 21, 29, 30)
        for i in [8,10,11,12,13,14,17,21,29,30] {
            IDT[i].set_handler(isr_stub_exception_err as *const () as u64);
        }

        // Diagnóstico real para las excepciones más probables en Ring 3:
        // #GP, #PF, #UD, #NM, #MF, #XM, #DE — matan el proceso en vez de loops infinitos.
        // Usan IST1 para stack dedicado y evitar corrupción del stack de usuario.
        IDT[13].set_handler(isr_stub_general_protection as *const () as u64);
        IDT[13].ist = 1;  // IST1
        IDT[14].set_handler(isr_stub_page_fault as *const () as u64);
        IDT[14].ist = 1;  // IST1
        IDT[6].set_handler(isr_stub_invalid_opcode as *const () as u64);
        IDT[6].ist = 1;   // IST1 — #UD (ud2 / undefined instruction)
        IDT[7].set_handler(isr_stub_device_not_avail as *const () as u64);
        IDT[7].ist = 1;   // IST1 — #NM (FPU not available)
        IDT[16].set_handler(isr_stub_x87_fp as *const () as u64);
        IDT[16].ist = 1;  // IST1 — #MF (x87 FP exception)
        IDT[19].set_handler(isr_stub_simd_fp as *const () as u64);
        IDT[19].ist = 1;  // IST1 — #XM (SSE/AVX exception)
        IDT[0].set_handler(isr_stub_divide_error as *const () as u64);
        IDT[0].ist = 1;   // IST1 — #DE (divide error)

        // IRQ0 — PIT timer (vector 32)
        IDT[32].set_handler(isr_stub_irq0 as *const () as u64);

        // IRQ1 — PS/2 keyboard (vector 33)
        IDT[33].set_handler(isr_stub_irq1 as *const () as u64);

        // Remaining IRQs (34-47) — default
        for i in 34..48 {
            IDT[i].set_handler(isr_stub_default_irq as *const () as u64);
        }

        // APIC Timer (vector 48) — preemptive scheduling
        IDT[48].set_handler(isr_stub_apic_timer as *const () as u64);

        // Spurious APIC (vector 255)
        IDT[255].set_handler(isr_stub_default_irq as *const () as u64);

        let idtr = Idtr {
            limit: (core::mem::size_of::<[IdtEntry; 256]>() - 1) as u16,
            base: core::ptr::addr_of!(IDT) as u64,
        };

        asm!("lidt [{}]", in(reg) &idtr, options(nostack));
    }
}

/// Register a handler for an IRQ (0-15).
pub fn register_irq(irq: u8, handler: fn()) {
    if (irq as usize) < 16 {
        unsafe { IRQ_HANDLERS[irq as usize] = Some(handler); }
    }
}

// ── Raw ISR stubs ──────────────────────────────────────────────────────────
// ALL stubs are #[naked] — the compiler MUST NOT generate prologue/epilogue.
// Without #[naked], `push rbp; mov rbp, rsp` would corrupt the interrupt
// frame and iretq would pop garbage → triple fault.

/// Exception handler for vectors that do NOT push an error code.
#[unsafe(naked)]
unsafe extern "C" fn isr_stub_exception_no_err() {
    naked_asm!(
        "iretq",
    );
}

/// Exception handler for vectors that DO push an error code.
/// Must pop the error code before iretq.
#[unsafe(naked)]
unsafe extern "C" fn isr_stub_exception_err() {
    naked_asm!(
        "add rsp, 8",  // pop error code
        "iretq",
    );
}

/// #GP — kill current process instead of halting CPU.
#[unsafe(naked)]
unsafe extern "C" fn isr_stub_general_protection() {
    naked_asm!(
        // Save minimal context for kill_current_process
        "push rax",
        "push rdi",
        "push rsi",

        // Get error code from stack (pushed by CPU before this handler)
        "mov rdi, 13",                 // vector = #GP (13)
        "mov rsi, [rsp + 24]",         // error code (after 3 pushes)

        // Call Rust handler
        "call exception_kill_handler_rust",

        // Should never return, but just in case
        "pop rsi",
        "pop rdi",
        "pop rax",
        "iretq",
    );
}

/// #PF — kill current process, capturing CR2 for diagnostics.
#[unsafe(naked)]
unsafe extern "C" fn isr_stub_page_fault() {
    naked_asm!(
        // Save minimal context for kill_current_process
        "push rax",
        "push rdi",
        "push rsi",
        "push rdx",

        // Get error code and CR2
        "mov rdi, 14",                 // vector = #PF (14)
        "mov rsi, [rsp + 32]",         // error code (after 4 pushes)
        "mov rdx, cr2",                // faulting address

        // Call Rust handler
        "call exception_kill_handler_rust",

        // Should never return, but just in case
        "pop rdx",
        "pop rsi",
        "pop rdi",
        "pop rax",
        "iretq",
    );
}

/// Default IRQ handler (vectors 34-47) — just iretq since PIC is removed.
#[unsafe(naked)]
unsafe extern "C" fn isr_stub_default_irq() {
    naked_asm!(
        "iretq",
    );
}

/// #UD — Invalid Opcode (ud2, undefined instruction). Kill current process.
#[unsafe(naked)]
unsafe extern "C" fn isr_stub_invalid_opcode() {
    naked_asm!(
        "push rax",
        "push rdi",
        "push rsi",
        "mov rdi, 6",          // vector = #UD (6)
        "xor rsi, rsi",        // error code = 0
        "call exception_kill_handler_rust",
        "pop rsi",
        "pop rdi",
        "pop rax",
        "iretq",
    );
}

/// #NM — Device Not Available (FPU/SSE). Handles lazy FPU context switching.
///
/// When CR0.TS is set (lazy FPU mode), the first FPU/SSE/AVX instruction triggers #NM.
/// We save the previous task's FPU state, restore the current task's state, and clear TS.
#[unsafe(naked)]
unsafe extern "C" fn isr_stub_device_not_avail() {
    naked_asm!(
        // Save minimal context
        "push rax",
        "push rdi",
        "push rsi",

        // Save FPU/SSE/AVX state for the preempted task
        // rdi = pointer to save area (will be provided by scheduler)
        // For now, just clear TS and let the task use FPU
        "call fpu_nm_handler_rust",

        "pop rsi",
        "pop rdi",
        "pop rax",
        "iretq",
    );
}

/// #MF — x87 FP Exception. Kill current process.
#[unsafe(naked)]
unsafe extern "C" fn isr_stub_x87_fp() {
    naked_asm!(
        "push rax",
        "push rdi",
        "push rsi",
        "mov rdi, 16",         // vector = #MF (16)
        "xor rsi, rsi",        // error code = 0
        "call exception_kill_handler_rust",
        "pop rsi",
        "pop rdi",
        "pop rax",
        "iretq",
    );
}

/// #XM — SIMD/AVX Exception. Kill current process.
#[unsafe(naked)]
unsafe extern "C" fn isr_stub_simd_fp() {
    naked_asm!(
        "push rax",
        "push rdi",
        "push rsi",
        "mov rdi, 19",         // vector = #XM (19)
        "xor rsi, rsi",        // error code = 0
        "call exception_kill_handler_rust",
        "pop rsi",
        "pop rdi",
        "pop rax",
        "iretq",
    );
}

/// #DE — Divide Error. Kill current process.
#[unsafe(naked)]
unsafe extern "C" fn isr_stub_divide_error() {
    naked_asm!(
        "push rax",
        "push rdi",
        "push rsi",
        "mov rdi, 0",          // vector = #DE (0)
        "xor rsi, rsi",        // error code = 0
        "call exception_kill_handler_rust",
        "pop rsi",
        "pop rdi",
        "pop rax",
        "iretq",
    );
}

/// APIC Timer (vector 48) — saves full context, calls scheduler tick, sends EOI.
#[unsafe(naked)]
unsafe extern "C" fn isr_stub_apic_timer() {
    naked_asm!(
        "push rax",
        "push rcx",
        "push rdx",
        "push rsi",
        "push rdi",
        "push r8",
        "push r9",
        "push r10",
        "push r11",
        "call apic_timer_handler_rust",
        "pop r11",
        "pop r10",
        "pop r9",
        "pop r8",
        "pop rdi",
        "pop rsi",
        "pop rdx",
        "pop rcx",
        "pop rax",
        "iretq",
    );
}

/// IRQ0 — PIT timer (vector 32). Just iretq for now to prevent crashes.
#[unsafe(naked)]
unsafe extern "C" fn isr_stub_irq0() {
    naked_asm!(
        "iretq",
    );
}

/// IRQ1 — PS/2 keyboard (vector 33). Saves all caller-saved regs, calls Rust handler.
#[unsafe(naked)]
unsafe extern "C" fn isr_stub_irq1() {
    naked_asm!(
        "push rax",
        "push rcx",
        "push rdx",
        "push rsi",
        "push rdi",
        "push r8",
        "push r9",
        "push r10",
        "push r11",
        "call irq1_handler_rust",
        "pop r11",
        "pop r10",
        "pop r9",
        "pop r8",
        "pop rdi",
        "pop rsi",
        "pop rdx",
        "pop rcx",
        "pop rax",
        "iretq",
    );
}

// ── Rust handler functions called from ISR stubs ────────────────────────────

#[unsafe(no_mangle)]
extern "C" fn irq0_handler_rust() {
    unsafe {
        if let Some(handler) = IRQ_HANDLERS[0] {
            handler();
        }
    }
}

#[unsafe(no_mangle)]
extern "C" fn irq1_handler_rust() {
    crate::diag::telemetry::t().cpu.interrupts.fetch_add(1, core::sync::atomic::Ordering::Relaxed);
    unsafe {
        if let Some(handler) = IRQ_HANDLERS[1] {
            handler();
        }
    }
}

/// APIC timer interrupt handler — tick scheduler + telemetry refresh + EOI.
#[unsafe(no_mangle)]
extern "C" fn apic_timer_handler_rust() {
    crate::diag::telemetry::t().cpu.timer_ticks.fetch_add(1, core::sync::atomic::Ordering::Relaxed);
    crate::sched::timer_tick();
    crate::diag::tick_refresh();
    crate::arch::apic::apic_eoi();
}

/// Exception handler that kills the current process and returns to scheduler.
/// Called from #GP and #PF ISR stubs.
#[unsafe(no_mangle)]
extern "C" fn exception_kill_handler_rust(vector: u64, error: u64, cr2: u64) -> ! {
    use core::sync::atomic::Ordering;
    let t = crate::diag::telemetry::t();
    match vector {
        13 => {
            t.cpu.gp_faults.fetch_add(1, Ordering::Relaxed);
            crate::diag::fault_u64("#GP", "general protection fault", error);
        }
        14 => {
            t.cpu.page_faults.fetch_add(1, Ordering::Relaxed);
            crate::diag::fault_u64("#PF", "page fault at CR2", cr2);
            crate::diag::fault_u64("#PF", "page fault error code", error);
        }
        7 => {
            t.cpu.nm_faults.fetch_add(1, Ordering::Relaxed);
            crate::diag::fault_u64("#NM", "device not available", error);
        }
        8 => {
            t.cpu.df_faults.fetch_add(1, Ordering::Relaxed);
            crate::diag::fault_u64("#DF", "double fault", error);
        }
        6 => {
            t.cpu.ud_faults.fetch_add(1, Ordering::Relaxed);
            crate::diag::fault_u64("#UD", "invalid opcode", error);
        }
        18 => {
            t.cpu.mc_faults.fetch_add(1, Ordering::Relaxed);
            crate::diag::fault_u64("#MC", "machine check", error);
        }
        _ => {
            t.cpu.other_faults.fetch_add(1, Ordering::Relaxed);
            crate::diag::fault_u64("trap", "fatal CPU exception", vector);
        }
    }

    // Kill current process and switch to scheduler
    crate::sched::process::kill_current_process(vector, error, cr2)
}

/// #NM handler for lazy FPU context switching.
///
/// Called when CR0.TS is set and a FPU/SSE/AVX instruction is executed.
/// Clears TS so the current task can use FPU/SSE/AVX.
/// Full FPU state save/restore will be integrated with the scheduler.
#[unsafe(no_mangle)]
extern "C" fn fpu_nm_handler_rust() {
    // Clear CR0.TS to allow FPU/SSE/AVX instructions
    crate::arch::fpu::clear_lazy_fpu();
}
