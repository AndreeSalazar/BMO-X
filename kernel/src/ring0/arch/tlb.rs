//! TLB Shootdown (Ring 0 HAL).
//!
//! Broadcasts TLB invalidation to all cores when a page mapping changes.
//! Without TLB shootdown, stale translations can cause data corruption.
//!
//! Architecture:
//!   - BSP sends IPI to all AP cores
//!   - Each AP executes INVLPG for the affected address
//!   - BSP waits for all APs to acknowledge

use core::arch::asm;

/// Send a TLB shootdown for a specific address to all cores.
///
/// # Safety
/// The address must be page-aligned.
pub unsafe fn shootdown_page(addr: u64) {
    let core_count = crate::arch::smp::core_count();
    if core_count <= 1 {
        asm!("invlpg [{}]", in(reg) addr, options(nostack));
        return;
    }

    // Broadcast INVLPG via IPI to all AP cores
    crate::arch::smp::ipi::broadcast_tlb_flush(addr);

    // BSP invalidates its own TLB
    asm!("invlpg [{}]", in(reg) addr, options(nostack));
}

/// Flush entire TLB on all cores (full reload).
pub unsafe fn flush_all() {
    let core_count = crate::arch::smp::core_count();
    if core_count <= 1 {
        // Single core: just reload CR3
        let cr3: u64;
        asm!("mov {}, cr3", out(reg) cr3, options(nostack));
        asm!("mov cr3, {}", in(reg) cr3, options(nostack));
        return;
    }

    // Broadcast to all cores, then reload our own
    crate::arch::smp::ipi::broadcast_tlb_flush(0);
    let cr3: u64;
    asm!("mov {}, cr3", out(reg) cr3, options(nostack));
    asm!("mov cr3, {}", in(reg) cr3, options(nostack));
}
