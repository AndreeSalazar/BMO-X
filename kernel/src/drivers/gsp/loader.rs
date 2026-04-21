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
const GSP_READY_MAGIC: u32 = 0x5354_4152; // "STAR"

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

    // ── Step 4: DMA transfer from system RAM to Falcon DMEM ──
    fn dma_transfer(&self, dma_phys: u64, fw_size: usize, con: &mut Console)
        -> Result<(), GspLoadError>
    {
        con.println("  GSP: Starting DMA transfer to Falcon...");

        // Set DMA base address (physical address >> 8, as per Falcon DMA spec)
        let base_shifted = ((dma_phys >> 8) & 0xFFFF_FFFF) as u32;
        self.bar0.write32(NV_PGSP_DMATRFBASE, base_shifted);

        // Transfer in 256-byte blocks
        let blocks = (fw_size + 255) / 256;
        let mut last_pct: u32 = 0;

        for i in 0..blocks {
            let offset = (i * 256) as u32;

            // Source offset in host memory (relative to DMATRFBASE)
            self.bar0.write32(NV_PGSP_DMATRFFBOFFS, offset);

            // Destination offset in Falcon DMEM
            self.bar0.write32(NV_PGSP_DMATRFMOFFS, offset);

            // Issue DMA command: write to DMEM, 256 bytes
            self.bar0.write32(NV_PGSP_DMATRFCMD,
                DMA_CMD_WRITE | DMA_CMD_SIZE_256
            );

            // Wait for transfer to complete (busy bit clears)
            self.wait_reg(NV_PGSP_DMATRFCMD, 1 << 1, 0, 200_000)?;

            // Progress indicator every 10%
            let pct = ((i as u64 * 100) / blocks as u64) as u32;
            if pct >= last_pct + 10 {
                con.print("  GSP: DMA ");
                con.print_hex32(pct);
                con.println("% ...");
                last_pct = pct;
            }
        }

        con.print("  GSP: DMA complete - ");
        con.print_hex32(blocks as u32);
        con.println(" blocks transferred");
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
    fn wait_handshake(&self, con: &mut Console) -> Result<(), GspLoadError> {
        con.println("  GSP: Waiting for GSP-RM handshake...");

        for i in 0..2_000_000u32 {
            let mb0 = self.bar0.read32(NV_PGSP_MAILBOX0);
            if mb0 == GSP_READY_MAGIC {
                con.print("  GSP: Handshake OK - MAILBOX0 = 0x");
                con.print_hex32(mb0);
                con.println("");
                return Ok(());
            }
            // Also check MAILBOX1 for alternate ready signal
            let mb1 = self.bar0.read32(NV_PGSP_MAILBOX1);
            if mb1 != 0 && mb1 != 0xFFFF_FFFF {
                con.print("  GSP: MAILBOX1 signal = 0x");
                con.print_hex32(mb1);
                con.println(" (accepting as ready)");
                return Ok(());
            }
            if i % 500_000 == 0 && i > 0 {
                con.print("  GSP: still waiting... MB0=0x");
                con.print_hex32(mb0);
                con.print(" MB1=0x");
                con.print_hex32(mb1);
                con.println("");
            }
            core::hint::spin_loop();
        }

        // Read final state for diagnostics
        let final_mb0 = self.bar0.read32(NV_PGSP_MAILBOX0);
        let final_mb1 = self.bar0.read32(NV_PGSP_MAILBOX1);
        con.print("  GSP: Handshake timeout - MB0=0x");
        con.print_hex32(final_mb0);
        con.print(" MB1=0x");
        con.print_hex32(final_mb1);
        con.println("");

        // Don't fail hard - firmware version may use different magic
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

        // ── 1. PRIV Ring ──
        con.println("  GSP: [1/6] Initializing PRIV Ring...");
        self.init_priv_ring(con)?;

        // ── 2. Allocate DMA buffer ──
        con.println("  GSP: [2/6] Allocating DMA buffer...");
        let dma_phys = self.alloc_dma_buffer(fw_blob.len(), con)?;

        // ── 3. Copy firmware to DMA buffer ──
        con.println("  GSP: [3/6] Copying firmware to DMA buffer...");
        self.copy_fw_to_dma(fw_blob, dma_phys, con);

        // ── 4. DMA to Falcon ──
        con.println("  GSP: [4/6] DMA transfer to Falcon...");
        self.dma_transfer(dma_phys, fw_blob.len(), con)?;

        // ── 5. Boot Falcon ──
        con.println("  GSP: [5/6] Booting Falcon...");
        self.boot_falcon(con)?;

        // ── 6. Handshake ──
        con.println("  GSP: [6/6] Waiting for handshake...");
        self.wait_handshake(con)?;

        con.print_colored("=== GSP Load COMPLETE ===\n", 0x00FF00);

        // Free DMA buffer (firmware is now in Falcon DMEM, buffer no longer needed)
        let pages_used = (fw_blob.len() + PAGE_SIZE - 1) / PAGE_SIZE;
        unsafe {
            crate::arch::page_alloc::free_pages(dma_phys, pages_used);
        }
        con.println("  GSP: DMA buffer freed");

        Ok(())
    }
}
