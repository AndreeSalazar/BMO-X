//! BSP local-APIC periodic tick.
//!
//! `s2_mem` calibrates and starts LAPIC vector 48. Ring 0 replaces that
//! vector's boot-time halt stub with this bounded accounting handler.

use boot_context::BootContext;
use core::arch::{asm, naked_asm};
use core::sync::atomic::{AtomicU64, Ordering};

const TIMER_VECTOR: usize = 48;
const SPURIOUS_VECTOR: usize = 0xFF;
const KERNEL_CS: u16 = 0x08;
const HIGH_MEM_BASE: u64 = 0xFFFF_8000_0000_0000;
const MSR_APIC_BASE: u32 = 0x1B;
const APIC_BASE_MASK: u64 = 0x000F_FFFF_FFFF_F000;
const APIC_ENABLED: u64 = 1 << 11;
const APIC_X2_MODE: u64 = 1 << 10;
const LAPIC_EOI_OFFSET: u64 = 0xB0;
const LAPIC_LVT_TIMER_OFFSET: u64 = 0x320;
const LAPIC_LVT_MASKED: u32 = 1 << 16;

static TICKS: AtomicU64 = AtomicU64::new(0);
static mut LAPIC_EOI: *mut u32 = core::ptr::null_mut();

#[repr(C, packed)]
struct IdtEntry {
    offset_low: u16,
    selector: u16,
    ist: u8,
    attributes: u8,
    offset_mid: u16,
    offset_high: u32,
    reserved: u32,
}

impl IdtEntry {
    fn interrupt_gate(handler: u64) -> Self {
        Self {
            offset_low: handler as u16,
            selector: KERNEL_CS,
            ist: 0,
            attributes: 0x8E,
            offset_mid: (handler >> 16) as u16,
            offset_high: (handler >> 32) as u32,
            reserved: 0,
        }
    }
}

#[unsafe(naked)]
unsafe extern "C" fn timer_entry() -> ! {
    naked_asm!(
        "push rax", "push rcx", "push rdx", "push rbx", "push rbp",
        "push rsi", "push rdi", "push r8", "push r9", "push r10",
        "push r11", "push r12", "push r13", "push r14", "push r15",
        "mov rbp, rsp",
        // Rust may use caller-clobbered SIMD registers. Preserve the complete
        // legacy x87/SSE state because an interrupt has no normal caller.
        "sub rsp, 528",
        "and rsp, -16",
        "fxsave64 [rsp]",
        "cld",
        "call {dispatch}",
        "fxrstor64 [rsp]",
        "mov rsp, rbp",
        "pop r15", "pop r14", "pop r13", "pop r12", "pop r11",
        "pop r10", "pop r9", "pop r8", "pop rdi", "pop rsi",
        "pop rbp", "pop rbx", "pop rdx", "pop rcx", "pop rax",
        "iretq",
        dispatch = sym timer_dispatch,
    );
}

#[unsafe(naked)]
unsafe extern "C" fn spurious_entry() -> ! {
    // A LAPIC spurious interrupt has no in-service bit and needs no EOI.
    naked_asm!("iretq");
}

#[unsafe(no_mangle)]
extern "C" fn timer_dispatch() {
    TICKS.fetch_add(1, Ordering::Relaxed);
    let _next_tid = crate::ring0::scheduler::tick();

    // SAFETY: initialized before vector 48 is installed and only used by the
    // BSP while SMP is disabled.
    unsafe { LAPIC_EOI.write_volatile(0) };
}

unsafe fn rdmsr(msr: u32) -> u64 {
    let low: u32;
    let high: u32;
    asm!("rdmsr", in("ecx") msr, out("eax") low, out("edx") high, options(nostack));
    ((high as u64) << 32) | low as u64
}

pub fn init(ctx: &BootContext) -> bool {
    if ctx.idt_ptr == 0 {
        return false;
    }

    let apic = unsafe { rdmsr(MSR_APIC_BASE) };
    if apic & APIC_ENABLED == 0 || apic & APIC_X2_MODE != 0 {
        return false;
    }

    let lapic_base = HIGH_MEM_BASE + (apic & APIC_BASE_MASK);
    let lvt_timer = unsafe {
        ((lapic_base + LAPIC_LVT_TIMER_OFFSET) as *const u32).read_volatile()
    };
    if lvt_timer & LAPIC_LVT_MASKED != 0 || lvt_timer as u8 as usize != TIMER_VECTOR {
        return false;
    }

    let flags: u64;
    unsafe {
        asm!("pushfq", "pop {}", "cli", out(reg) flags);
        LAPIC_EOI = (lapic_base + LAPIC_EOI_OFFSET) as *mut u32;
        let idt = ctx.idt_ptr as *mut IdtEntry;
        idt.add(TIMER_VECTOR).write_volatile(IdtEntry::interrupt_gate(timer_entry as *const () as u64));
        idt.add(SPURIOUS_VECTOR).write_volatile(IdtEntry::interrupt_gate(spurious_entry as *const () as u64));
        if flags & (1 << 9) != 0 {
            asm!("sti", options(nomem, nostack));
        }
    }
    true
}

pub fn ticks() -> u64 {
    TICKS.load(Ordering::Relaxed)
}

/// Enable maskable interrupts only after every Ring 0 subsystem is ready.
pub fn enable() {
    unsafe { asm!("sti", options(nomem, nostack)); }
}
