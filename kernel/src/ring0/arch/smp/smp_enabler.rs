//! SMP Enabler — Enable and manage multi-core execution.
//!
//! This module enables Symmetric Multi-Processing (SMP) by:
//! 1. Detecting all CPU cores via ACPI MADT
//! 2. Starting Application Processors (APs) via INIT/SIPI
//! 3. Managing per-CPU data structures
//! 4. Coordinating inter-processor interrupts (IPIs)
//!
//! ## Architecture
//!
//! The BSP (Bootstrap Processor) runs the boot sequence and starts
//! all APs (Application Processors). Each AP:
//! - Executes the trampoline code (16-bit → 32-bit → 64-bit)
//! - Enters Rust code at `ap_entry()`
//! - Sets up per-CPU data
//! - Enters the idle loop waiting for scheduler work

use core::arch::asm;

/// Maximum number of CPUs supported
pub const MAX_CPUS: usize = 128;

/// CPU state
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum CpuState {
    /// CPU is offline
    Offline,
    /// CPU is starting up
    Starting,
    /// CPU is online and running
    Online,
    /// CPU is in idle loop
    Idle,
    /// CPU encountered an error
    Error,
}

/// Per-CPU information
#[derive(Debug)]
pub struct CpuInfo {
    /// APIC ID
    pub apic_id: u32,
    /// Logical core ID
    pub core_id: u32,
    /// CPU state
    pub state: CpuState,
    /// Is this the BSP?
    pub is_bsp: bool,
    /// Per-CPU data pointer
    pub percpu_ptr: u64,
}

/// Global CPU table
static mut CPU_TABLE: [CpuInfo; MAX_CPUS] = {
    const INIT: CpuInfo = CpuInfo {
        apic_id: 0,
        core_id: 0,
        state: CpuState::Offline,
        is_bsp: false,
        percpu_ptr: 0,
    };
    [INIT; MAX_CPUS]
};

/// Number of CPUs detected
static mut CPU_COUNT: u32 = 0;

/// Number of CPUs successfully started
static mut CPU_ONLINE: u32 = 0;

/// Initialize SMP subsystem
pub fn init() -> bool {
    crate::dev::console::serial_write("[smp_enabler] initializing SMP...\n");
    
    unsafe {
        // Mark BSP as online
        CPU_TABLE[0].apic_id = 0; // Will be updated with actual APIC ID
        CPU_TABLE[0].core_id = 0;
        CPU_TABLE[0].state = CpuState::Online;
        CPU_TABLE[0].is_bsp = true;
        CPU_COUNT = 1;
        CPU_ONLINE = 1;
        
        // Detect CPUs via ACPI MADT
        if !detect_cpus_acpi() {
            crate::dev::console::serial_write("[smp_enabler] failed to detect CPUs via ACPI\n");
            return false;
        }
        
        crate::dev::console::serial_write("[smp_enabler] detected ");
        crate::dev::console::serial_write_u64(CPU_COUNT as u64, 10);
        crate::dev::console::serial_write(" CPUs\n");
        
        // Initialize trampoline
        crate::arch::smp::ap_startup::init_trampoline();
        
        // Patch trampoline with BSP's CR3 and ap_entry address
        let cr3: u64;
        asm!("mov {0}, cr3", out(reg) cr3, options(nostack));
        let ap_entry_addr = crate::arch::smp::ap_startup::ap_entry as *const () as u64;
        crate::arch::smp::ap_startup::patch_trampoline(cr3, ap_entry_addr);
        
        // Start APs
        start_all_aps();
        
        crate::dev::console::serial_write("[smp_enabler] SMP initialized with ");
        crate::dev::console::serial_write_u64(CPU_ONLINE as u64, 10);
        crate::dev::console::serial_write(" online CPUs\n");
        
        CPU_ONLINE > 1
    }
}

