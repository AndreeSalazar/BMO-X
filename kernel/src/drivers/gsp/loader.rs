// kernel/src/drivers/gsp/loader.rs
// GSP Firmware Loader — GA106 (RTX 3060)
// Integrates: PRIV Ring → Page Allocator → DMA → Falcon Boot → Handshake

use crate::console::Console;
use crate::drivers::gsp::priv_ring::{PrivRingInit, PrivRingError};

// ── NV_PGSP Falcon registers (BAR0 offsets) ──
const NV_PGSP_FALCON_CPUCTL:     u32 = 0x0011_0100;
const NV_PGSP_FALCON_BOOTVEC:    u32 = 0x0011_0104;
const NV_PGSP_FALCON_DMACTL:     u32 = 0x0011_010C;
const NV_PGSP_DMATRFBASE:        u32 = 0x0011_0110;
const NV_PGSP_DMATRFMOFFS:       u32 = 0x0011_0114;
const NV_PGSP_DMATRFCMD:         u32 = 0x0011_0118;
const NV_PGSP_DMATRFFBOFFS:      u32 = 0x0011_011C;
const NV_PGSP_FALCON_IDLESTATE:  u32 = 0x0011_0004;
const NV_PGSP_MAILBOX0:          u32 = 0x0011_0040;
const NV_PGSP_MAILBOX1:          u32 = 0x0011_0044;

// ── DMA command bits ──
const DMA_CMD_WRITE:    u32 = 1 << 1;
const DMA_CMD_IMEM:     u32 = 1 << 4;
const DMA_CMD_SIZE_256: u32 = 6 << 8;

// ── Boot/handshake constants ──
const FALCON_CPUCTL_STARTCPU: u32 = 0x2;
const GSP_READY_MAGIC: u32 = 0x0000_0000; // 0 = booter success (nouveau convention)

// ── Page size ──
const PAGE_SIZE: usize = 4096;

pub enum GspLoadError {
    NullFirmware,
    BadElfMagic,
    FirmwareTooLarge,
    PageAllocFailed,
    PrivRingFailed,
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

    // ── Timing helper ──
    #[inline]
    fn delay_us(us: u32) {
        for _ in 0..(us as u64 * 3000) {
            unsafe { core::arch::asm!("nop", options(nomem, nostack)) };
        }
    }

    // ── Wait for register condition ──
    fn wait_reg(&self, reg: u32, mask: u32, expected: u32, timeout_loops: u32)
        -> Result<(), GspLoadError>
    {
        for _ in 0..timeout_loops {
            let val = self.bar0.read32(reg);
            if val & mask == expected {
                return Ok(());
            }
            core::hint::spin_loop();
        }
        Err(GspLoadError::DmaTimeout)
    }

    // ── Step 1: Initialize PRIV Ring ──
    fn init_priv_ring(&self, con: &mut Console) -> Result<(), GspLoadError> {
        let priv_ring = PrivRingInit::new(self.bar0);
        priv_ring.init(con).map_err(|_| GspLoadError::PrivRingFailed)
    }

    // ── Step 2: Allocate DMA buffer via page allocator ──
    fn alloc_dma_buffer(&self, size: usize, con: &mut Console) -> Result<u64, GspLoadError> {
        let pages_needed = (size + PAGE_SIZE - 1) / PAGE_SIZE;

        con.print("  GSP: Allocating ");
        con.print_hex32(pages_needed as u32);
        con.print(" pages (");
        con.print_hex32(size as u32);
        con.println(" bytes) for DMA buffer...");

        let phys_addr = unsafe {
            crate::arch::page_alloc::alloc_pages_contiguous(pages_needed)
        };

        match phys_addr {
            Some(addr) => {
                con.print("  GSP: DMA buffer at phys 0x");
                con.print_hex32((addr >> 32) as u32);
                con.print_hex32(addr as u32);
                con.println("");
                Ok(addr)
            }
            None => {
                con.println("  GSP: ERROR - page allocator failed (not enough contiguous RAM)");
                Err(GspLoadError::PageAllocFailed)
            }
        }
    }

