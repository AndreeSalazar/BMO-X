//! GSP Scratch Register Test — Falcon Bootstrap Fix
//!
//! T10 fix: Enable GSP in PMC, reset FALCON, then verify scratch W/R.
//! The scratch register was returning 0 because the GSP (internal ARM)
//! was not active to maintain register state.

use crate::console::Console;

// Registros clave GA106 — BAR0 offsets
const NV_PMC_ENABLE: u32 = 0x0000_0200;
const NV_PMC_ENABLE_GSP: u32 = 1 << 20;

const NV_FALCON_CPUCTL: u32 = 0x0010_8100;
const NV_FALCON_CPUCTL_SRESET: u32 = 1 << 0;

const NV_FALCON_DMACTL: u32 = 0x0010_810C;
const NV_FALCON_BOOTVEC: u32 = 0x0010_8104;

// GSP scratch registers (BAR0)
const NV_PGSP_FALCON_MAILBOX0: u32 = 0x0011_0040;
const NV_PGSP_FALCON_MAILBOX1: u32 = 0x0011_0044;
const NV_PGSP_SCRATCH_BASE: u32 = 0x0011_0800;

#[derive(Debug)]
pub enum GspScratchError {
    FalconResetTimeout,
    ScratchMismatch { wrote: u32, read: u32 },
    PmcEnableFailed,
}

pub struct GspScratchTest<'a> {
    bar0: &'a nv_hal::MmioRegion,
}

impl<'a> GspScratchTest<'a> {
    pub fn new(bar0: &'a nv_hal::MmioRegion) -> Self {
        Self { bar0 }
    }

    /// Paso 1 — Habilitar GSP en PMC
    pub fn enable_gsp_pmc(&self) -> Result<(), GspScratchError> {
        let current = self.bar0.read32(NV_PMC_ENABLE);
        self.bar0.write32(NV_PMC_ENABLE, current | NV_PMC_ENABLE_GSP);

        // Verificar que el bit quedó seteado
        let verify = self.bar0.read32(NV_PMC_ENABLE);
        if verify & NV_PMC_ENABLE_GSP == 0 {
            return Err(GspScratchError::PmcEnableFailed);
        }
        Ok(())
    }

    /// Paso 2 — Reset del Falcon GSP
    pub fn reset_falcon(&self) -> Result<(), GspScratchError> {
        // Assert reset
        self.bar0.write32(NV_FALCON_CPUCTL, NV_FALCON_CPUCTL_SRESET);

        // Esperar que salga de reset (~100 ciclos PIT)
        for _ in 0..100_000u32 {
            let ctl = self.bar0.read32(NV_FALCON_CPUCTL);
            if ctl & NV_FALCON_CPUCTL_SRESET == 0 {
                return Ok(());
            }
            core::hint::spin_loop();
        }
        Err(GspScratchError::FalconResetTimeout)
    }

    /// Paso 3 — Verificar scratch W/R (tu T10)
    pub fn verify_scratch(&self) -> Result<(), GspScratchError> {
        let pattern: u32 = 0xFA57_0505; // magic number

        self.bar0.write32(NV_PGSP_SCRATCH_BASE, pattern);

        // Barrier — asegurar que el write llegó al hardware
        let _ = self.bar0.read32(NV_PGSP_FALCON_MAILBOX0);

        let readback = self.bar0.read32(NV_PGSP_SCRATCH_BASE);

        if readback != pattern {
            return Err(GspScratchError::ScratchMismatch {
                wrote: pattern,
                read: readback,
            });
        }
        Ok(())
    }

    /// Secuencia completa T10
    pub fn run(&self) -> Result<(), GspScratchError> {
        self.enable_gsp_pmc()?; // PMC primero
        self.reset_falcon()?; // Falcon out of reset
        self.verify_scratch()?; // Ahora el scratch debe persistir
        Ok(())
    }

    /// Report result to console
    pub fn report_result(&self, con: &mut Console) {
        con.print("  T10 GSP Scratch W/R  ");
        match self.run() {
            Ok(()) => {
                let val = self.bar0.read32(NV_PGSP_SCRATCH_BASE);
                con.print_colored("[PASS]", 0xFF00FF00);
                con.print(" write=readback=0x");
                con.print_hex32(val);
                con.println("");
            }
            Err(GspScratchError::FalconResetTimeout) => {
                con.print_colored("[FAIL]", 0xFFFF0000);
                con.println(" Falcon reset timeout");
            }
            Err(GspScratchError::ScratchMismatch { wrote, read }) => {
                con.print_colored("[FAIL]", 0xFFFF0000);
                con.print(" wrote=0x");
                con.print_hex32(wrote);
                con.print(" read=0x");
                con.print_hex32(read);
                con.println("");
            }
            Err(GspScratchError::PmcEnableFailed) => {
                con.print_colored("[FAIL]", 0xFFFF0000);
                con.println(" PMC GSP bit rejected");
            }
        }
    }
}
