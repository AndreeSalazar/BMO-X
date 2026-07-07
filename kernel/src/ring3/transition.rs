//! Primitivo compartido: transición a Ring 3 vía iretq.
//!
//! `ring3_transition(entry, stack_top)` es el único lugar que hace iretq.
//! Tanto `jump_to_ring3()` (demo) como `run_entry_point()` (app loader)
//! lo llaman. Cada llamador es responsable del page table setup (USER flag).

/// Real CPU-level transition to Ring 3 via iretq.
///
/// # Safety
///
/// - `entry` y `stack_top` deben ser direcciones virtuales accesibles
///   desde Ring 3 (flag USER en page tables).
/// - El stack debe tener al menos 40 bytes para el frame iretq.
/// - CS=0x23 (Ring 3, 64-bit), SS=0x1B (Ring 3, data), RFLAGS=0x202 (IF=1).
#[inline(always)]
pub unsafe fn ring3_transition(entry: u64, stack_top: u64) -> ! {
    core::arch::asm!(
        "push qword ptr {user_ss}",
        "push {stack_top}",
        "push qword ptr 0x202",
        "push qword ptr {user_cs}",
        "push {entry}",
        "iretq",
        user_ss  = const 0x1B_u64,
        user_cs  = const 0x23_u64,
        stack_top = in(reg) stack_top,
        entry    = in(reg) entry,
        options(noreturn),
    );
}
