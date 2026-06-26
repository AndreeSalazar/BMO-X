//! SMP — Symmetric Multi-Processing coordinator.
//!
//! Orchestrates AP core bring-up and provides the public SMP API.
//!
//! Architecture:
//!   - BSP (Bootstrap Processor) runs the main kernel
//!   - APs (Application Processors) are brought up via INIT/SIPI
//!   - Each core gets per-CPU data via GS-base
//!   - I/O APIC routes IRQs to specific cores
//!   - IPIs enable inter-core communication
//!
//! This module is arch-agnostic: it works on Intel and AMD x86-64.

#![allow(dead_code)]

pub mod percpu;
pub mod ap_startup;
pub mod ioapic;
pub mod ipi;

use core::arch::asm;

/// SMP state.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SmpState {
    /// Not yet initialized.
    Uninitialized,
    /// BSP is initializing AP cores.
    Starting,
    /// All online cores are running.
    Running,
    /// One or more AP cores failed to start.
    Degraded,
}

/// Global SMP state.
static mut SMP_STATE: SmpState = SmpState::Uninitialized;

/// Number of cores that successfully started.
static mut CORES_ONLINE: u32 = 0;

/// Get the current SMP state.
pub fn state() -> SmpState {
    unsafe { SMP_STATE }
}

/// Get the number of online cores.
pub fn core_count() -> u32 {
    unsafe { CORES_ONLINE }
}

/// Initialize SMP. Called once from the BSP during boot.
///
/// Steps:
///   1. Detect CPU topology
///   2. Set up per-CPU data for BSP
///   3. Initialize I/O APIC
///   4. Patch and copy trampoline to low memory
///   5. Start each AP core via INIT/SIPI
///   6. Enable APIC timer on BSP
pub unsafe fn init() {
    crate::dev::console::serial_write("\n[smp] === SMP initialization ===\n");

    // 1. Detect topology (if not already done)
    let topo = match crate::vendor::amd::cpu::zen3::fastos_cpu::topology() {
        Some(t) => t.clone(),
        None => {
            crate::dev::console::serial_write("[smp] ERROR: topology not detected\n");
            return;
        }
    };

    let total_threads = topo.cpu_count;
    crate::dev::console::serial_write("[smp] topology: ");
    crate::dev::console::serial_write_u64(total_threads as u64, 10);
    crate::dev::console::serial_write(" threads, ");
    crate::dev::console::serial_write_u64(topo.total_cores as u64, 10);
    crate::dev::console::serial_write(" cores\n");

    if total_threads <= 1 {
        crate::dev::console::serial_write("[smp] single CPU, SMP not needed\n");
        SMP_STATE = SmpState::Running;
        CORES_ONLINE = 1;
        return;
    }

    SMP_STATE = SmpState::Starting;

    // 2. Set up per-CPU data for BSP
    let bsp_stack = {
        extern "C" { static __bss_end: u8; }
        &__bss_end as *const u8 as u64 + 4 * 1024 * 1024 // 4 MB above BSS
    };
    percpu::init_bsp(topo.bsp.apic_id as u32, bsp_stack);
    crate::dev::console::serial_write("[smp] BSP per-CPU initialized\n");

    // 3. Initialize I/O APIC (try standard address first)
    let ioapic_base = ioapic::probe().unwrap_or(0xFEC0_0000);
    ioapic::init_ioapic(ioapic_base);

    // 4. Initialize trampoline
    let cr3: u64;
    asm!("mov {}, cr3", out(reg) cr3, options(nostack));

    // Get ap_entry address
    let ap_entry_addr = ap_startup::ap_entry as *const () as u64;

    ap_startup::init_trampoline();
    ap_startup::patch_trampoline(cr3, ap_entry_addr);

    // 5. Start AP cores
    let mut started = 1u32; // BSP is core 0
    for i in 1..total_threads {
        let cpu_info = &topo.cpus[i as usize];
        let apic_id = cpu_info.apic_id as u32;

        match ap_startup::start_ap(apic_id, started) {
            Ok(core_id) => {
                started += 1;
                crate::dev::console::serial_write("[smp] AP core ");
                crate::dev::console::serial_write_u64(core_id as u64, 10);
                crate::dev::console::serial_write(" online (APIC ");
                crate::dev::console::serial_write_u64(apic_id as u64, 10);
                crate::dev::console::serial_write(")\n");
            }
            Err(()) => {
                crate::dev::console::serial_write("[smp] AP APIC ");
                crate::dev::console::serial_write_u64(apic_id as u64, 10);
                crate::dev::console::serial_write(" failed\n");
            }
        }
    }

    CORES_ONLINE = started;
    if started == total_threads {
        SMP_STATE = SmpState::Running;
    } else {
        SMP_STATE = SmpState::Degraded;
    }

    crate::dev::console::serial_write("[smp] === ");
    crate::dev::console::serial_write_u64(started as u64, 10);
    crate::dev::console::serial_write("/");
    crate::dev::console::serial_write_u64(total_threads as u64, 10);
    crate::dev::console::serial_write(" cores online ===\n\n");
}

/// Halt the current CPU. Used for idle loop.
#[inline]
pub fn halt() {
    unsafe {
        asm!("sti; hlt", options(nostack));
    }
}

/// Halt without enabling interrupts (used during critical sections).
#[inline]
pub fn halt_no_irq() {
    unsafe {
        asm!("hlt", options(nostack));
    }
}

/// Get the current CPU's core ID.
#[inline]
pub fn core_id() -> u32 {
    percpu::current().core_id
}

/// Get the current CPU's APIC ID.
#[inline]
pub fn apic_id() -> u32 {
    percpu::current().apic_id
}

/// Is this the BSP (Bootstrap Processor)?
#[inline]
pub fn is_bsp() -> bool {
    percpu::current().core_id == 0
}

/// Send a function call to a specific core.
/// The function pointer is stored in the per-CPU call queue
/// and the target core executes it when it receives the IPI.
pub unsafe fn call_on_core(core_id: u32, func: fn()) {
    if let Some(pc) = percpu::get(core_id) {
        if !pc.online { return; }
        // Store the function pointer in a per-CPU slot
        // For now, use a simple approach: store in saved_rsp temporarily
        // TODO: implement proper per-CPU call queue
        let pc_mut = percpu::current_mut();
        let old_rsp = pc_mut.saved_rsp;
        pc_mut.saved_rsp = func as u64;
        ipi::send_call_ipi(pc.apic_id);
        pc_mut.saved_rsp = old_rsp;
    }
}

/// Broadcast a function call to all cores (including self).
pub unsafe fn call_on_all(func: fn()) {
    for i in 0..percpu::online_count() {
        if let Some(pc) = percpu::get(i) {
            if i == core_id() {
                // Execute directly on BSP
                func();
            } else {
                // Send IPI to AP
                call_on_core(i, func);
            }
        }
    }
}
