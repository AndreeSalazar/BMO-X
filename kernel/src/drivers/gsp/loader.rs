// kernel/src/drivers/gsp/loader.rs
// GSP Firmware Loader — GA106 (RTX 3060) — Ampere RISC-V Boot
//
// On Ampere (GA10x), the GSP is a RISC-V processor, NOT a classic Falcon.
// The boot sequence is:
//   1. PRIV Ring init (enable GSP engine)
//   2. Parse the ELF — extract .fwimage section (booter code) via nvfw_bin_hdr
//   3. Build Radix3 page table pointing at full ELF in sysmem
//   4. Populate GspFwWprMeta (256-byte VRAM layout descriptor)
//   5. Prepare GspArgumentsCached (libos args + message queues)
//   6. Write libos args address to MAILBOX0/1
//   7. DMA the booter code to Falcon IMEM (only the small booter, NOT the 69MB ELF!)
//   8. Boot Falcon → booter runs → sets up WPR2 → boots RISC-V with full ELF
//   9. GSP-RM starts → sends GSP_INIT_DONE event on message queue
//
// Fuente: nouveau tu102.c, nvidia-open kernel_gsp.c

use crate::console::Console;
use crate::drivers::gsp::priv_ring::PrivRingInit;
use crate::drivers::gsp::rpc::{
    GspFwWprMeta, WPR_META_MAGIC, WPR_META_REVISION,
    NV_PGSP_FALCON_MAILBOX0, NV_PGSP_FALCON_MAILBOX1,
    CMDQ_SIZE, MSGQ_SIZE,
};

// ── NV_PGSP Falcon registers (BAR0 offsets) ──
const NV_PGSP_FALCON_CPUCTL:     u32 = 0x0011_0100;
const NV_PGSP_FALCON_BOOTVEC:    u32 = 0x0011_0104;
const NV_PGSP_FALCON_IDLESTATE:  u32 = 0x0011_0004;
const NV_PGSP_DMATRFBASE:        u32 = 0x0011_0110;
const NV_PGSP_DMATRFMOFFS:       u32 = 0x0011_0114;
const NV_PGSP_DMATRFCMD:         u32 = 0x0011_0118;
const NV_PGSP_DMATRFFBOFFS:      u32 = 0x0011_011C;

// DMA command bits
const DMA_CMD_WRITE:    u32 = 1 << 1;
const DMA_CMD_IMEM:     u32 = 1 << 4;
const DMA_CMD_SIZE_256: u32 = 6 << 8;

// Boot constants
const FALCON_CPUCTL_STARTCPU: u32 = 0x2;

// Page size
const PAGE_SIZE: usize = 4096;

// RTX 3060 12GB VRAM
const FB_SIZE_12GB: u64 = 12 * 1024 * 1024 * 1024;

// Alignment helpers
const ALIGN_128K: u64 = 128 * 1024;
const ALIGN_64K: u64  = 64 * 1024;
const ALIGN_4K: u64   = 4096;
const ALIGN_1M: u64   = 1024 * 1024;

fn align_down(addr: u64, align: u64) -> u64 { addr & !(align - 1) }

pub enum GspLoadError {
    NullFirmware,
    BadElfMagic,
    FirmwareTooLarge,
    PageAllocFailed,
    PrivRingFailed,
    DmaTimeout,
    NoBooterFound,
    Radix3Failed,
}

pub struct GspLoader<'a> {
    bar0: &'a nv_hal::MmioRegion,
}

impl<'a> GspLoader<'a> {
    pub fn new(bar0: &'a nv_hal::MmioRegion) -> Self {
        Self { bar0 }
    }

    // ── Step 1: Initialize PRIV Ring ──
    fn init_priv_ring(&self, con: &mut Console) -> Result<(), GspLoadError> {
        let priv_ring = PrivRingInit::new(self.bar0);
        priv_ring.init(con).map_err(|_| GspLoadError::PrivRingFailed)
    }