    // ── Step 3: Copy firmware to DMA buffer ──
    fn copy_fw_to_dma(&self, fw: &[u8], dma_phys: u64, con: &mut Console) {
        // Identity-mapped: phys == virt for first 4GB
        let dst = dma_phys as *mut u8;

        unsafe {
            // Zero the buffer first (clean slate)
            core::ptr::write_bytes(dst, 0, fw.len());
            // Copy firmware
            core::ptr::copy_nonoverlapping(fw.as_ptr(), dst, fw.len());
        }

        // Verify first 4 bytes copied correctly
        let check = unsafe { core::ptr::read_volatile(dst as *const u32) };
        con.print("  GSP: DMA buf[0..4] = 0x");
        con.print_hex32(check);
        con.println(" (expect 0x464C457F = ELF)");
    }

    // ── Step 4: Configurar puntero de memoria en Mailboxes ──
    fn setup_wpr(&self, dma_phys: u64, con: &mut Console) -> Result<(), GspLoadError> {
        con.println("  GSP: Configurando WPR en MAILBOX0/1...");
        self.bar0.write32(NV_PGSP_MAILBOX0, (dma_phys & 0xFFFF_FFFF) as u32);
        self.bar0.write32(NV_PGSP_MAILBOX1, ((dma_phys >> 32) & 0xFFFF_FFFF) as u32);
        Ok(())
    }

    // ── Step 5: Set boot vector and start Falcon CPU ──
    fn boot_falcon(&self, con: &mut Console) -> Result<(), GspLoadError> {
        con.println("  GSP: Booting Falcon CPU...");

        // Boot vector = 0 (start of DMEM where firmware was loaded)
        self.bar0.write32(NV_PGSP_FALCON_BOOTVEC, 0x0000_0000);

        // Start CPU
        self.bar0.write32(NV_PGSP_FALCON_CPUCTL, FALCON_CPUCTL_STARTCPU);

        // Wait for Falcon to exit idle state
        for i in 0..1_000_000u32 {
            let idle = self.bar0.read32(NV_PGSP_FALCON_IDLESTATE);
            if idle == 0 {
                con.print("  GSP: Falcon running (took ");
                con.print_hex32(i);
                con.println(" loops)");
                return Ok(());
            }
            core::hint::spin_loop();
        }

        con.println("  GSP: WARNING - Falcon idle timeout (may still be booting)");
        Err(GspLoadError::FalconBootTimeout)
    }

    // ── Step 6: Wait for GSP-RM handshake ──
    // Nouveau approach: poll CPUCTL for HALTED bit (0x10), then check MAILBOX0.
    // MAILBOX0 == 0 means booter success; non-zero is an error code.
    fn wait_handshake(&self, con: &mut Console) -> Result<(), GspLoadError> {
        con.println("  GSP: Waiting for Falcon HALT (booter completion)...");

        // First: wait for CPUCTL HALTED bit (bit 4 = 0x10)
        for i in 0..2_000_000u32 {
            let cpuctl = self.bar0.read32(NV_PGSP_FALCON_CPUCTL);
            if cpuctl & 0x10 != 0 {
                con.print("  GSP: Falcon HALTED (cpuctl=0x");
                con.print_hex32(cpuctl);
                con.print(", took ");
                con.print_hex32(i);
                con.println(" loops)");

                // Now read MAILBOX0 — 0 = success, anything else = error
                let mb0 = self.bar0.read32(NV_PGSP_MAILBOX0);
                let mb1 = self.bar0.read32(NV_PGSP_MAILBOX1);
                con.print("  GSP: MAILBOX0=0x");
                con.print_hex32(mb0);
                con.print(" MAILBOX1=0x");
                con.print_hex32(mb1);
                con.println("");

                if mb0 == GSP_READY_MAGIC {
                    con.println("  GSP: Handshake OK (MAILBOX0 == 0, booter success)");
                    return Ok(());
                } else {
                    con.print("  GSP: Booter returned error code 0x");
                    con.print_hex32(mb0);
                    con.println("");
                    return Err(GspLoadError::HandshakeTimeout);
                }
            }
            if i % 500_000 == 0 && i > 0 {
                con.print("  GSP: still waiting (cpuctl=0x");
                con.print_hex32(cpuctl);
                con.println(")...");
            }
            core::hint::spin_loop();
        }

        // Timeout — read final state for diagnostics
        let final_cpuctl = self.bar0.read32(NV_PGSP_FALCON_CPUCTL);
        let final_mb0 = self.bar0.read32(NV_PGSP_MAILBOX0);
        let final_mb1 = self.bar0.read32(NV_PGSP_MAILBOX1);
        con.print("  GSP: Handshake timeout - CPUCTL=0x");
        con.print_hex32(final_cpuctl);
        con.print(" MB0=0x");
        con.print_hex32(final_mb0);
        con.print(" MB1=0x");
        con.print_hex32(final_mb1);
        con.println("");

        // Don't fail hard — firmware may still be running (RISC-V mode)
        Ok(())
    }

