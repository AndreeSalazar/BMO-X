//! Ring 0 Integration — Connect all new subsystems.
//!
//! This module provides the integration layer that connects:
//! 1. APIC MMIO mapping (fixes #GP on timer access)
//! 2. Ring 3 entry (enables user-mode execution)
//! 3. AHCI driver (enables storage I/O)
//! 4. SMP enabler (enables multi-core execution)
//!
//! ## Boot Sequence Integration
//!
//! These subsystems should be initialized in this order:
//! 1. APIC MMIO (before any APIC access)
//! 2. SMP (before scheduler starts)
//! 3. AHCI (before filesystem access)
//! 4. Ring 3 entry (before user-mode apps start)

/// Initialize all Ring 0 subsystems in the correct order.
///
/// This should be called from the boot sequence after basic
/// hardware initialization but before starting user-mode.
pub fn init_all() -> bool {
    crate::dev::console::serial_write("\n[ring0_integration] === Initializing Ring 0 subsystems ===\n");
    
    // 1. Initialize APIC MMIO mapping
    crate::dev::console::serial_write("[ring0_integration] Step 1: APIC MMIO mapping...\n");
    match crate::irq::apic_mmio::init() {
        Ok(()) => {
            crate::dev::console::serial_write("[ring0_integration] APIC MMIO: OK\n");
        }
        Err(e) => {
            crate::dev::console::serial_write("[ring0_integration] APIC MMIO: FAILED - ");
            crate::dev::console::serial_write(e);
            crate::dev::console::serial_write("\n");
            return false;
        }
    }
    
    // 2. Initialize SMP
    crate::dev::console::serial_write("[ring0_integration] Step 2: SMP initialization...\n");
    if crate::arch::smp::smp_enabler::init() {
        crate::dev::console::serial_write("[ring0_integration] SMP: OK (");
        crate::dev::console::serial_write_u64(crate::arch::smp::smp_enabler::online_count() as u64, 10);
        crate::dev::console::serial_write(" cores online)\n");
    } else {
        crate::dev::console::serial_write("[ring0_integration] SMP: FAILED (continuing with single core)\n");
    }
    
    // 3. Initialize AHCI
    crate::dev::console::serial_write("[ring0_integration] Step 3: AHCI initialization...\n");
    if crate::dev::ahci::init() {
        crate::dev::console::serial_write("[ring0_integration] AHCI: OK\n");
    } else {
        crate::dev::console::serial_write("[ring0_integration] AHCI: FAILED (no storage available)\n");
    }
    
    crate::dev::console::serial_write("[ring0_integration] === Ring 0 subsystems initialized ===\n\n");
    
    true
}

/// Check if all critical subsystems are ready.
pub fn is_ready() -> bool {
    // APIC MMIO must be initialized
    if crate::irq::apic_mmio::get_apic_virt_base() == 0 {
        return false;
    }
    
    // At least one CPU must be online
    if crate::arch::smp::smp_enabler::online_count() == 0 {
        return false;
    }
    
    true
}

/// Get a status report of all subsystems.
pub fn status_report() {
    crate::dev::console::serial_write("\n=== Ring 0 Subsystem Status ===\n");
    
    // APIC MMIO
    crate::dev::console::serial_write("APIC MMIO: ");
    if crate::irq::apic_mmio::get_apic_virt_base() != 0 {
        crate::dev::console::serial_write("OK (base=0x");
        crate::dev::console::serial_write_u64(crate::irq::apic_mmio::get_apic_virt_base(), 16);
        crate::dev::console::serial_write(")\n");
    } else {
        crate::dev::console::serial_write("NOT INITIALIZED\n");
    }
    
    // SMP
    crate::dev::console::serial_write("SMP: ");
    let online = crate::arch::smp::smp_enabler::online_count();
    let total = crate::arch::smp::smp_enabler::total_count();
    crate::dev::console::serial_write_u64(online as u64, 10);
    crate::dev::console::serial_write("/");
    crate::dev::console::serial_write_u64(total as u64, 10);
    crate::dev::console::serial_write(" cores online\n");
    
    // AHCI
    crate::dev::console::serial_write("AHCI: ");
    if crate::dev::storage::is_ready() {
        crate::dev::console::serial_write("OK (");
        crate::dev::console::serial_write_u64(crate::dev::storage::port_count() as u64, 10);
        crate::dev::console::serial_write(" ports)\n");
    } else {
        crate::dev::console::serial_write("NOT READY\n");
    }
    
    // Ring 3
    crate::dev::console::serial_write("Ring 3: ");
    if crate::arch::syscall::ring3_alive() {
        crate::dev::console::serial_write("ACTIVE\n");
    } else {
        crate::dev::console::serial_write("NOT STARTED\n");
    }
    
    crate::dev::console::serial_write("================================\n\n");
}

/// Transition to Ring 3 with the first user-mode process.
///
/// This is the final step in the boot sequence. It sets up the
/// initial user-mode process and jumps to it.
///
/// # Arguments
///
/// * `entry_point` - Virtual address of the user-mode entry point
/// * `user_stack` - Virtual address of the user-mode stack top
pub fn enter_usermode(entry_point: u64, user_stack: u64) -> ! {
    crate::dev::console::serial_write("[ring0_integration] transitioning to usermode...\n");
    
    // Ensure all subsystems are ready
    if !is_ready() {
        crate::dev::console::serial_write("[ring0_integration] ERROR: not all subsystems ready\n");
        loop {
            core::arch::asm!("hlt");
        }
    }
    
    // Enter Ring 3
    crate::arch::ring3_entry::enter_ring3(entry_point, user_stack);
}