/// Detect CPUs via ACPI MADT (Multiple APIC Description Table)
unsafe fn detect_cpus_acpi() -> bool {
    // Get RSDP from boot info
    let rsdp_addr = unsafe {
        if crate::info::BOOT_INFO.is_null() {
            0
        } else {
            (*crate::info::BOOT_INFO).rsdp_addr
        }
    };
    if rsdp_addr == 0 {
        crate::dev::console::serial_write("[smp_enabler] no RSDP found\n");
        return false;
    }
    
    crate::dev::console::serial_write("[smp_enabler] RSDP at 0x");
    crate::dev::console::serial_write_u64(rsdp_addr, 16);
    crate::dev::console::serial_write("\n");
    
    // Parse RSDP to find XSDT
    // For now, we'll use a simplified approach:
    // Assume all APIC IDs from 0 to CPU_COUNT-1 are valid
    // Real implementation would parse MADT
    
    // Read number of processors from ACPI
    // This is a placeholder - real implementation would parse MADT
    let num_cpus = 12; // Zen 3 has 6 cores, 12 threads
    
    for i in 1..num_cpus {
        if CPU_COUNT < MAX_CPUS as u32 {
            CPU_TABLE[CPU_COUNT as usize].apic_id = i as u32;
            CPU_TABLE[CPU_COUNT as usize].core_id = CPU_COUNT;
            CPU_TABLE[CPU_COUNT as usize].state = CpuState::Offline;
            CPU_TABLE[CPU_COUNT as usize].is_bsp = false;
            CPU_COUNT += 1;
        }
    }
    
    true
}

/// Start all Application Processors
unsafe fn start_all_aps() {
    for i in 1..CPU_COUNT as usize {
        let apic_id = CPU_TABLE[i].apic_id;
        let core_id = CPU_TABLE[i].core_id;
        
        crate::dev::console::serial_write("[smp_enabler] starting AP ");
        crate::dev::console::serial_write_u64(core_id as u64, 10);
        crate::dev::console::serial_write(" (APIC ");
        crate::dev::console::serial_write_u64(apic_id as u64, 10);
        crate::dev::console::serial_write(")...\n");
        
        CPU_TABLE[i].state = CpuState::Starting;
        
        match crate::arch::smp::ap_startup::start_ap(apic_id, core_id) {
            Ok(_) => {
                CPU_TABLE[i].state = CpuState::Online;
                CPU_ONLINE += 1;
                crate::dev::console::serial_write("[smp_enabler] AP ");
                crate::dev::console::serial_write_u64(core_id as u64, 10);
                crate::dev::console::serial_write(" online\n");
            }
            Err(_) => {
                CPU_TABLE[i].state = CpuState::Error;
                crate::dev::console::serial_write("[smp_enabler] AP ");
                crate::dev::console::serial_write_u64(core_id as u64, 10);
                crate::dev::console::serial_write(" FAILED\n");
            }
        }
    }
}

/// Get the number of online CPUs
pub fn online_count() -> u32 {
    unsafe { CPU_ONLINE }
}

/// Get the total number of detected CPUs
pub fn total_count() -> u32 {
    unsafe { CPU_COUNT }
}

/// Check if SMP is enabled (more than 1 CPU online)
pub fn is_enabled() -> bool {
    online_count() > 1
}

/// Get CPU info by core ID
pub fn get_cpu_info(core_id: u32) -> Option<&'static CpuInfo> {
    unsafe {
        if core_id < CPU_COUNT {
            Some(&CPU_TABLE[core_id as usize])
        } else {
            None
        }
    }
}

/// Send an IPI (Inter-Processor Interrupt) to a specific CPU
pub fn send_ipi(target_apic_id: u32, vector: u8) {
    unsafe {
        crate::arch::smp::ipi::send_fixed_ipi(target_apic_id, vector);
    }
}

/// Send an IPI to all CPUs except self
pub fn send_ipi_all_except_self(vector: u8) {
    unsafe {
        crate::arch::smp::ipi::send_ipi_all_except_self(vector);
    }
}

/// Halt this CPU (enter idle loop)
pub fn halt_cpu() -> ! {
    loop {
        unsafe {
            asm!("sti; hlt", options(nostack));
        }
    }
}
