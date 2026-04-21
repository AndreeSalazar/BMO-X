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

        // IRQ0 — PIT timer (vector 32)
        IDT[32].set_handler(isr_stub_irq0 as *const () as u64);

        // IRQ1 — PS/2 keyboard (vector 33)
        IDT[33].set_handler(isr_stub_irq1 as *const () as u64);

        // Remaining IRQs (34-47) — default
        for i in 34..48 {
            IDT[i].set_handler(isr_stub_default_irq as *const () as u64);
        }

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

/// Default IRQ handler (vectors 34-47) — just sends EOI.
#[unsafe(naked)]
unsafe extern "C" fn isr_stub_default_irq() {
    naked_asm!(
        "push rax",
        "mov al, 0x20",
        "out 0x20, al",   // EOI to master PIC
        "pop rax",
        "iretq",
    );
}

/// IRQ0 — PIT timer (vector 32). Saves all caller-saved regs, calls Rust handler.
#[unsafe(naked)]
unsafe extern "C" fn isr_stub_irq0() {
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
        "call irq0_handler_rust",
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
    super::pic::send_eoi(0);
}

#[unsafe(no_mangle)]
extern "C" fn irq1_handler_rust() {
    unsafe {
        if let Some(handler) = IRQ_HANDLERS[1] {
            handler();
        }
    }
    super::pic::send_eoi(1);
}