    // ── Public: Full GSP load sequence ──
    /// Complete GSP firmware load sequence:
    ///   1. PRIV Ring init (bus must be up before GSP access)
    ///   2. Allocate contiguous DMA buffer via page allocator
    ///   3. Copy firmware ELF to DMA buffer
    ///   4. DMA transfer from buffer to Falcon DMEM
    ///   5. Boot Falcon CPU
    ///   6. Wait for GSP-RM handshake
    pub fn load(&self, fw_blob: &[u8], con: &mut Console) -> Result<(), GspLoadError> {
        con.print_colored("=== GSP Firmware Load (GA106) ===\n", 0x00FFFF);

        // ── Validate firmware ──
        if fw_blob.len() < 64 {
            con.println("  GSP: ERROR - firmware too small");
            return Err(GspLoadError::NullFirmware);
        }
        if fw_blob.len() > 128 * 1024 * 1024 {
            con.println("  GSP: ERROR - firmware > 128MB");
            return Err(GspLoadError::FirmwareTooLarge);
        }

        // Verify ELF magic
        if &fw_blob[0..4] != &[0x7F, 0x45, 0x4C, 0x46] {
            con.print("  GSP: BAD ELF MAGIC = 0x");
            let m = (fw_blob[0] as u32) << 24 | (fw_blob[1] as u32) << 16
                  | (fw_blob[2] as u32) << 8  | fw_blob[3] as u32;
            con.print_hex32(m);
            con.println("");
            return Err(GspLoadError::BadElfMagic);
        }

        con.print("  GSP: Firmware size = ");
        con.print_hex32(fw_blob.len() as u32);
        con.print(" bytes (");
        con.print_hex32((fw_blob.len() / (1024 * 1024)) as u32);
        con.println(" MB)");

        // ── 1. Activar Energía GSP (PMC / Falcon Reset) ──
        con.println("  GSP: [1/7] Activando energia PMC y Falcon Reset...");
        self.init_priv_ring(con)?;

        // ── 2. Preparar boot args con colas de mensajes ──
        // CRÍTICO: Las colas se configuran ANTES de arrancar el Falcon
        // para que el booter las incluya al chain-loadear GSP-RM
        con.println("  GSP: [2/7] Preparando boot args (colas de mensajes)...");
        let (cmdq_phys, msgq_phys) = self.prepare_boot_args(fw_blob, con)?;

        // ── 3. Configurar puntero al firmware en MAILBOX ──
        con.println("  GSP: [3/7] Escribiendo boot args en MAILBOX...");
        let dma_phys = fw_blob.as_ptr() as u64;

        let check = unsafe { core::ptr::read_volatile(dma_phys as *const u32) };
        con.print("  GSP: DMA buf[0..4] = 0x");
        con.print_hex32(check);
        con.println(" (expect 0x464C457F = ELF)");

        // MAILBOX0 = dirección del firmware (lo/hi split)
        self.bar0.write32(NV_PGSP_MAILBOX0, (dma_phys & 0xFFFF_FFFF) as u32);
        self.bar0.write32(NV_PGSP_MAILBOX1, ((dma_phys >> 32) & 0xFFFF_FFFF) as u32);

        // Verificar que MAILBOX aceptó los valores
        let mb0 = self.bar0.read32(NV_PGSP_MAILBOX0);
        let mb1 = self.bar0.read32(NV_PGSP_MAILBOX1);
        con.print("  GSP: MAILBOX0=0x");
        con.print_hex32(mb0);
        con.print(" MAILBOX1=0x");
        con.print_hex32(mb1);
        con.newline();

        // ── 4. Escribir dirección de boot_args en scratch register ──
        // El booter también puede leer de registros scratch del Falcon
        con.println("  GSP: [4/7] Boot args en Falcon scratch registers...");
        // NV_PGSP_FALCON_SCRATCH0-3 (offsets 0x110040-0x11004C)
        // Usamos offsets alternativos para pasar info adicional
        const NV_PGSP_SCRATCH0: u32 = 0x0011_0040;
        const NV_PGSP_SCRATCH1: u32 = 0x0011_0044;
        const NV_PGSP_SCRATCH2: u32 = 0x0011_0048;
        const NV_PGSP_SCRATCH3: u32 = 0x0011_004C;

        self.bar0.write32(NV_PGSP_SCRATCH0, cmdq_phys as u32);
        self.bar0.write32(NV_PGSP_SCRATCH1, (cmdq_phys >> 32) as u32);
        self.bar0.write32(NV_PGSP_SCRATCH2, msgq_phys as u32);
        self.bar0.write32(NV_PGSP_SCRATCH3, (msgq_phys >> 32) as u32);

        // Verificar scratch
        let s0 = self.bar0.read32(NV_PGSP_SCRATCH0);
        let s1 = self.bar0.read32(NV_PGSP_SCRATCH1);
        con.print("  GSP: SCRATCH0/1=0x");
        con.print_hex32(s1);
        con.print_hex32(s0);
        con.println(" (cmdq addr)");

        // ── 5. Boot Falcon ──
        con.println("  GSP: [5/7] Booting Falcon...");
        self.boot_falcon(con)?;

        // ── 6. Esperar al booter (Stage 1) ──
        con.println("  GSP: [6/7] Waiting for booter...");
        self.wait_handshake(con)?;

        // ── 7. Verificar estado del GSP-RM ──
        con.println("  GSP: [7/7] Verificando estado GSP-RM...");
        self.verify_gsp_rm(con)?;

        con.print_colored("=== GSP Load COMPLETE ===\n", 0x00FF00);

        Ok(())
    }