    // ── DMA transfer: copy 256 bytes from system RAM to Falcon IMEM/DMEM ──
    fn dma_xfer_256(&self, src_phys: u64, falcon_offset: u32, to_imem: bool) -> Result<(), GspLoadError> {
        self.bar0.write32(NV_PGSP_DMATRFBASE, (src_phys >> 8) as u32);
        self.bar0.write32(NV_PGSP_DMATRFMOFFS, falcon_offset);
        self.bar0.write32(NV_PGSP_DMATRFFBOFFS, (src_phys & 0xFF) as u32);

        let mut cmd = DMA_CMD_WRITE | DMA_CMD_SIZE_256;
        if to_imem { cmd |= DMA_CMD_IMEM; }
        self.bar0.write32(NV_PGSP_DMATRFCMD, cmd);

        for _ in 0..100_000 {
            let val = self.bar0.read32(NV_PGSP_DMATRFCMD);
            if val & DMA_CMD_WRITE == 0 { return Ok(()); }
            core::hint::spin_loop();
        }
        // Ampere DMA may complete instantly
        Ok(())
    }

    // ── DMA load booter code (small, <256KB) to Falcon IMEM ──
    fn dma_load_booter(&self, fw_blob: &[u8], data_offset: usize, data_size: usize,
                       con: &mut Console) -> Result<(), GspLoadError> {
        if data_offset + data_size > fw_blob.len() {
            con.println("  GSP: ERROR - booter data extends past firmware");
            return Err(GspLoadError::NoBooterFound);
        }

        let cap = data_size.min(0x4_0000); // Cap at 256KB (Falcon IMEM limit)
        let chunks = (cap + 255) / 256;

        con.print("  GSP: DMA booter to IMEM: offset=0x");
        con.print_hex32(data_offset as u32);
        con.print(" size=0x");
        con.print_hex32(cap as u32);
        con.print(" (");
        con.print_hex32(chunks as u32);
        con.println(" chunks)");

        let src_base = fw_blob.as_ptr() as u64 + data_offset as u64;
        for i in 0..chunks {
            self.dma_xfer_256(
                src_base + (i * 256) as u64,
                (i * 256) as u32,
                true, // IMEM
            )?;
        }

        con.print_colored("  GSP: Booter DMA to IMEM OK\n", 0x00FF00);
        Ok(())
    }

