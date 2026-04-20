//! GSP Scratch Register Test — Falcon Bootstrap Fix
//!
//! T10 fix: Enable GSP in PMC, reset FALCON, then verify scratch W/R.
//! The scratch register was returning 0 because the GSP (internal ARM)
//! was not active to maintain register state.

use crate::console::Console;

// Registros clave GA106 — BAR0 offsets
const NV_PMC_ENABLE: u32 = 0x0000_0200;
const NV_PMC_ENABLE_2: u32 = 0x0000_0204;  // Ampere específico
const NV_PMC_DEVICE_ENABLE: u32 = 0x0000_0208;
const NV_PMC_GSP_BIT: u32 = 1 << 20;      // bit 20 en ENABLE
const NV_PMC_GSP_BIT_2: u32 = 1 << 7;       // bit 7 en ENABLE_2

const NV_FALCON_CPUCTL: u32 = 0x0010_8100;
const NV_FALCON_CPUCTL_SRESET: u32 = 1 << 0;
const NV_PGSP_SCRATCH_BASE: u32 = 0x0011_0800;
const NV_PGSP_FALCON_MAILBOX0: u32 = 0x0011_0040;

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
    pub fn enable_gsp_pmc(&self, con: &mut Console) -> Result<(), GspScratchError> {
        // Leer estado actual de ambos registros
        let pmc_en = self.bar0.read32(NV_PMC_ENABLE);
        let pmc_en2 = self.bar0.read32(NV_PMC_ENABLE_2);

        // Log para diagnóstico — ver valores reales en pantalla
        con.print("  PMC_ENABLE   = 0x");
        con.print_hex32(pmc_en);
        con.println("");
        con.print("  PMC_ENABLE_2 = 0x");
        con.print_hex32(pmc_en2);
        con.println("");

        // GA106 Ampere requiere habilitar ENABLE_2 primero
        self.bar0.write32(NV_PMC_ENABLE_2, pmc_en2 | NV_PMC_GSP_BIT_2);

        // Delay — dar tiempo al hardware
        for _ in 0..10_000u32 {
            core::hint::spin_loop();
        }

        // Luego ENABLE principal
        self.bar0.write32(NV_PMC_ENABLE, pmc_en | NV_PMC_GSP_BIT);

        // Delay segundo
        for _ in 0..10_000u32 {
            core::hint::spin_loop();
        }

        // Verificar ENABLE_2 — GA106 respeta este
        let verify_2 = self.bar0.read32(NV_PMC_ENABLE_2);
        let verify_1 = self.bar0.read32(NV_PMC_ENABLE);

        con.print("  PMC_ENABLE   after = 0x");
        con.print_hex32(verify_1);
        con.println("");
        con.print("  PMC_ENABLE_2 after = 0x");
        con.print_hex32(verify_2);
        con.println("");

        // Si ninguno de los dos aceptó el bit — fallo real
        if (verify_2 & NV_PMC_GSP_BIT_2 == 0) && (verify_1 & NV_PMC_GSP_BIT == 0) {
            return Err(GspScratchError::PmcEnableFailed);
        }

        Ok(())
    }

    /// Paso 2 — Reset del Falcon GSP
    pub fn reset_falcon(&self, con: &mut Console) -> Result<(), GspScratchError> {
        // Leer estado actual
        let cpuctl = self.bar0.read32(NV_FALCON_CPUCTL);
        con.print("  FALCON_CPUCTL before = 0x");
        con.print_hex32(cpuctl);
        con.println("");

        // Assert reset
        self.bar0.write32(NV_FALCON_CPUCTL, NV_FALCON_CPUCTL_SRESET);

        // Delay post-reset
        for _ in 0..50_000u32 {
            core::hint::spin_loop();
        }

        // Deassert reset
        self.bar0.write32(NV_FALCON_CPUCTL, 0x0);

        // Esperar que salga de reset
        for _ in 0..200_000u32 {
            let ctl = self.bar0.read32(NV_FALCON_CPUCTL);
            if ctl & NV_FALCON_CPUCTL_SRESET == 0 {
                con.print("  FALCON_CPUCTL after  = 0x");
                con.print_hex32(ctl);
                con.println("");
                return Ok(());
            }
            core::hint::spin_loop();
        }

        Err(GspScratchError::FalconResetTimeout)
    }

    /// Paso 3 — Verificar scratch W/R (tu T10)
    pub fn verify_scratch(&self, con: &mut Console) -> Result<(), GspScratchError> {
        let pattern: u32 = 0xFA57_0505;

        // Escribir
        self.bar0.write32(NV_PGSP_SCRATCH_BASE, pattern);

        // Memory barrier — leer registro diferente para forzar flush
        let _ = self.bar0.read32(NV_PGSP_FALCON_MAILBOX0);

        // Delay antes de leer
        for _ in 0..5_000u32 {
            core::hint::spin_loop();
        }

        // Leer de vuelta
        let readback = self.bar0.read32(NV_PGSP_SCRATCH_BASE);

        con.print("  SCRATCH wrote = 0x");
        con.print_hex32(pattern);
        con.println("");
        con.print("  SCRATCH read  = 0x");
        con.print_hex32(readback);
        con.println("");

        if readback != pattern {
            return Err(GspScratchError::ScratchMismatch {
                wrote: pattern,
                read: readback,
            });
        }

        Ok(())
    }

    /// Secuencia completa T10
    pub fn run(&self, con: &mut Console) -> Result<(), GspScratchError> {
        self.enable_gsp_pmc(con)?; // PMC primero
        self.reset_falcon(con)?; // Falcon out of reset
        self.verify_scratch(con)?; // Ahora el scratch debe persistir
        Ok(())
    }

    /// Report result to console
    pub fn report_result(&self, con: &mut Console) {
        con.print("  T10 GSP Scratch W/R  ");
        match self.run(con) {
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
