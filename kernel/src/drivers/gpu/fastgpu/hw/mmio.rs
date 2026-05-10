//! MMIO Access Layer
//! Safe abstractions over volatile BAR0 reads/writes, integrated with evidence tracing.

use core::ptr::{read_volatile, write_volatile};
use crate::drivers::gpu::fastgpu::runtime::GpuRuntimeMode;
use crate::evidence_println;

/// MMIO accessor for the GPU BAR0 space.
pub struct Mmio {
    bar0_base: u64,
    mode: GpuRuntimeMode,
}

impl Mmio {
    /// Creates a new MMIO accessor.
    pub const unsafe fn new(bar0_base: u64, mode: GpuRuntimeMode) -> Self {
        Self { bar0_base, mode }
    }

    /// Returns the BAR0 base address.
    pub fn base(&self) -> u64 {
        self.bar0_base
    }

    /// Returns the current runtime mode.
    pub fn mode(&self) -> GpuRuntimeMode {
        self.mode
    }

    /// Reads a 32-bit register at the given offset.
    /// In DryRun/ObserveOnly, does NOT touch hardware — returns 0.
    #[inline(always)]
    pub fn read32(&self, offset: u32) -> u32 {
        match self.mode {
            GpuRuntimeMode::Active => {
                let val = unsafe { read_volatile((self.bar0_base + offset as u64) as *const u32) };
                evidence_println!("[MMIO] READ  0x{:08X} -> 0x{:08X}", offset, val);
                val
            }
            _ => {
                evidence_println!("[MMIO-DRYRUN] READ  0x{:08X} -> 0x00000000 (simulated)", offset);
                0
            }
        }
    }

    /// Writes a 32-bit value to the given offset.
    /// In DryRun/ObserveOnly, does NOT touch hardware.
    #[inline(always)]
    pub fn write32(&mut self, offset: u32, val: u32) {
        match self.mode {
            GpuRuntimeMode::Active => {
                evidence_println!("[MMIO] WRITE 0x{:08X} <- 0x{:08X}", offset, val);
                unsafe { write_volatile((self.bar0_base + offset as u64) as *mut u32, val) };
            }
            _ => {
                evidence_println!("[MMIO-DRYRUN] WRITE 0x{:08X} <- 0x{:08X} (blocked)", offset, val);
            }
        }
    }

    /// Polls a register until (val & mask) == expected or timeout.
    /// In DryRun, always succeeds immediately.
    pub fn poll32(&self, offset: u32, mask: u32, expected: u32, timeout_iters: usize) -> Result<(), &'static str> {
        if self.mode != GpuRuntimeMode::Active {
            evidence_println!("[MMIO-DRYRUN] POLL  0x{:08X} mask=0x{:08X} expected=0x{:08X} (simulated OK)", offset, mask, expected);
            return Ok(());
        }

        for i in 0..timeout_iters {
            let val = unsafe { read_volatile((self.bar0_base + offset as u64) as *const u32) };
            if (val & mask) == expected {
                evidence_println!("[MMIO] POLL  0x{:08X} OK at iter {}", offset, i);
                return Ok(());
            }
            for _ in 0..100 {
                unsafe { core::arch::asm!("pause") };
            }
        }
        evidence_println!("[MMIO] POLL  0x{:08X} TIMEOUT after {} iters", offset, timeout_iters);
        Err("MMIO Poll Timeout")
    }
}