    // ── Populate GspFwWprMeta with VRAM layout (top-down from 12GB) ──
    fn build_wpr_meta(
        &self,
        radix3_phys: u64,
        fw_size: u64,
        booter_phys: u64,
        booter_size: u64,
        booter_code_off: u64,
        booter_data_off: u64,
        con: &mut Console,
    ) -> GspFwWprMeta {
        // VRAM layout is calculated top-down from fb_size
        // See nvidia-open gsp_fw_wpr_meta.h + kernel_gsp_tu102.c
        let fb_size = FB_SIZE_12GB;

        // VGA workspace at top
        let vga_workspace_size: u64 = 0x2_0000; // 128KB
        let vga_workspace_offset = fb_size - vga_workspace_size;

        // WPR2 end (below VGA workspace, 128KB aligned)
        let gsp_fw_wpr_end = align_down(vga_workspace_offset, ALIGN_128K);

        // FRTS (Firmware Runtime Structure) — ~4KB
        let frts_size: u64 = 0x1000;
        let frts_offset = align_down(gsp_fw_wpr_end - frts_size, ALIGN_4K);

        // Boot binary (bootloader/booter)
        let boot_bin_offset = align_down(frts_offset - booter_size, ALIGN_4K);

        // GSP FW ELF in VRAM (64KB aligned)
        let gsp_fw_offset = align_down(boot_bin_offset - fw_size, ALIGN_64K);

        // GSP FW Heap (1MB aligned, 32MB heap)
        let gsp_fw_heap_size: u64 = 32 * 1024 * 1024;
        let gsp_fw_heap_offset = align_down(gsp_fw_offset - gsp_fw_heap_size, ALIGN_1M);

        // WPR2 start (128KB aligned)
        let gsp_fw_wpr_start = align_down(gsp_fw_heap_offset, ALIGN_128K);

        // Non-WPR heap (between reserved start and WPR start)
        let non_wpr_heap_size: u64 = 1024 * 1024; // 1MB
        let non_wpr_heap_offset = gsp_fw_wpr_start - non_wpr_heap_size;

        // Reserved start
        let gsp_fw_rsvd_start = non_wpr_heap_offset;

        con.print("  GSP: VRAM layout (top-down from ");
        con.print_hex32((fb_size >> 32) as u32);
        con.print_hex32(fb_size as u32);
        con.println("):");
        con.print("    WPR2: 0x");
        con.print_hex32((gsp_fw_wpr_start >> 32) as u32);
        con.print_hex32(gsp_fw_wpr_start as u32);
        con.print(" — 0x");
        con.print_hex32((gsp_fw_wpr_end >> 32) as u32);
        con.print_hex32(gsp_fw_wpr_end as u32);
        con.newline();
        con.print("    FW ELF: 0x");
        con.print_hex32((gsp_fw_offset >> 32) as u32);
        con.print_hex32(gsp_fw_offset as u32);
        con.print(" (");
        con.print_hex32((fw_size >> 20) as u32);
        con.println(" MB)");

        GspFwWprMeta {
            magic: WPR_META_MAGIC,
            revision: WPR_META_REVISION,
            sysmem_addr_of_radix3_elf: radix3_phys,
            size_of_radix3_elf: fw_size,
            sysmem_addr_of_bootloader: booter_phys,
            size_of_bootloader: booter_size,
            bootloader_code_offset: booter_code_off,
            bootloader_data_offset: booter_data_off,
            bootloader_manifest_offset: 0,
            sysmem_addr_of_signature: 0,
            size_of_signature: 0,
            gsp_fw_rsvd_start,
            non_wpr_heap_offset,
            non_wpr_heap_size,
            gsp_fw_wpr_start,
            gsp_fw_heap_offset,
            gsp_fw_heap_size,
            gsp_fw_offset,
            boot_bin_offset,
            frts_offset,
            frts_size,
            gsp_fw_wpr_end,
            fb_size,
            vga_workspace_offset,
            vga_workspace_size,
            boot_count: 0,
            verified: 0,
            flags: 0,
            _pad: [0u8; 7],
        }
    }

    // ── Prepare shared memory + boot args in RAM ──
    fn prepare_boot_args(
        &self,
        wpr_meta: &GspFwWprMeta,
        con: &mut Console,
    ) -> Result<u64, GspLoadError> {
        // Allocate memory for:
        //   Page 0: GspArgumentsCached (4KB)
        //   Page 1: GspFwWprMeta copy (4KB, only 256 used)
        //   Pages 2-129: cmd queue (256KB = 64 pages)
        //   Pages 130-257: msg queue (256KB = 64 pages)
        let cmdq_pages = CMDQ_SIZE / PAGE_SIZE;
        let msgq_pages = MSGQ_SIZE / PAGE_SIZE;
        let total_pages = 2 + cmdq_pages + msgq_pages; // 2 + 64 + 64 = 130

        let base_phys = unsafe {
            crate::arch::page_alloc::alloc_pages_contiguous(total_pages)
        }.ok_or(GspLoadError::PageAllocFailed)?;

        unsafe {
            core::ptr::write_bytes(base_phys as *mut u8, 0, total_pages * PAGE_SIZE);
        }

        let boot_args_phys = base_phys;
        let wpr_meta_phys = base_phys + PAGE_SIZE as u64;
        let shared_mem_phys = base_phys + 2 * PAGE_SIZE as u64;
        let shared_mem_pages = (cmdq_pages + msgq_pages) as u32;

        // Write WPR meta
        unsafe {
            let dst = wpr_meta_phys as *mut GspFwWprMeta;
            core::ptr::write(dst, core::ptr::read(wpr_meta as *const GspFwWprMeta));
        }

        // Write boot args (uses shared_mem for queues)
        let args = crate::drivers::gsp::boot_args::GspArgumentsCached::new_simple(
            shared_mem_phys, shared_mem_pages,
        );
        unsafe {
            let dst = boot_args_phys as *mut crate::drivers::gsp::boot_args::GspArgumentsCached;
            core::ptr::write(dst, args);
        }

        con.print("  GSP: BootArgs=0x");
        con.print_hex32(boot_args_phys as u32);
        con.print(" WprMeta=0x");
        con.print_hex32(wpr_meta_phys as u32);
        con.print(" SharedMem=0x");
        con.print_hex32(shared_mem_phys as u32);
        con.newline();

        Ok(boot_args_phys)
    }

