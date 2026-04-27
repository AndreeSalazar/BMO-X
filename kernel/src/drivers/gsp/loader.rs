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

    // ── DMA transfer: copy 256 bytes from system RAM to Falcon IMEM/DMEM ──
    fn dma_xfer(&self, src_phys: u64, falcon_offset: u32, to_imem: bool) -> Result<(), GspLoadError> {
        // DMATRFBASE = source physical address >> 8
        self.bar0.write32(NV_PGSP_DMATRFBASE, (src_phys >> 8) as u32);

        // DMATRFMOFFS = destination offset in IMEM/DMEM
        self.bar0.write32(NV_PGSP_DMATRFMOFFS, falcon_offset);

        // DMATRFFBOFFS = source offset within the 256-byte aligned block
        self.bar0.write32(NV_PGSP_DMATRFFBOFFS, (src_phys & 0xFF) as u32);

        // Build command: WRITE | SIZE_256 | optionally IMEM
        let mut cmd = DMA_CMD_WRITE | DMA_CMD_SIZE_256;
        if to_imem {
            cmd |= DMA_CMD_IMEM;
        }
        self.bar0.write32(NV_PGSP_DMATRFCMD, cmd);

        // Wait for DMA to complete (DMATRFCMD bit 1 clears when done)
        for _ in 0..100_000 {
            let val = self.bar0.read32(NV_PGSP_DMATRFCMD);
            if val & DMA_CMD_WRITE == 0 {
                return Ok(());
            }
            core::hint::spin_loop();
        }

        // On Ampere, the DMA might complete instantly — check IDLESTATE
        Ok(())
    }

    // ── DMA load a segment to Falcon IMEM or DMEM ──
    fn dma_load_segment(&self, fw_blob: &[u8], seg: &super::elf_parser::LoadSegment,
                        con: &mut Console) -> Result<(), GspLoadError> {
        let file_off = seg.file_offset as usize;
        let size = seg.file_size as usize;
        let falcon_dst = seg.phys_addr as u32;
        let target = if seg.is_code { "IMEM" } else { "DMEM" };

        con.print("  GSP: DMA ");
        con.print(target);
        con.print(" offset=0x");
        con.print_hex32(falcon_dst);
        con.print(" size=0x");
        con.print_hex32(size as u32);
        con.newline();

        if file_off + size > fw_blob.len() {
            con.println("  GSP: WARNING - segment extends past firmware, skipping");
            return Ok(());
        }

        // Cap segment size to prevent loading huge data (Falcon IMEM ≤ 1MB)
        let max_load = if seg.is_code { 0x40000 } else { 0x80000 }; // 256KB IMEM, 512KB DMEM
        let actual_size = size.min(max_load);

        // DMA in 256-byte chunks
        let src_base = fw_blob.as_ptr() as u64 + file_off as u64;
        let chunks = (actual_size + 255) / 256;

        for i in 0..chunks {
            let src_phys = src_base + (i * 256) as u64;
            let dst_offset = falcon_dst + (i * 256) as u32;
            self.dma_xfer(src_phys, dst_offset, seg.is_code)?;
        }

        con.print("  GSP: DMA ");
        con.print(target);
        con.print(" OK (");
        con.print_hex32(chunks as u32);
        con.println(" chunks)");

        Ok(())
    }

    // ── Public: Full GSP load sequence ──
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

        con.print("  GSP: Firmware size = ");
        con.print_hex32(fw_blob.len() as u32);
        con.print(" bytes (");
        con.print_hex32((fw_blob.len() / (1024 * 1024)) as u32);
        con.println(" MB)");

        // ── 1. PRIV Ring init ──
        con.println("  GSP: [1/7] PRIV Ring + Falcon Reset...");
        self.init_priv_ring(con)?;

        // ── 2. Parse ELF to extract PT_LOAD segments ──
        con.println("  GSP: [2/7] Parsing firmware ELF...");
        let fw_info = super::elf_parser::parse_firmware(fw_blob, con)
            .ok_or(GspLoadError::BadElfMagic)?;

        // ── 3. Prepare boot args in RAM ──
        con.println("  GSP: [3/7] Preparing boot args (GSP_ARGUMENTS_CACHED)...");
        let (boot_args_phys, _shared_mem_phys) = self.prepare_boot_args(fw_blob, con)?;

        // ── 4. DMA transfer segments to Falcon IMEM/DMEM ──
        con.println("  GSP: [4/7] DMA loading segments into Falcon...");

        let mut loaded_any = false;
        for i in 0..fw_info.num_segments {
            let seg = &fw_info.segments[i];
            // Skip segments that are too large or have very high addresses
            // (they're for RISC-V mode, not Falcon IMEM/DMEM)
            if seg.file_size > 0x100000 || seg.phys_addr > 0x100000 {
                con.print("  GSP: Seg[");
                con.print_hex32(i as u32);
                con.println("] SKIP (too large/high addr for Falcon)");
                continue;
            }
            self.dma_load_segment(fw_blob, seg, con)?;
            loaded_any = true;
        }

        if !loaded_any {
            con.println("  GSP: No small segments found — firmware is RISC-V only");
            con.println("  GSP: Using whole-blob approach (pass addr via MAILBOX)...");
        }

        // ── 5. Configure MAILBOX with boot args and firmware address ──
        con.println("  GSP: [5/7] Writing MAILBOX...");
        let dma_phys = fw_blob.as_ptr() as u64;

        // Write boot_args address to MAILBOX0, firmware address to MAILBOX1
        self.bar0.write32(NV_PGSP_MAILBOX0, boot_args_phys as u32);
        self.bar0.write32(NV_PGSP_MAILBOX1, dma_phys as u32);

        let mb0 = self.bar0.read32(NV_PGSP_MAILBOX0);
        let mb1 = self.bar0.read32(NV_PGSP_MAILBOX1);
        con.print("  GSP: MB0=0x");
        con.print_hex32(mb0);
        con.print(" MB1=0x");
        con.print_hex32(mb1);
        con.newline();

        // ── 6. Boot Falcon ──
        con.println("  GSP: [6/7] Booting Falcon...");

        // Set boot vector from ELF entry point (truncated to 32-bit for Falcon)
        let bootvec = (fw_info.entry_point & 0xFFFF_FFFF) as u32;
        con.print("  GSP: BOOTVEC=0x");
        con.print_hex32(bootvec);
        con.newline();

        self.bar0.write32(NV_PGSP_FALCON_BOOTVEC, bootvec);
        self.bar0.write32(NV_PGSP_FALCON_CPUCTL, FALCON_CPUCTL_STARTCPU);

        // Wait for boot (running or halted)
        con.print("  GSP: Waiting ");
        for i in 0..5_000_000u32 {
            let cpuctl = self.bar0.read32(NV_PGSP_FALCON_CPUCTL);
            let halted = cpuctl & 0x10 != 0;

            if halted && i > 10 {
                con.print_colored(" HALTED\n", 0xFFFF00);
                con.print("  GSP: cpuctl=0x");
                con.print_hex32(cpuctl);
                con.print(" (");
                con.print_hex32(i);
                con.println(" loops)");

                let mb0 = self.bar0.read32(NV_PGSP_MAILBOX0);
                let mb1 = self.bar0.read32(NV_PGSP_MAILBOX1);
                con.print("  GSP: Post-boot MB0=0x");
                con.print_hex32(mb0);
                con.print(" MB1=0x");
                con.print_hex32(mb1);
                con.newline();

                if mb0 == 0 && i < 100 {
                    con.println("  GSP: Falcon halted instantly — no code in IMEM");
                } else if mb0 == 0 {
                    con.println("  GSP: Booter success (MB0=0)");
                } else {
                    con.print("  GSP: Booter error code: 0x");
                    con.print_hex32(mb0);
                    con.newline();
                }
                break;
            }

            if i % 1_000_000 == 0 && i > 0 { con.print("."); }
            core::hint::spin_loop();
        }

        // ── 7. Diagnose final state ──
        con.println("  GSP: [7/7] Final state diagnostics...");
        self.verify_gsp_rm(con)?;

        con.print_colored("=== GSP Load COMPLETE ===\n", 0x00FF00);
        Ok(())
    }

    /// Prepara shared memory + GSP_ARGUMENTS_CACHED en RAM
    fn prepare_boot_args(&self, _fw: &[u8], con: &mut Console) -> Result<(u64, u64), GspLoadError> {
        let base_phys = unsafe {
            crate::arch::page_alloc::alloc_pages_contiguous(4)
        }.ok_or(GspLoadError::PageAllocFailed)?;

        let boot_args_phys = base_phys;
        let shared_mem_phys = base_phys + 0x1000;

        unsafe {
            core::ptr::write_bytes(base_phys as *mut u8, 0, PAGE_SIZE * 4);
        }

        let args = crate::drivers::gsp::boot_args::GspArgumentsCached::new(
            shared_mem_phys, 3,
        );

        unsafe {
            let dst = boot_args_phys as *mut crate::drivers::gsp::boot_args::GspArgumentsCached;
            core::ptr::write(dst, args);
        }

        con.print("  GSP: BootArgs=0x");
        con.print_hex32(boot_args_phys as u32);
        con.print(" SharedMem=0x");
        con.print_hex32(shared_mem_phys as u32);
        con.newline();

        Ok((boot_args_phys, shared_mem_phys))
    }

    /// Diagnose final Falcon/GSP-RM state
    fn verify_gsp_rm(&self, con: &mut Console) -> Result<(), GspLoadError> {
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

        // Read Falcon IMEM/DMEM tag to verify DMA worked
        const NV_PGSP_IMEMC0: u32 = 0x0011_0180;
        const NV_PGSP_IMEMD0: u32 = 0x0011_0184;
        const NV_PGSP_DMEMC0: u32 = 0x0011_01C0;
        const NV_PGSP_DMEMD0: u32 = 0x0011_01C4;

        // Set IMEMC0 to read from offset 0
        self.bar0.write32(NV_PGSP_IMEMC0, 0x0000_0002); // offset=0, auto-increment
        let imem_val = self.bar0.read32(NV_PGSP_IMEMD0);
        con.print("  GSP: IMEM[0]=0x");
        con.print_hex32(imem_val);

        // Set DMEMC0 to read from offset 0
        self.bar0.write32(NV_PGSP_DMEMC0, 0x0000_0002);
        let dmem_val = self.bar0.read32(NV_PGSP_DMEMD0);
        con.print(" DMEM[0]=0x");
        con.print_hex32(dmem_val);
        con.newline();

        if imem_val == 0 && dmem_val == 0 {
            con.print_colored("  GSP: IMEM/DMEM empty — DMA transfer did NOT load code\n", 0xFF4444);
        } else {
            con.print_colored("  GSP: IMEM/DMEM has data — code was loaded!\n", 0x00FF00);
        }

        Ok(())
    }
}

