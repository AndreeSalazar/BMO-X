//! BSP local-APIC periodic tick on the unified trap frame.
//!
//! `s2_mem` calibrates and starts LAPIC vector 48. Ring 0 replaces that
//! vector's boot-time halt stub with this handler, which shares the exact
//! frame layout and epilogue with the SYSCALL entry: contexts captured here
//! and contexts captured there are interchangeable to the scheduler.

use boot_context::BootContext;
use core::arch::{asm, naked_asm};
use core::sync::atomic::{AtomicU64, Ordering};

use crate::ring0::percpu;
use crate::ring0::trap::TrapFrame;

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
        // The CPU pushed rip/cs/rflags (plus rsp/ss from Ring 3). swapgs only
        // if we interrupted userland; Ring 0 already runs on its PerCpu GS.
        "cmp qword ptr [rsp+8], 0x08",
        "je 1f",
        "swapgs",
        // 15 GPRs, same push order as the SYSCALL entry.
        "1: push rax", "push rcx", "push rdx", "push rbx", "push rbp",
        "push rsi", "push rdi", "push r8", "push r9", "push r10",
        "push r11", "push r12", "push r13", "push r14", "push r15",
        "mov rbp, rsp",
        "sub rsp, {reserva}",
        "and rsp, -64",                // XSAVE exige 64 bytes de alineacion
        "mov [rsp+{area}], rbp",
        // RFBM = -1: lo que XCR0 tenga habilitado. rax/rdx ya estan salvados.
        "mov eax, -1", "mov edx, -1",
        "xsave64 [rsp]",
        "mov gs:[0x10], rsp",
        "cld",
        "mov rdi, rbp",
        "call {dispatch}",
        // Shared trap epilogue: rax = xsave-base of the context to run.
        "mov rsp, rax",
        "mov eax, -1", "mov edx, -1",
        "xrstor64 [rsp]",
        "mov rsp, [rsp+{area}]",
        "pop r15", "pop r14", "pop r13", "pop r12", "pop r11",
        "pop r10", "pop r9", "pop r8", "pop rdi", "pop rsi",
        "pop rbp", "pop rbx", "pop rdx", "pop rcx", "pop rax",
        "cmp qword ptr [rsp+8], 0x08",
        "je 2f",
        "swapgs",
        "2: iretq",
        dispatch = sym timer_dispatch,
        area = const crate::ring0::trap::XSAVE_AREA,
        reserva = const crate::ring0::trap::XSAVE_RESERVA,
    );
}

#[unsafe(naked)]
unsafe extern "C" fn spurious_entry() -> ! {
    // A LAPIC spurious interrupt has no in-service bit and needs no EOI.
    naked_asm!("iretq");
}

#[unsafe(no_mangle)]
extern "C" fn timer_dispatch(_frame: &mut TrapFrame) -> u64 {
    let n = TICKS.fetch_add(1, Ordering::Relaxed) + 1;
    // Budgeted estuary service before the scheduler decision: pending
    // submissions become completions and their WAITers wake this tick.
    // Must run before on_timer so no scheduler lock is held here.
    crate::ring0::channel::service_all();
    crate::ring0::scheduler::on_timer();
    // Heartbeat from IRQ context every 64 ticks: paints even when no task
    // ever reaches the shell poll loop. If this row stops updating, ticks
    // stopped — the hang is running with IF masked (a syscall dispatch or a
    // cli section); if it updates, the counters say where execution lives.
    // NOTA: CABINA se pinta desde el loop del shell (bajo CR3 del kernel,
    // seguro). Pintar desde AQUI (contexto IRQ: switch de CR3 + 4 filas de
    // framebuffer) era pesado y causaba cuelgue->reset en el arranque. El
    // shell ya la mantiene always-on, asi que este llamado sobra.
    let _ = n;

    // SAFETY: initialized before vector 48 is installed and only used by the
    // BSP while SMP is disabled.
    unsafe { LAPIC_EOI.write_volatile(0) };
    percpu::trap_rsp()
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
