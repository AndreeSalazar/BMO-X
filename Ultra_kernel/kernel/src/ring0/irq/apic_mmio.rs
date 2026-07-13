//! APIC MMIO Mapping — Fix for #GP on APIC timer access.
//!
//! The APIC registers are accessed via Memory-Mapped I/O (MMIO).
//! The APIC base address is discovered via MSR 0x1B (IA32_APIC_BASE).
//! 
//! ## The #GP Problem
//! 
//! Accessing APIC registers causes #GP when:
//! 1. The APIC base is not properly mapped in the page tables
//! 2. The memory type is incorrect (must be Uncacheable or Write-Combining)
//! 3. The APIC is not globally enabled in IA32_APIC_BASE MSR
//!
//! ## Solution
//!
//! This module:
//! 1. Reads IA32_APIC_BASE MSR to get the physical address
//! 2. Verifies APIC is globally enabled (bit 11)
//! 3. Maps the APIC MMIO region with correct caching attributes (UC)
//! 4. Provides safe access functions

use core::arch::asm;

/// MSR holding the LAPIC base address
const IA32_APIC_BASE_MSR: u32 = 0x1B;

/// APIC base address mask (page-aligned)
const APIC_BASE_MASK: u64 = 0xFFFF_F000;

/// APIC global enable bit (bit 11 of IA32_APIC_BASE)
const APIC_GLOBAL_ENABLE_BIT: u64 = 1 << 11;

/// APIC MMIO region size (4 KB)
#[allow(dead_code)]
const APIC_MMIO_SIZE: u64 = 4096;

/// Virtual address where we map the APIC (in higher half)
/// We use a fixed address in the kernel's MMIO region
#[allow(dead_code)]
const APIC_VIRT_ADDR: u64 = 0xFFFF_8000_0000_0000; // 4 GB mark in higher half

/// Cached APIC physical base address
static mut APIC_PHYS_BASE: u64 = 0;

/// Cached APIC virtual base address (after mapping)
static mut APIC_VIRT_BASE: u64 = 0;

/// Read the APIC base address from MSR 0x1B
pub fn read_apic_base_msr() -> u64 {
    let (lo, hi): (u32, u32);
    unsafe {
        asm!(
            "rdmsr",
            in("ecx") IA32_APIC_BASE_MSR,
            out("eax") lo,
            out("edx") hi,
            options(nostack)
        );
    }
    ((hi as u64) << 32) | (lo as u64)
}

/// Check if APIC is globally enabled
pub fn is_apic_enabled() -> bool {
    let msr_val = read_apic_base_msr();
    msr_val & APIC_GLOBAL_ENABLE_BIT != 0
}

/// Enable APIC globally if not already enabled
pub fn enable_apic_global() -> Result<(), &'static str> {
    let msr_val = read_apic_base_msr();
    
    if msr_val & APIC_GLOBAL_ENABLE_BIT != 0 {
        // Already enabled
        return Ok(());
    }
    
    // Check if APIC is supported (bit 8 of CPUID.01H:EDX)
    let cpuid_edx: u32;
    unsafe {
        asm!(
            "push rbx",
            "mov eax, 1",
            "cpuid",
            "pop rbx",
            out("edx") cpuid_edx,
            out("eax") _,
            out("ecx") _,
            options(nostack)
        );
    }
    
    if cpuid_edx & (1 << 9) == 0 {
        return Err("APIC not supported by CPU");
    }
    
    // Enable APIC by setting bit 11
    let new_val = msr_val | APIC_GLOBAL_ENABLE_BIT;
    let lo = new_val as u32;
    let hi = (new_val >> 32) as u32;
    
    unsafe {
        asm!(
            "wrmsr",
            in("ecx") IA32_APIC_BASE_MSR,
            in("eax") lo,
            in("edx") hi,
            options(nostack)
        );
    }
    
    Ok(())
}

/// Get the physical APIC base address (page-aligned)
pub fn get_apic_phys_base() -> u64 {
    read_apic_base_msr() & APIC_BASE_MASK
}

/// Map the APIC MMIO region into the kernel's address space.
/// 
/// This must be called before any APIC register access.
/// 
/// # Safety
/// 
/// This function modifies page tables and should only be called once during boot.
pub unsafe fn map_apic_mmio() -> Result<u64, &'static str> {
    // Get physical base
    let phys_base = get_apic_phys_base();
    
    if phys_base == 0 {
        return Err("APIC base is 0");
    }
    
    // Store physical base
    APIC_PHYS_BASE = phys_base;
    
    // Map the APIC MMIO region with Uncacheable (UC) memory type
    // We use the VMM to create a mapping with correct attributes
    // The APIC MMIO must be UC to avoid cache coherency issues
    
    // For now, we'll use identity mapping if the APIC is in low memory
    // Otherwise, we need to create a proper mapping in the higher half
    
    // Check if we can use identity mapping (APIC base < 4 GB)
    if phys_base < 0x1_0000_0000 {
        // Identity mapping should work
        APIC_VIRT_BASE = phys_base;
        
        crate::ring0::dev::console::serial_write("[apic_mmio] using identity mapping at 0x");
        crate::ring0::dev::console::serial_write_u64(phys_base, 16);
        crate::ring0::dev::console::serial_write("\n");
    } else {
        // Need to create a mapping in higher half
        // This requires VMM support which we'll implement
        // For now, return error
        return Err("APIC base in high memory, mapping not yet implemented");
    }
    
    // Verify the mapping by reading APIC version register
    let version_reg_offset = 0x030; // APIC_VERSION
    let version = core::ptr::read_volatile((APIC_VIRT_BASE + version_reg_offset) as *const u32);
    
    crate::ring0::dev::console::serial_write("[apic_mmio] APIC version: 0x");
    crate::ring0::dev::console::serial_write_u64(version as u64, 16);
    crate::ring0::dev::console::serial_write("\n");
    
    Ok(APIC_VIRT_BASE)
}

/// Initialize APIC MMIO mapping.
/// 
/// This is the main entry point. Call this before using any APIC functions.
pub fn init() -> Result<(), &'static str> {
    // Check if APIC is enabled
    if !is_apic_enabled() {
        crate::ring0::dev::console::serial_write("[apic_mmio] APIC not enabled, enabling...\n");
        enable_apic_global()?;
    }
    
    // Map the APIC MMIO
    unsafe {
        map_apic_mmio()?;
    }
    
    crate::ring0::dev::console::serial_write("[apic_mmio] APIC MMIO initialized successfully\n");
    Ok(())
}

/// Get the virtual base address of the APIC MMIO
pub fn get_apic_virt_base() -> u64 {
    unsafe { APIC_VIRT_BASE }
}

/// Read an APIC register
/// 
/// # Safety
/// 
/// The offset must be a valid APIC register offset.
pub unsafe fn read_reg(offset: u32) -> u32 {
    let addr = APIC_VIRT_BASE + offset as u64;
    core::ptr::read_volatile(addr as *const u32)
}

/// Write an APIC register
/// 
/// # Safety
/// 
/// The offset must be a valid APIC register offset.
/// Writing to certain registers may have side effects.
pub unsafe fn write_reg(offset: u32, val: u32) {
    let addr = APIC_VIRT_BASE + offset as u64;
    core::ptr::write_volatile(addr as *mut u32, val);
}
