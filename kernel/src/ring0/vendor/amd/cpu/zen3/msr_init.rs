//! MSR initialization for the Ryzen 5 5600X.
//!
//! Implements `AMD/ryzen_5_5600x.md` Â§10 + Â§11 (SYSCALL/SYSRET setup).
//!
//! Configures all the MSRs that RING 0 needs to bootstrap a 64-bit
//! environment, including:
//! - IA32_EFER: enable SYSCALL/SYSRET, NXE, LMA, LME
//! - IA32_STAR: kernel/user code/stack selectors
//! - IA32_LSTAR: 64-bit syscall entry point
//! - IA32_FMASK: which RFLAGS bits to clear on SYSCALL
//! - IA32_KERNEL_GS_BASE / IA32_GS_BASE: per-CPU GS for swapgs
//! - IA32_PAT: page attribute table (WC for framebuffer)
//! - IA32_TSC_AUX: BSP core ID for RDTSCP serialization
//!
//! Status: âœ… COMPLETO â€” implementaciÃ³n completa de MSR setup.
//!
//! References:
//! - AMD64 APM Vol. 2, Â§6.1 (MSR addressing)
//! - AMD64 APM Vol. 2, Â§11.1 (SYSCALL/SYSRET)
//! - AMD Zen 3 Family 19h BKDG, Â§3.12 (CPU setup)

use super::msr_definitions::{rdmsr, wrmsr, MSR_IA32_EFER, MSR_IA32_STAR,
                              MSR_IA32_LSTAR, MSR_IA32_CSTAR, MSR_IA32_FMASK,
                              MSR_IA32_GS_BASE, MSR_IA32_KERNEL_GS_BASE,
                              MSR_IA32_TSC_AUX, MSR_IA32_PAT,
                              MSR_IA32_TSC_DEADLINE};
use super::msr_definitions::efer;

/// Star selector values. Standard BMO layout:
/// - SYSCALL target: kernel CS=0x08, kernel SS=0x10.
/// - SYSRET base: STAR[63:48]=0x10 so AMD64 derives user SS=0x18 and
///   user CS=0x20, matching `arch::gdt`'s Ring 3 layout.
///
/// Keep this identical to `arch::syscall::init_syscall`; that function calls
/// this module after writing LSTAR, so a wrong STAR here would overwrite the
/// working syscall setup.
pub const STAR_VALUE: u64 = 0x0010_0008_0000_0000;

/// RFLAGS mask (cleared on SYSCALL). Disable interrupts (IF=0x200) and
/// direction flag (DF=0x400). User can re-enable with STI.
pub const FMASK_VALUE: u64 = 0x0000_0000_0000_0600;

