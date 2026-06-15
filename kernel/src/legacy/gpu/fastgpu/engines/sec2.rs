//! SEC2 Falcon Engine Implementation

use crate::drivers::gpu::fastgpu::falcon::FalconEngine;
use crate::drivers::gpu::fastgpu::hw::mmio::Mmio;
use crate::drivers::gpu::fastgpu::intelligence::mmio_map::registers::*;
use crate::evidence_println;

pub struct Sec2Engine<'a> {
    mmio: &'a mut Mmio,
}

impl<'a> Sec2Engine<'a> {
    pub fn new(mmio: &'a mut Mmio) -> Self {
        Self { mmio }
    }

    fn write_reg(&mut self, offset: u32, val: u32) {
        evidence_println!("[SEC2] Writing (0x{:08X}) <- 0x{:08X}", offset, val);
        self.mmio.write32(offset, val);
    }

    fn read_reg(&self, offset: u32) -> u32 {
        let val = self.mmio.read32(offset);
        evidence_println!("[SEC2] Read (0x{:08X}) -> 0x{:08X}", offset, val);
        val
    }

    /// Step 1: Clock ungating (PMC enable SEC2)
    pub fn enable_pmc(&mut self) {
        // PMC SEC2 enable bit is usually bit 13
        let current = self.read_reg(PMC_ENABLE);
        self.write_reg(PMC_ENABLE, current | (1 << 13));
    }
}

impl<'a> FalconEngine for Sec2Engine<'a> {
    fn reset(&mut self) -> Result<(), &'static str> {
        // Step 2: SEC2 reset release
        // Write to SEC2 Engine Reset offset (approx 0x200 or engine specific)
        // For now, using a placeholder if ENGINE is not in mmio_map
        // self.write_reg(SEC2_FALCON_ENGINE, 0x2); 
        Ok(())
    }

    fn load_imem(&mut self, data: &[u8]) -> Result<(), &'static str> {
        // Step 3: IMEM Load
        self.write_reg(IMEMC, 0x01000000); // Set auto-increment flag (AIF)
        
        // Push words
        let chunks = data.chunks_exact(4);
        for chunk in chunks {
            let word = u32::from_le_bytes(chunk.try_into().unwrap());
            self.mmio.write32(IMEMC + 4, word); // IMEMD is usually IMEMC + 4
        }
        evidence_println!("[SEC2] Loaded {} bytes into IMEM", data.len());
        Ok(())
    }

    fn load_dmem(&mut self, data: &[u8]) -> Result<(), &'static str> {
        // Step 4: DMEM Load
        self.write_reg(DMEMC, 0x01000000); 
        
        let chunks = data.chunks_exact(4);
        for chunk in chunks {
            let word = u32::from_le_bytes(chunk.try_into().unwrap());
            self.mmio.write32(DMEMC + 4, word); // DMEMD is usually DMEMC + 4
        }
        evidence_println!("[SEC2] Loaded {} bytes into DMEM", data.len());
        Ok(())
    }

    fn set_bootvec(&mut self, vec: u32) -> Result<(), &'static str> {
        // Step 5: Set BOOTVEC
        self.write_reg(BOOTVEC, vec);
        Ok(())
    }

    fn start_cpu(&mut self) -> Result<(), &'static str> {
        // Step 6: CPUCTL start
        self.write_reg(CPUCTL, 0x2); // Start bit
        Ok(())
    }

    fn validate_hs_mode(&self) -> Result<bool, &'static str> {
        // Step 7: Poll HS Mode
        let status = self.read_reg(CPUCTL);
        let halted = (status & 0x10) != 0;
        let hs_mode = (status & 0x80) != 0; // typically bit 7 or similar for HS mode indicator
        
        evidence_println!("[SEC2] CPUCTL Halted={}, HSMode={}", halted, hs_mode);
        Ok(hs_mode)
    }

    fn handle_irq(&mut self) -> Result<(), &'static str> {
        let irq = self.read_reg(IRQSTAT);
        evidence_println!("[SEC2] IRQ status: 0x{:08X}", irq);
        Ok(())
    }
}
