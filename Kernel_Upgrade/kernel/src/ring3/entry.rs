//! Ring 3 Entry — configura un Task para Ring 3.

pub fn prepare_task_for_ring3(
    _task: *mut u8,
    entry_point: u64,
    user_stack: u64,
) {
    // Stub: the actual transition is done via hal_ring3_transition
}

pub fn is_in_ring3() -> bool {
    let cs: u16;
    unsafe { core::arch::asm!("mov {0:x}, cs", out(reg) cs, options(nostack, nomem)); }
    (cs & 0x3) == 3
}

pub fn current_privilege_level() -> u8 {
    let cs: u16;
    unsafe { core::arch::asm!("mov {0:x}, cs", out(reg) cs, options(nostack, nomem)); }
    (cs & 0x3) as u8
}