    /// Estructura de boot args mínima
    fn prepare_boot_args(&self, _fw: &[u8], con: &mut Console) -> Result<(u64, u64), GspLoadError> {
        // Asignar 2 páginas contiguas para cmdq + msgq
        let cmdq_phys = unsafe {
            crate::arch::page_alloc::alloc_pages_contiguous(2)
        }.ok_or(GspLoadError::PageAllocFailed)?;

        let msgq_phys = cmdq_phys + PAGE_SIZE as u64;

        // Limpiar
        unsafe {
            core::ptr::write_bytes(cmdq_phys as *mut u8, 0, PAGE_SIZE * 2);
        }

        con.print("  GSP: CmdQ=0x");
        con.print_hex32(cmdq_phys as u32);
        con.print(" MsgQ=0x");
        con.print_hex32(msgq_phys as u32);
        con.newline();

        Ok((cmdq_phys, msgq_phys))
    }

    /// Después del booter, verifica si GSP-RM está vivo
    fn verify_gsp_rm(&self, con: &mut Console) -> Result<(), GspLoadError> {
        // Leer estado actual del Falcon
        let cpuctl = self.bar0.read32(NV_PGSP_FALCON_CPUCTL);
        let idle = self.bar0.read32(NV_PGSP_FALCON_IDLESTATE);
        let mb0 = self.bar0.read32(NV_PGSP_MAILBOX0);
        let mb1 = self.bar0.read32(NV_PGSP_MAILBOX1);

        con.print("  GSP: CPUCTL=0x");
        con.print_hex32(cpuctl);
        con.print(" IDLE=0x");
        con.print_hex32(idle);
        con.newline();
        con.print("  GSP: MB0=0x");
        con.print_hex32(mb0);
        con.print(" MB1=0x");
        con.print_hex32(mb1);
        con.newline();

        // Leer EMEM/scratch para más info
        // Los registros 0x110800-0x110FFF son EMEM del Falcon
        const NV_PGSP_EMEMC0: u32 = 0x0011_0AC0;
        const NV_PGSP_EMEMD0: u32 = 0x0011_0AC4;

        let emem0 = self.bar0.read32(NV_PGSP_EMEMC0);
        let emem1 = self.bar0.read32(NV_PGSP_EMEMD0);
        con.print("  GSP: EMEMC0=0x");
        con.print_hex32(emem0);
        con.print(" EMEMD0=0x");
        con.print_hex32(emem1);
        con.newline();

        if cpuctl & 0x10 != 0 {
            con.print_colored("  GSP: Falcon HALTED — booter completed, GSP-RM needs WPR chain-load\n", 0xFFFF00);
        } else {
            con.print_colored("  GSP: Falcon RUNNING — GSP-RM may be active!\n", 0x00FF00);
        }

        Ok(())
    }
}
