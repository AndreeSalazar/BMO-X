//! TLB Shootdown — invalidates TLB entries on the local CPU.
//!
//! On a single-core system, the only thing required is an `invlpg`
//! (or `mov cr3, cr3` for a full flush). When SMP is enabled, an IPI
//! must be sent to other cores so they invalidate the same entries.
//! This file provides the single-core implementation and a TODO hook
//! for the IPI path.

use core::arch::asm;

/// Invalidate a single TLB entry for the virtual address `vaddr`.
///
/// # Safety
/// Caller must guarantee `vaddr` is a valid canonical address.
#[inline]
pub unsafe fn invlpg(vaddr: u64) {
    asm!("invlpg [{}]", in(reg) vaddr, options(nostack, preserves_flags));
}

/// Flush the entire TLB on the local CPU by reloading CR3.
///
/// # Safety
/// Caller must guarantee that all other CPU-local TLB state is OK to drop.
#[inline]
pub unsafe fn flush_all() {
    let cr3: u64;
    asm!("mov {}, cr3", out(reg) cr3, options(nostack, preserves_flags));
    asm!("mov cr3, {}", in(reg) cr3, options(nostack, preserves_flags));
}

/// TLB shootdown stub. On SMP this would send an IPI to all other cores
/// and wait for an ACK. For now, just flush locally.
pub fn shootdown(vaddr: u64) {
    unsafe { invlpg(vaddr); }
}

/// TLB shootdown for a full address space. Used on process teardown.
pub fn flush_full() {
    unsafe { flush_all(); }
}