/// Initialize all the MSRs that RING 0 needs to bootstrap a 64-bit
/// environment. Call this once on the BSP, very early.
///
/// `syscall_entry` is the address of the SYSCALL handler in Ring 0
/// (typically `arch::system_call_dispatcher::syscall_entry_naked`).
/// `bsp_apic_id` is the APIC ID of the BSP (used as TSC_AUX).
pub fn init_msr_common(syscall_entry: u64, bsp_apic_id: u32) {
    unsafe {
        // â”€â”€ IA32_EFER: enable SYSCALL/SYSRET, NXE, LMA, LME â”€â”€â”€â”€â”€â”€â”€â”€â”€
        let mut efer_val = rdmsr(MSR_IA32_EFER);
        efer_val |= efer::SCE;     // SYSCALL/SYSRET enable
        efer_val |= efer::NXE;     // No-Execute enable
        // LME and LMA should already be set by the bootloader.
        // We set LME explicitly to be safe (LMA is read-only).
        efer_val |= efer::LME;
        efer_val |= efer::FFXSR;   // Fast FXSAVE/FXSTOR
        wrmsr(MSR_IA32_EFER, efer_val);
        crate::dev::console::serial_write("[msr] IA32_EFER: ");
        crate::dev::console::serial_write_u64(efer_val, 16);
        crate::dev::console::serial_write("\n");

        // â”€â”€ IA32_STAR: kernel/user code/stack selectors â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€
        wrmsr(MSR_IA32_STAR, STAR_VALUE);
        crate::dev::console::serial_write("[msr] IA32_STAR: ");
        crate::dev::console::serial_write_u64(STAR_VALUE, 16);
        crate::dev::console::serial_write("\n");

        // â”€â”€ IA32_LSTAR: 64-bit SYSCALL entry â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€
        wrmsr(MSR_IA32_LSTAR, syscall_entry);
        crate::dev::console::serial_write("[msr] IA32_LSTAR: ");
        crate::dev::console::serial_write_u64(syscall_entry, 16);
        crate::dev::console::serial_write("\n");

        // â”€â”€ IA32_CSTAR: not used in 64-bit mode, but set to 0 â”€â”€â”€â”€â”€â”€â”€
        wrmsr(MSR_IA32_CSTAR, 0);

        // â”€â”€ IA32_FMASK: clear IF + DF on SYSCALL â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€
        wrmsr(MSR_IA32_FMASK, FMASK_VALUE);

        // â”€â”€ IA32_KERNEL_GS_BASE: kernel's GS (used with swapgs) â”€â”€â”€â”€â”€
        // Set to a per-CPU data pointer. For now, 0 â€” populate when
        // PerCpu is allocated.
        wrmsr(MSR_IA32_KERNEL_GS_BASE, 0);

        // â”€â”€ IA32_GS_BASE: user's GS (not used in Ring 0 directly) â”€â”€
        wrmsr(MSR_IA32_GS_BASE, 0);

        // â”€â”€ IA32_TSC_AUX: APIC ID of the BSP (for RDTSCP) â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€
        // RDTSCP returns TSC_AUX in ECX, useful for identifying
        // which core took a timestamp.
        wrmsr(MSR_IA32_TSC_AUX, bsp_apic_id as u64);

        // â”€â”€ IA32_PAT: Page Attribute Table â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€
        // The default PAT layout (after reset):
        //   PA0 = WB
        //   PA1 = WT
        //   PA2 = UC-
        //   PA3 = UC
        //   PA4 = WT (we change this to WC)
        //   PA5 = WP
        //   PA6 = WB
        //   PA7 = UC
        // We override PA4 to be WC (Write-Combining, used for framebuffer).
        // Encoding for entry N at bits [8N+2 : 8N].
        let pat_default: u64 = 0x0007_0406_0007_0406;  // standard layout
        // Replace PA4 (bits 41:40 in PAT): in the PA indexing convention,
        // PA0=bits[2:0], PA1=bits[10:8], ..., PA4=bits[42:40], PA5=bits[50:48]...
        // We set entry 4 to WC = 0b001 (per APM Vol 2 Table 7-9)
        let pat_new = (pat_default & !(0x7u64 << 40)) | (0x1u64 << 40);
        wrmsr(MSR_IA32_PAT, pat_new);

        // â”€â”€ IA32_TSC_DEADLINE: clear any pending TSC-deadline â”€â”€â”€â”€â”€â”€
        // (Important after warm reset.)
        // v1.8.17: Disabled here. Writing to TSC_DEADLINE when the local APIC
        // is software-disabled (the default state during Phase 0) triggers a #GP.
        // wrmsr(MSR_IA32_TSC_DEADLINE, 0);
    }
    crate::dev::console::serial_write("[msr] all common MSRs initialized\n");
}

/// Set the IA32_TSC_DEADLINE to a specific TSC value. The APIC timer
/// will fire an IRQ when TSC >= deadline. Used for high-precision
/// timers.
#[inline]
pub unsafe fn set_tsc_deadline(deadline_tsc: u64) {
    wrmsr(MSR_IA32_TSC_DEADLINE, deadline_tsc);
}

/// Read the current TSC deadline value.
#[inline]
pub unsafe fn read_tsc_deadline() -> u64 {
    rdmsr(MSR_IA32_TSC_DEADLINE)
}

/// Set the kernel GS base (used with `swapgs` instruction).
#[inline]
pub unsafe fn set_kernel_gs_base(base: u64) {
    wrmsr(MSR_IA32_KERNEL_GS_BASE, base);
}

/// Get the kernel GS base.
#[inline]
pub unsafe fn read_kernel_gs_base() -> u64 {
    rdmsr(MSR_IA32_KERNEL_GS_BASE)
}

/// Set the user GS base.
#[inline]
pub unsafe fn set_user_gs_base(base: u64) {
    wrmsr(MSR_IA32_GS_BASE, base);
}

/// Set the TSC_AUX value (APIC ID for RDTSCP).
#[inline]
pub unsafe fn set_tsc_aux(aux: u64) {
    wrmsr(MSR_IA32_TSC_AUX, aux);
}