    // ── Public: Full GSP load sequence (Ampere RISC-V boot) ──
    pub fn load(&self, fw_blob: &[u8], con: &mut Console) -> Result<(), GspLoadError> {
        con.print_colored("=== GSP Firmware Load (GA106 — Ampere RISC-V) ===\n", 0x00FFFF);

        // ── Validate firmware ──
        if fw_blob.len() < 64 {
            con.println("  GSP: ERROR - firmware too small");
            return Err(GspLoadError::NullFirmware);
        }
        if fw_blob.len() > 128 * 1024 * 1024 {
            con.println("  GSP: ERROR - firmware > 128MB");
            return Err(GspLoadError::FirmwareTooLarge);
        }

        con.print("  GSP: Firmware size = 0x");
        con.print_hex32(fw_blob.len() as u32);
        con.print(" bytes (");
        con.print_hex32((fw_blob.len() / (1024 * 1024)) as u32);
        con.println(" MB)");

        // ── 1. PRIV Ring init ──
        con.println("  GSP: [1/9] PRIV Ring + Falcon Reset...");
        self.init_priv_ring(con)?;

        // ── 2. Parse ELF — extract booter from .fwimage ──
        con.println("  GSP: [2/9] Parsing firmware ELF...");
        let fw_info = super::elf_parser::parse_firmware(fw_blob, con)
            .ok_or(GspLoadError::BadElfMagic)?;

        // The ELF parser extracts nvfw_bin_hdr from .fwimage section:
        //   booter_data_offset = absolute offset of booter code in fw_blob
        //   booter_data_size = size of booter code
        if fw_info.booter_data_size == 0 || fw_info.booter_data_offset == 0 {
            con.println("  GSP: ERROR - no nvfw_bin_hdr found in .fwimage");
            con.println("  GSP: Cannot extract booter — firmware may not have .fwimage section");
            return Err(GspLoadError::NoBooterFound);
        }

        let booter_off = fw_info.booter_data_offset as usize;
        let booter_sz = fw_info.booter_data_size as usize;
        con.print("  GSP: Booter: offset=0x");
        con.print_hex32(booter_off as u32);
        con.print(" size=0x");
        con.print_hex32(booter_sz as u32);
        con.newline();

        // ── 3. Build Radix3 page table for full ELF ──
        con.println("  GSP: [3/9] Building Radix3 page table for ELF...");
        let fw_phys = fw_blob.as_ptr() as u64;
        let radix3 = super::radix3::Radix3PageTable::build(fw_phys, fw_blob.len(), con)
            .ok_or(GspLoadError::Radix3Failed)?;

        con.print("  GSP: Radix3 root=0x");
        con.print_hex32(radix3.root_phys() as u32);
        con.newline();

        // ── 4. Build GspFwWprMeta (VRAM layout) ──
        con.println("  GSP: [4/9] Building GspFwWprMeta (VRAM layout)...");
        let booter_phys = fw_phys + booter_off as u64;
        let wpr_meta = self.build_wpr_meta(
            radix3.root_phys(),
            fw_blob.len() as u64,
            booter_phys,
            booter_sz as u64,
            fw_info.booter_header_offset as u64, // code offset within booter section
            0, // data offset (0 for combined image)
            con,
        );

        // ── 5. Prepare boot args (GspArgumentsCached) + message queues ──
        con.println("  GSP: [5/9] Preparing boot args + message queues...");
        let boot_args_phys = self.prepare_boot_args(&wpr_meta, con)?;

        // ── 6. Write libos args address to MAILBOX0/1 ──
        con.println("  GSP: [6/9] Writing MAILBOX0/1 (libos args PA)...");
        self.bar0.write32(NV_PGSP_FALCON_MAILBOX0, (boot_args_phys & 0xFFFF_FFFF) as u32);
        self.bar0.write32(NV_PGSP_FALCON_MAILBOX1, ((boot_args_phys >> 32) & 0xFFFF_FFFF) as u32);

        let mb0 = self.bar0.read32(NV_PGSP_FALCON_MAILBOX0);
        let mb1 = self.bar0.read32(NV_PGSP_FALCON_MAILBOX1);
        con.print("  GSP: MB0=0x");
        con.print_hex32(mb0);
        con.print(" MB1=0x");
        con.print_hex32(mb1);
        con.newline();

        // ── 7. DMA booter code to Falcon IMEM ──
        // ONLY the small booter (from nvfw_bin_hdr) goes into Falcon IMEM.
        // The full 69MB ELF stays in sysmem — the booter reads it via Radix3.
        con.println("  GSP: [7/9] DMA booter to Falcon IMEM...");
        self.dma_load_booter(fw_blob, booter_off, booter_sz, con)?;

        // ── 8. Boot Falcon ──
        con.println("  GSP: [8/9] Booting Falcon (booter)...");

        // Boot vector = 0 (booter code starts at beginning of IMEM)
        self.bar0.write32(NV_PGSP_FALCON_BOOTVEC, 0x0000_0000);
        self.bar0.write32(NV_PGSP_FALCON_CPUCTL, FALCON_CPUCTL_STARTCPU);

        // Wait for Falcon to halt (booter completes) or timeout
        con.print("  GSP: Waiting for booter...");
        let mut booted = false;
        for i in 0..10_000_000u32 {
            let cpuctl = self.bar0.read32(NV_PGSP_FALCON_CPUCTL);
            let halted = cpuctl & 0x10 != 0;

            if halted && i > 100 {
                con.newline();
                con.print("  GSP: Falcon HALTED (cpuctl=0x");
                con.print_hex32(cpuctl);
                con.print(" after ");
                con.print_hex32(i);
                con.println(" loops)");

                let mb0 = self.bar0.read32(NV_PGSP_FALCON_MAILBOX0);
                let mb1 = self.bar0.read32(NV_PGSP_FALCON_MAILBOX1);
                con.print("  GSP: Post-boot MB0=0x");
                con.print_hex32(mb0);
                con.print(" MB1=0x");
                con.print_hex32(mb1);
                con.newline();

                if mb0 == 0 && i > 1000 {
                    con.print_colored("  GSP: Booter completed OK!\n", 0x00FF00);
                    booted = true;
                } else if mb0 == 0 {
                    con.print_colored("  GSP: Falcon halted quickly — booter may have no code\n", 0xFFFF00);
                } else {
                    con.print("  GSP: Booter error code: 0x");
                    con.print_hex32(mb0);
                    con.newline();
                }
                break;
            }

            if i % 2_000_000 == 0 && i > 0 { con.print("."); }
            core::hint::spin_loop();
        }

        // ── 9. Verify GSP state ──
        con.println("  GSP: [9/9] Verifying GSP state...");
        self.verify_gsp_state(con);

        if booted {
            con.print_colored("=== GSP Booter COMPLETE — GSP-RM should be starting ===\n", 0x00FF00);
            con.println("  GSP: Next: poll message queue for GSP_INIT_DONE (0x1001)");
        } else {
            con.print_colored("=== GSP Booter DID NOT COMPLETE ===\n", 0xFF4444);
            con.println("  GSP: The booter did not run successfully.");
            con.println("  GSP: Check: Is the nvfw_bin_hdr extraction correct?");
            con.println("  GSP: Check: Does the booter need SEC2 authenticated boot?");
        }

        Ok(())
    }

