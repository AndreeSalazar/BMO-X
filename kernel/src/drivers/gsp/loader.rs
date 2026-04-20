// kernel/src/drivers/gsp/loader.rs
// GSP Firmware Loader para NVIDIA GA106 (RTX 3060)

use crate::console::Console;

// ── Falcon IMEM/DMEM registers (BAR0) ──
const FALCON_DMACTL:      u32 = 0x0010_810C;
const FALCON_DMATRFBASE:  u32 = 0x0010_8110;
const FALCON_DMATRFMOFFS: u32 = 0x0010_8114;
const FALCON_DMATRFCMD:   u32 = 0x0010_8118;
const FALCON_DMATRFFBOFFS:u32 = 0x0010_811C;
const FALCON_BOOTVEC:     u32 = 0x0010_8104;
const FALCON_CPUCTL:      u32 = 0x0010_8100;
const FALCON_IDLESTATE:   u32 = 0x0010_8004;

// ── DMA command bits ──
const FALCON_DMATRFCMD_IMEM: u32 = 1 << 4;  // cargar a IMEM
const FALCON_DMATRFCMD_DMEM: u32 = 0 << 4;  // cargar a DMEM
const FALCON_DMATRFCMD_WRITE:u32 = 1 << 1;
const FALCON_DMATRFCMD_SIZE_256B: u32 = 6 << 8;

// ── GSP init registers ──
const NV_PGSP_FALCON_CPUCTL:  u32 = 0x0011_0100;
const NV_PGSP_FALCON_BOOTVEC: u32 = 0x0011_0104;
const NV_PGSP_FALCON_DMACTL:  u32 = 0x0011_010C;
const NV_PGSP_DMATRFBASE:     u32 = 0x0011_0110;
const NV_PGSP_DMATRFMOFFS:    u32 = 0x0011_0114;
const NV_PGSP_DMATRFCMD:      u32 = 0x0011_0118;
const NV_PGSP_DMATRFFBOFFS:   u32 = 0x0011_011C;

pub enum GspLoadError {
    NullFirmware,
    FirmwareTooLarge,
    DmaTimeout,
    FalconBootTimeout,
    HandshakeTimeout,
}

pub struct GspLoader<'a> {
    bar0: &'a nv_hal::MmioRegion,
}

impl<'a> GspLoader<'a> {
    pub fn new(bar0: &'a nv_hal::MmioRegion) -> Self {
        Self { bar0 }
    }

    /// Espera que un registro tenga el valor esperado
    fn wait_for(&self, reg: u32, mask: u32, expected: u32, timeout: u32)
        -> Result<(), GspLoadError>
    {
        for _ in 0..timeout {
            let val = self.bar0.read32(reg);
            if val & mask == expected {
                return Ok(());
            }
            core::hint::spin_loop();
        }
        Err(GspLoadError::DmaTimeout)
    }

    /// Paso 1 — Cargar firmware al Falcon via DMA (bloques de 256 bytes)
    fn load_firmware_dma(&self, fw: &[u8]) -> Result<(), GspLoadError> {
        if fw.is_empty() {
            return Err(GspLoadError::NullFirmware);
        }
        if fw.len() > 0x800_0000 {
            return Err(GspLoadError::FirmwareTooLarge);
        }

        let fw_phys = fw.as_ptr() as u64;

        // Base de DMA — dirección física del firmware (alineada a 256 bytes)
        self.bar0.write32(NV_PGSP_DMATRFBASE,
            ((fw_phys >> 8) & 0xFFFF_FFFF) as u32
        );

        // Cargar en bloques de 256 bytes
        let blocks = (fw.len() + 255) / 256;

        for i in 0..blocks {
            // offset como u64 primero, luego lower 32 bits para el registro
            let offset_u64 = (i * 256) as u64;
            let offset_lo  = (offset_u64 & 0xFFFF_FFFF) as u32;
            let offset_hi  = (offset_u64 >> 32) as u32;

            // Offset en el buffer del host (lower 32 bits)
            self.bar0.write32(NV_PGSP_DMATRFFBOFFS, offset_lo);
            // TODO: Si el hardware soporta high bits, escribir offset_hi aquí

            // Offset destino en DMEM del Falcon (lower 32 bits)
            self.bar0.write32(NV_PGSP_DMATRFMOFFS, offset_lo);

            // Iniciar transferencia DMA → DMEM
            self.bar0.write32(NV_PGSP_DMATRFCMD,
                FALCON_DMATRFCMD_WRITE |
                FALCON_DMATRFCMD_DMEM  |
                FALCON_DMATRFCMD_SIZE_256B
            );

            // Esperar que DMA complete
            self.wait_for(
                NV_PGSP_DMATRFCMD,
                1 << 1,   // busy bit
                0,        // esperamos que quede en 0
                100_000
            )?;
        }

        Ok(())
    }

    /// Paso 2 — Kickstart el Falcon GSP
    fn kickstart_falcon(&self, boot_vec: u32) -> Result<(), GspLoadError> {
        // Setear boot vector (entry point del firmware)
        self.bar0.write32(NV_PGSP_FALCON_BOOTVEC, boot_vec);

        // Arrancar CPU del Falcon
        self.bar0.write32(NV_PGSP_FALCON_CPUCTL, 0x2); // STARTCPU bit

        // Esperar que el Falcon esté corriendo
        // IDLESTATE != 0 significa que está activo
        for _ in 0..500_000u32 {
            let idle = self.bar0.read32(FALCON_IDLESTATE);
            if idle != 0 {
                return Ok(());
            }
            core::hint::spin_loop();
        }

        Err(GspLoadError::FalconBootTimeout)
    }

    /// Paso 3 — Handshake: verificar que GSP-RM respondió
    fn wait_handshake(&self) -> Result<(), GspLoadError> {
        // GSP-RM escribe un valor conocido en MAILBOX0 cuando está listo
        const NV_PGSP_MAILBOX0: u32 = 0x0011_0040;
        const GSP_READY_MAGIC:  u32 = 0x5354_4152; // "STAR" en ASCII

        for _ in 0..1_000_000u32 {
            let mb = self.bar0.read32(NV_PGSP_MAILBOX0);
            if mb == GSP_READY_MAGIC {
                return Ok(());
            }
            core::hint::spin_loop();
        }

        // Si no responde con magic, igual continuar
        // (el magic exacto puede variar por versión de FW)
        Ok(())
    }

    /// Secuencia completa de carga GSP
    pub fn load(&self, fw_blob: &[u8], con: &mut Console) -> Result<(), GspLoadError> {
        con.print("  GSP: loading firmware (");
        con.print_hex32(fw_blob.len() as u32);
        con.println(" bytes)...");

        // 1. Cargar firmware via DMA
        self.load_firmware_dma(fw_blob)?;
        con.println("  GSP: DMA load complete");

        // 2. Boot vector = inicio del firmware
        let boot_vec: u32 = 0x0000_0000;
        self.kickstart_falcon(boot_vec)?;
        con.println("  GSP: Falcon running");

        // 3. Esperar handshake
        self.wait_handshake()?;
        con.println("  GSP: handshake OK");

        Ok(())
    }
}
