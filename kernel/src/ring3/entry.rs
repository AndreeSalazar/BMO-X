//! Ring 3 Entry — Orchestrador para entrar a modo usuario.
//!
//! `prepare_task_for_ring3` configura un Task para Ring 3.
//! `is_in_ring3` / `current_privilege_level` consultan el nivel actual.
//! La transición CPU real (iretq) está en `transition.rs`.
//!
//! # Dependencia
//!
//! Ninguna — solo usa tipos de kernel. La implementación pesada
//! (BEF loader, gateway, desktop) vive en crates_Personal/.

/// Prepara un Task para ejecución en Ring 3 vía scheduler/context switch.
///
/// Configura los registros salvados para que la próxima vez que el
/// scheduler elija este task, entre a Ring 3 vía iretq.
pub fn prepare_task_for_ring3(
    task: &mut crate::proc::task::Task,
    entry_point: u64,
    user_stack: u64,
) {
    use crate::arch::gdt::{USER_CS, USER_DS};
    const USER_RFLAGS: u64 = 0x202;

    task.regs.rip = entry_point;
    task.regs.rsp = user_stack;
    task.regs.cs = USER_CS as u64;
    task.regs.ss = USER_DS as u64;
    task.regs.rflags = USER_RFLAGS;
    task.regs.rax = 0;
    task.regs.rbx = 0;
    task.regs.rcx = 0;
    task.regs.rdx = 0;
    task.regs.rsi = 0;
    task.regs.rdi = 0;
    task.regs.rbp = 0;
    task.regs.r8 = 0;
    task.regs.r9 = 0;
    task.regs.r10 = 0;
    task.regs.r11 = 0;
    task.regs.r12 = 0;
    task.regs.r13 = 0;
    task.regs.r14 = 0;
    task.regs.r15 = 0;
}

/// Verifica si la CPU está actualmente en Ring 3 (CPL=3).
pub fn is_in_ring3() -> bool {
    let cs: u16;
    unsafe {
        core::arch::asm!("mov {0:x}, cs", out(reg) cs, options(nostack, nomem));
    }
    (cs & 0x3) == 3
}

/// Nivel de privilegio actual: 0 = Ring 0, 3 = Ring 3.
pub fn current_privilege_level() -> u8 {
    let cs: u16;
    unsafe {
        core::arch::asm!("mov {0:x}, cs", out(reg) cs, options(nostack, nomem));
    }
    (cs & 0x3) as u8
}