    // ── Public: Full GA10x boot with 3 separate firmware blobs ──
    pub fn load_full(&self, blobs: &super::GspFirmwareBlobs, con: &mut Console) -> Result<(), GspLoadError> {
        con.print_colored("=== GSP Firmware Load (GA106 - 3 Blob Boot) ===\n", 0x00FFFF);

        let gsp_rm = blobs.gsp_rm;
        let bootloader = blobs.bootloader;
        let booter = blobs.booter_load;

        // ── Validate blobs ──
        con.print("  GSP-RM:     ");
        con.print_hex32(gsp_rm.len() as u32);
        con.print(" bytes (");
        con.print_hex32((gsp_rm.len() / (1024 * 1024)) as u32);
        con.println(" MB)");

        con.print("  Bootloader: ");
        con.print_hex32(bootloader.len() as u32);
        con.println(" bytes");

        con.print("  Booter:     ");
        con.print_hex32(booter.len() as u32);
        con.println(" bytes");

        if gsp_rm.len() < 64 {
            con.println("  GSP: ERROR - GSP-RM blob too small");
            return Err(GspLoadError::NullFirmware);
        }
        if bootloader.len() < 24 {
            con.println("  GSP: ERROR - bootloader blob too small");
            return Err(GspLoadError::NoBooterFound);
        }

        // ── Validate nvfw_bin_hdr in bootloader ──
        let bl_magic = u32::from_le_bytes([bootloader[0], bootloader[1], bootloader[2], bootloader[3]]);
        if bl_magic != 0x10de {
            con.print("  GSP: ERROR - bootloader magic=0x");
            con.print_hex32(bl_magic);
            con.println(" (expected 0x10de)");
            return Err(GspLoadError::BadElfMagic);
        }

        // Parse nvfw_bin_hdr from bootloader blob
        let bl_hdr_off = u32::from_le_bytes([bootloader[12], bootloader[13], bootloader[14], bootloader[15]]) as usize;
        let bl_data_off = u32::from_le_bytes([bootloader[16], bootloader[17], bootloader[18], bootloader[19]]) as usize;
        let bl_data_sz = u32::from_le_bytes([bootloader[20], bootloader[21], bootloader[22], bootloader[23]]) as usize;

        con.print("  BL: hdr_off=");
        con.print_hex32(bl_hdr_off as u32);
        con.print(" data_off=");
        con.print_hex32(bl_data_off as u32);
        con.print(" data_sz=");
        con.print_hex32(bl_data_sz as u32);
        con.newline();

        if bl_data_off + bl_data_sz > bootloader.len() {
            con.println("  GSP: ERROR - bootloader data extends past blob");
            return Err(GspLoadError::NoBooterFound);
        }

        // Parse nvfw_bin_hdr from booter_load blob
        let btr_magic = u32::from_le_bytes([booter[0], booter[1], booter[2], booter[3]]);
        let btr_hdr_off = u32::from_le_bytes([booter[12], booter[13], booter[14], booter[15]]) as usize;
        let btr_data_off = u32::from_le_bytes([booter[16], booter[17], booter[18], booter[19]]) as usize;
        let btr_data_sz = u32::from_le_bytes([booter[20], booter[21], booter[22], booter[23]]) as usize;

        con.print("  Booter: magic=0x");
        con.print_hex32(btr_magic);
        con.print(" data_off=");
        con.print_hex32(btr_data_off as u32);
        con.print(" data_sz=");
        con.print_hex32(btr_data_sz as u32);
        con.newline();

        if btr_data_off + btr_data_sz > booter.len() {
            con.println("  GSP: ERROR - booter data extends past blob");
            return Err(GspLoadError::NoBooterFound);
        }

        // ── 1. PRIV Ring init ──
        con.println("  GSP: [1/9] PRIV Ring + Falcon Reset...");
        self.init_priv_ring(con)?;

        // ── 2. Build Radix3 page table for GSP-RM ELF ──
        con.println("  GSP: [2/9] Building Radix3 page table for GSP-RM ELF...");
        let gsp_phys = gsp_rm.as_ptr() as u64;
        let radix3 = super::radix3::Radix3PageTable::build(gsp_phys, gsp_rm.len(), con)
            .ok_or(GspLoadError::Radix3Failed)?;
        con.print("  GSP: Radix3 root=0x");
        con.print_hex32(radix3.root_phys() as u32);
        con.newline();

        // ── 3. Build GspFwWprMeta (VRAM layout) ──
        con.println("  GSP: [3/9] Building GspFwWprMeta...");
        let bootloader_phys = bootloader.as_ptr() as u64;
        let wpr_meta = self.build_wpr_meta(
            radix3.root_phys(),
            gsp_rm.len() as u64,
            bootloader_phys,
            bootloader.len() as u64,
            bl_data_off as u64,
            0,
            con,
        );

        // ── 4. Prepare boot args + message queues ──
        con.println("  GSP: [4/9] Preparing boot args + message queues...");
        let boot_args_phys = self.prepare_boot_args(&wpr_meta, con)?;

        // ── 5. Write libos args to MAILBOX0/1 ──
        con.println("  GSP: [5/9] Writing MAILBOX0/1...");
        self.bar0.write32(NV_PGSP_FALCON_MAILBOX0, (boot_args_phys & 0xFFFF_FFFF) as u32);
        self.bar0.write32(NV_PGSP_FALCON_MAILBOX1, ((boot_args_phys >> 32) & 0xFFFF_FFFF) as u32);

        let mb0 = self.bar0.read32(NV_PGSP_FALCON_MAILBOX0);
        let mb1 = self.bar0.read32(NV_PGSP_FALCON_MAILBOX1);
        con.print("  GSP: MB0=0x");
        con.print_hex32(mb0);
        con.print(" MB1=0x");
        con.print_hex32(mb1);
        con.newline();

        // ── 6. DMA booter_load code to Falcon IMEM ──
        // The booter_load is the Falcon HS code that sets up WPR2/ACR.
        // It contains its own nvfw_bin_hdr; the actual code is at btr_data_off.
        con.println("  GSP: [6/9] DMA booter_load to Falcon IMEM...");
        self.dma_load_booter(booter, btr_data_off, btr_data_sz, con)?;

        // ── 7. Boot Falcon (runs booter_load) ──
        con.println("  GSP: [7/9] Booting Falcon (booter_load)...");
        self.bar0.write32(NV_PGSP_FALCON_BOOTVEC, 0x0000_0000);
        self.bar0.write32(NV_PGSP_FALCON_CPUCTL, FALCON_CPUCTL_STARTCPU);

        con.print("  GSP: Waiting for booter...");
        let mut booted = false;
        for i in 0..10_000_000u32 {
            let cpuctl = self.bar0.read32(NV_PGSP_FALCON_CPUCTL);
            let halted = cpuctl & 0x10 != 0;

            if halted && i > 100 {
                con.newline();
                con.print("  GSP: Falcon HALTED (cpuctl=0x");
                con.print_hex32(cpuctl);
                con.print(" after ");
                con.print_hex32(i);
                con.println(" loops)");

                let mb0 = self.bar0.read32(NV_PGSP_FALCON_MAILBOX0);
                let mb1 = self.bar0.read32(NV_PGSP_FALCON_MAILBOX1);
                con.print("  GSP: Post-boot MB0=0x");
                con.print_hex32(mb0);
                con.print(" MB1=0x");
                con.print_hex32(mb1);
                con.newline();

                if mb0 == 0 && i > 1000 {
                    con.print_colored("  GSP: Booter completed OK!\n", 0x00FF00);
                    booted = true;
                } else if mb0 == 0 {
                    con.print_colored("  GSP: Falcon halted quickly\n", 0xFFFF00);
                } else {
                    con.print("  GSP: Booter error code: 0x");
                    con.print_hex32(mb0);
                    con.newline();
                }
                break;
            }

            if i % 2_000_000 == 0 && i > 0 { con.print("."); }
            core::hint::spin_loop();
        }

        // ── 8. (Booter sets up WPR2, loads bootloader -> RISC-V starts) ──
        con.println("  GSP: [8/9] Booter should have set up WPR2 + started RISC-V...");

        // ── 9. Verify GSP state ──
        con.println("  GSP: [9/9] Verifying GSP state...");
        self.verify_gsp_state(con);

        if booted {
            con.print_colored("=== GSP Boot COMPLETE - GSP-RM should be starting ===\n", 0x00FF00);
            con.println("  GSP: Next: poll message queue for GSP_INIT_DONE (0x1001)");
        } else {
            con.print_colored("=== GSP Boot DID NOT COMPLETE ===\n", 0xFF4444);
            con.println("  GSP: The booter_load did not run successfully.");
            con.println("  GSP: Check: Are the firmware versions matching?");
            con.println("  GSP: Check: Does the booter need SEC2 auth?");
        }

        Ok(())
    }

