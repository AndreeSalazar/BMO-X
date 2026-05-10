//! SEC2 Falcon Engine Implementation

use crate::drivers::gpu::fastgpu::falcon::FalconEngine;
use crate::drivers::gpu::fastgpu::hw::mmio::Mmio;
use crate::drivers::gpu::fastgpu::intelligence::mmio_map::*;
use crate::drivers::gpu::fastgpu::intelligence::core_types::RegisterDescriptor;
use crate::evidence_println;

pub struct Sec2Engine<'a> {
    mmio: &'a mut Mmio,
}

impl<'a> Sec2Engine<'a> {
    pub fn new(mmio: &'a mut Mmio) -> Self {
        Self { mmio }
    }

    fn write_reg(&mut self, reg: &RegisterDescriptor, val: u32) {
        evidence_println!("[SEC2] [{:?}] Writing {} (0x{:08X}) <- 0x{:08X} | Source: {}", 
            reg.confidence, reg.name, reg.offset, val, reg.source);
        self.mmio.write32(reg.offset, val);
    }

    fn read_reg(&self, reg: &RegisterDescriptor) -> u32 {
        let val = self.mmio.read32(reg.offset);
        evidence_println!("[SEC2] [{:?}] Read {} (0x{:08X}) -> 0x{:08X} | Source: {}", 
            reg.confidence, reg.name, reg.offset, val, reg.source);
        val
    }

    /// Step 1: Clock ungating (PMC enable SEC2)
    pub fn enable_pmc(&mut self) {
        // PMC SEC2 enable bit is usually bit 13
        let current = self.read_reg(&PMC_ENABLE);
        self.write_reg(&PMC_ENABLE, current | (1 << 13));
    }
}

impl<'a> FalconEngine for Sec2Engine<'a> {
    fn reset(&mut self) -> Result<(), &'static str> {
        // Step 2: SEC2 reset release
        // Write to SEC2_FALCON_ENGINE
        self.write_reg(&SEC2_FALCON_ENGINE, 0x2); // Example reset command
        Ok(())
    }

    fn load_imem(&mut self, data: &[u8]) -> Result<(), &'static str> {
        // Step 3: IMEM Load
        self.write_reg(&SEC2_FALCON_IMEMC, 0x01000000); // Set auto-increment flag (AIF)
        
        // Push words
        let chunks = data.chunks_exact(4);
        for chunk in chunks {
            let word = u32::from_le_bytes(chunk.try_into().unwrap());
            self.mmio.write32(SEC2_FALCON_IMEMD.offset, word); // Direct write to avoid log spam
        }
        evidence_println!("[SEC2] Loaded {} bytes into IMEM", data.len());
        Ok(())
    }

    fn load_dmem(&mut self, data: &[u8]) -> Result<(), &'static str> {
        // Step 4: DMEM Load
        self.write_reg(&SEC2_FALCON_DMEMC, 0x01000000); 
        
        let chunks = data.chunks_exact(4);
        for chunk in chunks {
            let word = u32::from_le_bytes(chunk.try_into().unwrap());
            self.mmio.write32(SEC2_FALCON_DMEMD.offset, word);
        }
        evidence_println!("[SEC2] Loaded {} bytes into DMEM", data.len());
        Ok(())
    }

    fn set_bootvec(&mut self, vec: u32) -> Result<(), &'static str> {
        // Step 5: Set BOOTVEC
        self.write_reg(&SEC2_FALCON_BOOTVEC, vec);
        Ok(())
    }

    fn start_cpu(&mut self) -> Result<(), &'static str> {
        // Step 6: CPUCTL start
        self.write_reg(&SEC2_FALCON_CPUCTL, 0x2); // Start bit
        Ok(())
    }

    fn validate_hs_mode(&self) -> Result<bool, &'static str> {
        // Step 7: Poll HS Mode
        let status = self.read_reg(&SEC2_FALCON_CPUCTL);
        let halted = (status & 0x10) != 0;
        let hs_mode = (status & 0x80) != 0; // typically bit 7 or similar for HS mode indicator
        
        evidence_println!("[SEC2] CPUCTL Halted={}, HSMode={}", halted, hs_mode);
        Ok(hs_mode)
    }

    fn handle_irq(&mut self) -> Result<(), &'static str> {
        let irq = self.read_reg(&SEC2_FALCON_IRQSTAT);
        evidence_println!("[SEC2] IRQ status: 0x{:08X}", irq);
        Ok(())
    }
}