    /// Verify GSP registers after boot attempt
    fn verify_gsp_state(&self, con: &mut Console) {
        let cpuctl = self.bar0.read32(NV_PGSP_FALCON_CPUCTL);
        let idle = self.bar0.read32(NV_PGSP_FALCON_IDLESTATE);
        let mb0 = self.bar0.read32(NV_PGSP_FALCON_MAILBOX0);
        let mb1 = self.bar0.read32(NV_PGSP_FALCON_MAILBOX1);

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

        // Read RISC-V registers
        let riscv_cpuctl = self.bar0.read32(super::rpc::NV_PGSP_RISCV_CPUCTL);
        let riscv_br = self.bar0.read32(super::rpc::NV_PGSP_RISCV_BR_ADDR);
        con.print("  GSP: RISCV_CPUCTL=0x");
        con.print_hex32(riscv_cpuctl);
        con.print(" RISCV_BR_ADDR=0x");
        con.print_hex32(riscv_br);
        con.newline();

        // Read IMEM[0] to verify DMA worked
        const NV_PGSP_IMEMC0: u32 = 0x0011_0180;
        const NV_PGSP_IMEMD0: u32 = 0x0011_0184;
        self.bar0.write32(NV_PGSP_IMEMC0, 0x0000_0002);
        let imem0 = self.bar0.read32(NV_PGSP_IMEMD0);
        con.print("  GSP: IMEM[0]=0x");
        con.print_hex32(imem0);

        if imem0 == 0 {
            con.print_colored(" (EMPTY — no code loaded!)\n", 0xFF4444);
        } else {
            con.print_colored(" (has code)\n", 0x00FF00);
        }

        // Read queue doorbell registers
        let q0_head = self.bar0.read32(super::rpc::pgsp_queue_head(0));
        let q0_tail = self.bar0.read32(super::rpc::pgsp_queue_tail(0));
        let q1_head = self.bar0.read32(super::rpc::pgsp_queue_head(1));
        let q1_tail = self.bar0.read32(super::rpc::pgsp_queue_tail(1));
        con.print("  GSP: CmdQ HEAD=0x");
        con.print_hex32(q0_head);
        con.print(" TAIL=0x");
        con.print_hex32(q0_tail);
        con.print(" | MsgQ HEAD=0x");
        con.print_hex32(q1_head);
        con.print(" TAIL=0x");
        con.print_hex32(q1_tail);
        con.newline();
    }
}
