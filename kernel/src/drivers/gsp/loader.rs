// kernel/src/drivers/gsp/loader.rs
// GSP Firmware Loader — GA106 (RTX 3060) — Ampere RISC-V Boot via SEC2
//
// On Ampere (GA10x), the GSP is a RISC-V processor. The boot path is:
//   1. PRIV Ring init (enable GSP + SEC2 engines)
//   2. Parse ELF — extract .fwimage + .fwsignature_ga10x sections
//   3. Build Radix3 page table pointing at full ELF in sysmem
//   4. Populate GspFwWprMeta (256-byte VRAM layout descriptor)
//   5. Prepare GspArgumentsCached (libos args + message queues)
//   6. Reset GSP Falcon into RISC-V mode (register 0x00111668)
//   7. Write libos args to PGSP MAILBOX0/1 (for GSP libos, read after RISC-V boots)
//   8. Write WPR meta phys to SEC2 MAILBOX0/1 (for booter_load)
//   9. DMA booter_load HS code to SEC2 Falcon IMEM → boot SEC2
//  10. SEC2 runs booter_load → sets up WPR2 → loads bootloader → starts RISC-V
//  11. GSP RISC-V boots libos → reads GspArgumentsCached → sends GSP_INIT_DONE
//
// CRITICAL: booter_load runs on SEC2, NOT on PGSP Falcon directly.
// Running unauthenticated code on PGSP causes immediate halt (HS mode).
//
// Fuente: nouveau tu102.c + ga102.c, nvidia-open kernel_gsp.c

use crate::console::Console;
use crate::drivers::gsp::priv_ring::PrivRingInit;
use crate::drivers::gsp::rpc::{
    GspFwWprMeta, WPR_META_MAGIC, WPR_META_REVISION,
    NV_PGSP_FALCON_MAILBOX0, NV_PGSP_FALCON_MAILBOX1,
    NV_PGSP_RISCV_MODE, NV_PGSP_RISCV_MODE_MASK,
    NV_WPR2_HI,
    NV_PSEC2_FALCON_MAILBOX0, NV_PSEC2_FALCON_MAILBOX1,
    NV_PSEC2_FALCON_CPUCTL, NV_PSEC2_FALCON_BOOTVEC,
    NV_PSEC2_FALCON_IDLESTATE, NV_PSEC2_FALCON_RESET,
    NV_PSEC2_FALCON_ENGINE,
    NV_PSEC2_DMATRFBASE, NV_PSEC2_DMATRFMOFFS,
    NV_PSEC2_DMATRFCMD, NV_PSEC2_DMATRFFBOFFS,
    NV_PSEC2_FALCON_EMEM_ACCESS, NV_PSEC2_FALCON_UCODE_ID,
    NV_PSEC2_FALCON_ENGINE_ID, NV_PSEC2_FALCON_DMEM_SIGN,
    NV_PSEC2_FALCON_DMACTL, NV_PSEC2_FALCON_IRQMSET,
    NV_PSEC2_FALCON_ENGCTL,
    CMDQ_SIZE, MSGQ_SIZE,
};

// ── NV_PGSP Falcon registers (BAR0 offsets) — kept for diagnostics ──
const NV_PGSP_FALCON_CPUCTL:     u32 = 0x0011_0100;
const NV_PGSP_FALCON_BOOTVEC:    u32 = 0x0011_0104;
const NV_PGSP_FALCON_IDLESTATE:  u32 = 0x0011_0004;
const NV_PGSP_FALCON_RESET:      u32 = 0x0011_0094;
const NV_PGSP_DMATRFBASE:        u32 = 0x0011_0110;
const NV_PGSP_DMATRFMOFFS:       u32 = 0x0011_0114;
const NV_PGSP_DMATRFCMD:         u32 = 0x0011_0118;
const NV_PGSP_DMATRFFBOFFS:      u32 = 0x0011_011C;

// DMA command bits (same for both PGSP and SEC2 Falcon)
const DMA_CMD_WRITE:    u32 = 1 << 1;
const DMA_CMD_IMEM:     u32 = 1 << 4;
const DMA_CMD_SIZE_256: u32 = 6 << 8;

// Boot constants
const FALCON_CPUCTL_STARTCPU: u32 = 0x2;
const FALCON_CPUCTL_HALTED:   u32 = 0x10;

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
    Sec2BootFailed,
    RiscvStartFailed,
    SignatureNotFound,
    HsManifestParseError,
}

/// Both physical addresses needed for the two-mailbox boot handoff
struct BootMem {
    boot_args_phys: u64,  // GspArgumentsCached → PGSP MAILBOX
    wpr_meta_phys: u64,   // GspFwWprMeta copy → SEC2 MAILBOX
    shared_mem_phys: u64,
}

/// Parsed HS manifest from booter_load-535.113.01.bin
/// Layout (from nouveau extract-firmware-nouveau.py + nvfw/hs.h):
///   nvfw_bin_hdr (24 bytes) → nvfw_hs_header_v2 (36 bytes) → signatures
///   → patch_loc(u32) + patch_sig(u32) + meta(12 bytes) + num_sigs(u32)
///   → nvfw_hs_load_header_v2 (20+ bytes)
struct HsManifest {
    // From nvfw_bin_hdr
    data_offset: u32,
    data_size: u32,
    // From nvfw_hs_header_v2
    sig_prod_offset: u32,
    sig_prod_size: u32,
    // Values read via patch_loc/patch_sig offsets
    patch_loc_value: u32,   // offset within data where WPR meta PA goes
    patch_sig_value: u32,   // offset within data where sig index goes
    // From meta_data
    fuse_ver: u32,
    engine_id: u32,
    ucode_id: u32,
    num_sigs: u32,
    // From nvfw_hs_load_header_v2
    os_code_offset: u32,
    os_code_size: u32,
    os_data_offset: u32,
    os_data_size: u32,
    app0_offset: u32,
    app0_size: u32,
    // Computed
    dmem_sign: u32,         // = patch_loc_value - os_data_offset
}

pub struct GspLoader<'a> {
    bar0: &'a nv_hal::MmioRegion,
}

impl<'a> GspLoader<'a> {
    pub fn new(bar0: &'a nv_hal::MmioRegion) -> Self {
        Self { bar0 }
    }

    // ── Step 1: Initialize PRIV Ring (enables GSP + SEC2) ──
    fn init_priv_ring(&self, con: &mut Console) -> Result<(), GspLoadError> {
        let priv_ring = PrivRingInit::new(self.bar0);
        priv_ring.init(con).map_err(|_| GspLoadError::PrivRingFailed)
    }

    // ── SEC2 DMA transfer: copy 256 bytes from sysmem to SEC2 Falcon IMEM ──
    fn sec2_dma_xfer_256(&self, src_phys: u64, falcon_offset: u32, to_imem: bool) -> Result<(), GspLoadError> {
        self.bar0.write32(NV_PSEC2_DMATRFBASE, (src_phys >> 8) as u32);
        self.bar0.write32(NV_PSEC2_DMATRFMOFFS, falcon_offset);
        self.bar0.write32(NV_PSEC2_DMATRFFBOFFS, (src_phys & 0xFF) as u32);

        let mut cmd = DMA_CMD_WRITE | DMA_CMD_SIZE_256;
        if to_imem { cmd |= DMA_CMD_IMEM; }
        self.bar0.write32(NV_PSEC2_DMATRFCMD, cmd);

        for _ in 0..100_000 {
            let val = self.bar0.read32(NV_PSEC2_DMATRFCMD);
            if val & DMA_CMD_WRITE == 0 { return Ok(()); }
            core::hint::spin_loop();
        }
        Err(GspLoadError::DmaTimeout)
    }

    // ── PGSP DMA transfer: copy 256 bytes from sysmem to PGSP Falcon IMEM/DMEM ──
    fn pgsp_dma_xfer_256(&self, src_phys: u64, falcon_offset: u32, to_imem: bool) -> Result<(), GspLoadError> {
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
        Err(GspLoadError::DmaTimeout)
    }

    // ── DMA load booter_load HS code to SEC2 Falcon IMEM ──
    fn sec2_dma_load_booter(&self, fw_blob: &[u8], data_offset: usize, data_size: usize,
                            con: &mut Console) -> Result<(), GspLoadError> {
        if data_offset + data_size > fw_blob.len() {
            con.println("  GSP: ERROR - booter data extends past firmware");
            return Err(GspLoadError::NoBooterFound);
        }

        let cap = data_size.min(0x4_0000); // Cap at 256KB (Falcon IMEM limit)
        let chunks = (cap + 255) / 256;

        con.print("  GSP: DMA booter_load to SEC2 IMEM: offset=0x");
        con.print_hex32(data_offset as u32);
        con.print(" size=0x");
        con.print_hex32(cap as u32);
        con.print(" (");
        con.print_hex32(chunks as u32);
        con.println(" chunks)");

        let src_base = fw_blob.as_ptr() as u64 + data_offset as u64;
        for i in 0..chunks {
            self.sec2_dma_xfer_256(
                src_base + (i * 256) as u64,
                (i * 256) as u32,
                true, // IMEM
            )?;
        }

        con.print_colored("  GSP: Booter DMA to SEC2 IMEM OK\n", 0x00FF00);
        Ok(())
    }

    // ── Parse nvfw_bin_hdr + nvfw_hs_header_v2 + nvfw_hs_load_header_v2 from booter .bin ──
    fn parse_hs_manifest(blob: &[u8], con: &mut Console) -> Result<HsManifest, GspLoadError> {
        fn r32(d: &[u8], o: usize) -> u32 {
            if o + 4 > d.len() { return 0; }
            u32::from_le_bytes([d[o], d[o+1], d[o+2], d[o+3]])
        }

        if blob.len() < 24 {
            con.println("  GSP: [HS] blob too small for nvfw_bin_hdr");
            return Err(GspLoadError::HsManifestParseError);
        }

        // ── nvfw_bin_hdr (24 bytes) ──
        let bin_magic = r32(blob, 0);
        let bin_ver   = r32(blob, 4);
        let bin_size  = r32(blob, 8);
        let hdr_off   = r32(blob, 12) as usize;  // offset to nvfw_hs_header_v2
        let data_off  = r32(blob, 16) as usize;  // offset to actual firmware code/data
        let data_sz   = r32(blob, 20) as usize;

        con.print("  GSP: [HS] bin_hdr: magic=0x");
        con.print_hex32(bin_magic);
        con.print(" ver=");
        con.print_hex32(bin_ver);
        con.print(" size=0x");
        con.print_hex32(bin_size);
        con.newline();
        con.print("  GSP: [HS]   hdr_off=0x");
        con.print_hex32(hdr_off as u32);
        con.print(" data_off=0x");
        con.print_hex32(data_off as u32);
        con.print(" data_sz=0x");
        con.print_hex32(data_sz as u32);
        con.newline();

        if bin_magic != 0x10de {
            con.println("  GSP: [HS] ERROR — bad bin_magic");
            return Err(GspLoadError::HsManifestParseError);
        }

        // ── nvfw_hs_header_v2 (36 bytes = 9 × u32) at hdr_off ──
        if hdr_off + 36 > blob.len() {
            con.println("  GSP: [HS] ERROR — hs_header_v2 extends past blob");
            return Err(GspLoadError::HsManifestParseError);
        }
        let sig_prod_offset    = r32(blob, hdr_off + 0);
        let sig_prod_size      = r32(blob, hdr_off + 4);
        let patch_loc_offset   = r32(blob, hdr_off + 8) as usize;
        let patch_sig_offset   = r32(blob, hdr_off + 12) as usize;
        let meta_data_offset   = r32(blob, hdr_off + 16) as usize;
        let _meta_data_size    = r32(blob, hdr_off + 20);
        let num_sig_offset     = r32(blob, hdr_off + 24) as usize;
        let load_hdr_offset    = r32(blob, hdr_off + 28) as usize;
        let load_hdr_size      = r32(blob, hdr_off + 32);

        con.print("  GSP: [HS] hs_hdr_v2: sig_off=0x");
        con.print_hex32(sig_prod_offset);
        con.print(" sig_sz=0x");
        con.print_hex32(sig_prod_size);
        con.newline();
        con.print("  GSP: [HS]   patch_loc@0x");
        con.print_hex32(patch_loc_offset as u32);
        con.print(" patch_sig@0x");
        con.print_hex32(patch_sig_offset as u32);
        con.print(" meta@0x");
        con.print_hex32(meta_data_offset as u32);
        con.newline();
        con.print("  GSP: [HS]   num_sig@0x");
        con.print_hex32(num_sig_offset as u32);
        con.print(" load_hdr@0x");
        con.print_hex32(load_hdr_offset as u32);
        con.print(" load_hdr_sz=0x");
        con.print_hex32(load_hdr_size);
        con.newline();

        // Read values at patch_loc and patch_sig offsets
        if patch_loc_offset + 4 > blob.len() || patch_sig_offset + 4 > blob.len() {
            con.println("  GSP: [HS] ERROR — patch offsets out of bounds");
            return Err(GspLoadError::HsManifestParseError);
        }
        let patch_loc_value = r32(blob, patch_loc_offset);
        let patch_sig_value = r32(blob, patch_sig_offset);

        con.print("  GSP: [HS]   patch_loc_value=0x");
        con.print_hex32(patch_loc_value);
        con.print(" patch_sig_value=0x");
        con.print_hex32(patch_sig_value);
        con.newline();

        // Read meta_data: fuse_ver, engine_id, ucode_id
        if meta_data_offset + 12 > blob.len() {
            con.println("  GSP: [HS] ERROR — meta_data out of bounds");
            return Err(GspLoadError::HsManifestParseError);
        }
        let fuse_ver  = r32(blob, meta_data_offset);
        let engine_id = r32(blob, meta_data_offset + 4);
        let ucode_id  = r32(blob, meta_data_offset + 8);

        // Read num_sigs
        if num_sig_offset + 4 > blob.len() {
            con.println("  GSP: [HS] ERROR — num_sig out of bounds");
            return Err(GspLoadError::HsManifestParseError);
        }
        let num_sigs = r32(blob, num_sig_offset);

        con.print("  GSP: [HS]   fuse_ver=0x");
        con.print_hex32(fuse_ver);
        con.print(" engine_id=0x");
        con.print_hex32(engine_id);
        con.print(" ucode_id=0x");
        con.print_hex32(ucode_id);
        con.print(" num_sigs=");
        con.print_hex32(num_sigs);
        con.newline();

        // ── nvfw_hs_load_header_v2 at load_hdr_offset ──
        // struct: os_code_offset(4), os_code_size(4), os_data_offset(4),
        //         os_data_size(4), num_apps(4), app[num_apps] × {offset,size,data_off,data_sz}
        if load_hdr_offset + 20 > blob.len() {
            con.println("  GSP: [HS] ERROR — load_hdr out of bounds");
            return Err(GspLoadError::HsManifestParseError);
        }
        let os_code_offset = r32(blob, load_hdr_offset);
        let os_code_size   = r32(blob, load_hdr_offset + 4);
        let os_data_offset = r32(blob, load_hdr_offset + 8);
        let os_data_size   = r32(blob, load_hdr_offset + 12);
        let num_apps       = r32(blob, load_hdr_offset + 16);

        con.print("  GSP: [HS] load_hdr: os_code_off=0x");
        con.print_hex32(os_code_offset);
        con.print(" os_code_sz=0x");
        con.print_hex32(os_code_size);
        con.newline();
        con.print("  GSP: [HS]   os_data_off=0x");
        con.print_hex32(os_data_offset);
        con.print(" os_data_sz=0x");
        con.print_hex32(os_data_size);
        con.print(" num_apps=");
        con.print_hex32(num_apps);
        con.newline();

        // Read app[0] if available
        let (app0_offset, app0_size) = if num_apps > 0 && load_hdr_offset + 20 + 16 <= blob.len() {
            let app_base = load_hdr_offset + 20;
            let off = r32(blob, app_base);
            let sz  = r32(blob, app_base + 4);
            con.print("  GSP: [HS]   app[0] off=0x");
            con.print_hex32(off);
            con.print(" sz=0x");
            con.print_hex32(sz);
            con.newline();
            (off, sz)
        } else {
            (os_code_offset, os_code_size)
        };

        // Compute dmem_sign: offset of WPR meta PA within DMEM
        let dmem_sign = patch_loc_value.checked_sub(os_data_offset).unwrap_or(0);
        con.print("  GSP: [HS]   dmem_sign=0x");
        con.print_hex32(dmem_sign);
        con.println(" (patch_loc - os_data_offset)");

        Ok(HsManifest {
            data_offset: data_off as u32,
            data_size: data_sz as u32,
            sig_prod_offset,
            sig_prod_size,
            patch_loc_value,
            patch_sig_value,
            fuse_ver,
            engine_id,
            ucode_id,
            num_sigs,
            os_code_offset,
            os_code_size,
            os_data_offset,
            os_data_size,
            app0_offset,
            app0_size,
            dmem_sign,
        })
    }

    /// HS-authenticated boot of booter_load on SEC2 (GA102+ path).
    ///
    /// 1. Parse HS manifest from booter blob
    /// 2. Configure SEC2 DMA/cache registers (ga102_flcn_fw_load style)
    /// 3. DMA IMEM (os_code) and DMEM (os_data) separately
    /// 4. Patch DMEM at patch_loc with WPR meta PA
    /// 5. Program HS registers (dmem_sign, engine_id, ucode_id)
    /// 6. Boot SEC2 and wait
    fn sec2_hs_boot_booter(
        &self,
        booter: &[u8],
        wpr_meta_phys: u64,
        con: &mut Console,
    ) -> Result<(), GspLoadError> {
        con.println("  GSP: [HS-BOOT] Parsing booter_load HS manifest...");
        let hs = Self::parse_hs_manifest(booter, con)?;

        let data_base = hs.data_offset as usize;
        if data_base + hs.data_size as usize > booter.len() {
            con.println("  GSP: [HS-BOOT] ERROR — data section extends past booter blob");
            return Err(GspLoadError::NoBooterFound);
        }

        // ── Reset SEC2 ──
        self.reset_sec2_falcon(con);

        // ── Configure SEC2 Falcon DMA/cache (ga102_flcn_fw_load) ──
        con.println("  GSP: [HS-BOOT] Configuring SEC2 DMA/cache...");
        let irqmset = self.bar0.read32(NV_PSEC2_FALCON_IRQMSET);
        self.bar0.write32(NV_PSEC2_FALCON_IRQMSET, irqmset | 0x80);
        self.bar0.write32(NV_PSEC2_FALCON_ENGCTL, 0x0);
        self.bar0.write32(NV_PSEC2_FALCON_DMACTL, (1 << 2) | 1);

        // ── DMA IMEM: os_code from data section → SEC2 IMEM ──
        let imem_src_off = data_base + hs.os_code_offset as usize;
        let imem_size = hs.os_code_size as usize;
        if imem_src_off + imem_size > booter.len() || imem_size == 0 {
            con.println("  GSP: [HS-BOOT] ERROR — IMEM region invalid");
            return Err(GspLoadError::NoBooterFound);
        }
        let imem_chunks = (imem_size + 255) / 256;
        con.print("  GSP: [HS-BOOT] DMA IMEM: off=0x");
        con.print_hex32(imem_src_off as u32);
        con.print(" sz=0x");
        con.print_hex32(imem_size as u32);
        con.print(" (");
        con.print_hex32(imem_chunks as u32);
        con.println(" chunks)");

        let imem_phys = booter.as_ptr() as u64 + imem_src_off as u64;
        for i in 0..imem_chunks {
            self.sec2_dma_xfer_256(
                imem_phys + (i * 256) as u64,
                (i * 256) as u32,
                true, // IMEM
            )?;
        }
        con.print_colored("  GSP: [HS-BOOT] IMEM loaded OK\n", 0x00FF00);

        // ── Prepare DMEM bounce buffer with patching ──
        let dmem_src_off = data_base + hs.os_data_offset as usize;
        let dmem_size = hs.os_data_size as usize;
        if dmem_src_off + dmem_size > booter.len() || dmem_size == 0 {
            con.println("  GSP: [HS-BOOT] ERROR — DMEM region invalid");
            return Err(GspLoadError::NoBooterFound);
        }

        // Allocate bounce buffer (page-aligned) for DMEM patching
        let dmem_pages = (dmem_size + PAGE_SIZE - 1) / PAGE_SIZE;
        let dmem_buf_phys = unsafe {
            crate::arch::page_alloc::alloc_pages_contiguous(dmem_pages)
        }.ok_or(GspLoadError::PageAllocFailed)?;

        unsafe {
            core::ptr::write_bytes(dmem_buf_phys as *mut u8, 0, dmem_pages * PAGE_SIZE);
            core::ptr::copy_nonoverlapping(
                booter.as_ptr().add(dmem_src_off),
                dmem_buf_phys as *mut u8,
                dmem_size,
            );
        }

        // Patch WPR meta PA at patch_loc (within DMEM)
        let patch_off = hs.dmem_sign as usize;
        if patch_off + 4 <= dmem_size {
            let wpr_lo = (wpr_meta_phys & 0xFFFF_FFFF) as u32;
            unsafe {
                let dst = (dmem_buf_phys as *mut u8).add(patch_off) as *mut u32;
                core::ptr::write(dst, wpr_lo);
            }
            con.print("  GSP: [HS-BOOT] Patched DMEM+0x");
            con.print_hex32(patch_off as u32);
            con.print(" = WPR meta PA lo 0x");
            con.print_hex32(wpr_lo);
            con.newline();
        } else {
            con.print_colored("  GSP: [HS-BOOT] WARNING — patch_loc outside DMEM!\n", 0xFFFF00);
        }

        // Patch signature index at patch_sig (within DMEM)
        // patch_sig_value is a signature index (e.g., 0 for prod), NOT a DMEM offset.
        // No patching needed in DMEM for signature index.

        // ── DMA DMEM → SEC2 DMEM ──
        let dmem_chunks = (dmem_size + 255) / 256;
        con.print("  GSP: [HS-BOOT] DMA DMEM: sz=0x");
        con.print_hex32(dmem_size as u32);
        con.print(" (");
        con.print_hex32(dmem_chunks as u32);
        con.println(" chunks)");

        for i in 0..dmem_chunks {
            self.sec2_dma_xfer_256(
                dmem_buf_phys + (i * 256) as u64,
                (i * 256) as u32,
                false, // DMEM
            )?;
        }
        con.print_colored("  GSP: [HS-BOOT] DMEM loaded OK\n", 0x00FF00);

        // ── Program HS registers (ga102_flcn_fw_boot style) ──
        con.println("  GSP: [HS-BOOT] Programming SEC2 HS registers...");
        self.bar0.write32(NV_PSEC2_FALCON_DMEM_SIGN, hs.dmem_sign);
        self.bar0.write32(NV_PSEC2_FALCON_ENGINE_ID, hs.engine_id);
        self.bar0.write32(NV_PSEC2_FALCON_UCODE_ID, hs.ucode_id);
        self.bar0.write32(NV_PSEC2_FALCON_EMEM_ACCESS, 0x1);

        con.print("  GSP: [HS-BOOT] dmem_sign=0x");
        con.print_hex32(hs.dmem_sign);
        con.print(" engine_id=0x");
        con.print_hex32(hs.engine_id);
        con.print(" ucode_id=0x");
        con.print_hex32(hs.ucode_id);
        con.newline();

        // ── Write SEC2 MAILBOX with WPR meta PA ──
        self.bar0.write32(NV_PSEC2_FALCON_MAILBOX0, (wpr_meta_phys & 0xFFFF_FFFF) as u32);
        self.bar0.write32(NV_PSEC2_FALCON_MAILBOX1, ((wpr_meta_phys >> 32) & 0xFFFF_FFFF) as u32);

        // ── Boot SEC2 Falcon ──
        con.println("  GSP: [HS-BOOT] Starting SEC2 Falcon (booter_load HS)...");
        self.bar0.write32(NV_PSEC2_FALCON_BOOTVEC, 0x0);
        self.bar0.write32(NV_PSEC2_FALCON_CPUCTL, FALCON_CPUCTL_STARTCPU);

        // ── Wait for completion ──
        self.sec2_boot_and_wait(con)
    }

    // ── Reset SEC2 Falcon to a known state ──
    fn reset_sec2_falcon(&self, con: &mut Console) {
        con.println("  GSP: Resetting SEC2 Falcon...");

        self.bar0.write32(NV_PSEC2_FALCON_RESET, 0x1);
        for _ in 0..50_000u32 { core::hint::spin_loop(); }
        self.bar0.write32(NV_PSEC2_FALCON_RESET, 0x0);
        for _ in 0..200_000u32 { core::hint::spin_loop(); }

        let engine = self.bar0.read32(NV_PSEC2_FALCON_ENGINE);
        con.print("  GSP: SEC2 FALCON_ENGINE=0x");
        con.print_hex32(engine);
        con.newline();
    }

    // ── Reset PGSP Falcon to a known state ──
    fn reset_pgsp_falcon(&self, con: &mut Console) {
        con.println("  GSP: Resetting PGSP Falcon...");

        self.bar0.write32(NV_PGSP_FALCON_RESET, 0x1);
        for _ in 0..100_000u32 { core::hint::spin_loop(); }
        self.bar0.write32(NV_PGSP_FALCON_RESET, 0x0);
        for _ in 0..200_000u32 { core::hint::spin_loop(); }
    }

    // ── Reset GSP into RISC-V mode (from nouveau ga102_gsp_reset) ──
    fn reset_gsp_riscv_mode(&self, con: &mut Console) {
        con.println("  GSP: Resetting GSP Falcon + switching to RISC-V mode...");

        // Reset GSP engine
        self.bar0.write32(NV_PGSP_FALCON_RESET, 0x1);
        for _ in 0..100_000u32 { core::hint::spin_loop(); }
        self.bar0.write32(NV_PGSP_FALCON_RESET, 0x0);
        for _ in 0..200_000u32 { core::hint::spin_loop(); }

        // Switch GSP to RISC-V execution mode (register 0x00111668)
        // This tells the hardware that the GSP core should run RISC-V code
        // instead of classic Falcon microcode
        let mode_before = self.bar0.read32(NV_PGSP_RISCV_MODE);
        let new_mode = (mode_before & !NV_PGSP_RISCV_MODE_MASK) | NV_PGSP_RISCV_MODE_MASK;
        self.bar0.write32(NV_PGSP_RISCV_MODE, new_mode);
        for _ in 0..50_000u32 { core::hint::spin_loop(); }

        let mode_after = self.bar0.read32(NV_PGSP_RISCV_MODE);
        con.print("  GSP: RISCV_MODE: 0x");
        con.print_hex32(mode_before);
        con.print(" → 0x");
        con.print_hex32(mode_after);
        con.newline();
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
        booter_manifest_off: u64,
        sig_phys: u64,
        sig_size: u64,
        con: &mut Console,
    ) -> GspFwWprMeta {
        let fb_size = FB_SIZE_12GB;

        // VGA workspace at top
        let vga_workspace_size: u64 = 0x2_0000; // 128KB
        let vga_workspace_offset = fb_size - vga_workspace_size;

        // WPR2 end (below VGA workspace, 128KB aligned)
        let gsp_fw_wpr_end = align_down(vga_workspace_offset, ALIGN_128K);

        // FRTS (Firmware Runtime Structure) — 1MB on GA106 with VBIOS
        // nouveau uses 0x100000 when VBIOS exists (which it does on discrete GPUs)
        let frts_size: u64 = 0x10_0000; // 1MB (was incorrectly 0x1000!)
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
        con.print("    FRTS: 0x");
        con.print_hex32((frts_offset >> 32) as u32);
        con.print_hex32(frts_offset as u32);
        // For Ampere (GA106), WPR2 is at 0x1FA824/0x1FA828
        const NV_WPR2_LO: u32 = 0x001FA824;
        const NV_WPR2_HI: u32 = 0x001FA828;
        con.print(" size=0x");
        con.print_hex32(frts_size as u32);
        con.println(" (1MB)");

        GspFwWprMeta {
            magic: WPR_META_MAGIC,
            revision: WPR_META_REVISION,
            sysmem_addr_of_radix3_elf: radix3_phys,
            size_of_radix3_elf: fw_size,
            sysmem_addr_of_bootloader: booter_phys,
            size_of_bootloader: booter_size,
            bootloader_code_offset: booter_code_off,
            bootloader_data_offset: booter_data_off,
            bootloader_manifest_offset: booter_manifest_off,
            sysmem_addr_of_signature: sig_phys,
            size_of_signature: sig_size,
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
    // Returns BOTH physical addresses for the two-mailbox handoff:
    //   - boot_args_phys → PGSP MAILBOX (for GSP libos after RISC-V boots)
    //   - wpr_meta_phys  → SEC2 MAILBOX (for booter_load)
    fn prepare_boot_mem(
        &self,
        wpr_meta: &GspFwWprMeta,
        con: &mut Console,
    ) -> Result<BootMem, GspLoadError> {
        let cmdq_pages = CMDQ_SIZE / PAGE_SIZE;
        let msgq_pages = MSGQ_SIZE / PAGE_SIZE;
        let total_pages = 2 + cmdq_pages + msgq_pages; // args + wpr_meta + queues

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

        // Write WPR meta at page 1
        unsafe {
            let dst = wpr_meta_phys as *mut GspFwWprMeta;
            core::ptr::write(dst, core::ptr::read(wpr_meta as *const GspFwWprMeta));
        }

        // Write boot args (GspArgumentsCached) at page 0
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

        Ok(BootMem { boot_args_phys, wpr_meta_phys, shared_mem_phys })
    }

    // ── Find ELF section by name (returns offset+size) ──
    fn find_elf_section(fw: &[u8], name: &[u8]) -> Option<(u64, u64)> {
        if fw.len() < 64 { return None; }
        if fw[4] != 2 { return None; } // ELF64 only

        let shoff = u64::from_le_bytes([
            fw[40], fw[41], fw[42], fw[43], fw[44], fw[45], fw[46], fw[47],
        ]) as usize;
        let shentsize = u16::from_le_bytes([fw[58], fw[59]]) as usize;
        let shnum = u16::from_le_bytes([fw[60], fw[61]]) as usize;
        let shstrndx = u16::from_le_bytes([fw[62], fw[63]]) as usize;

        if shoff == 0 || shentsize < 64 || shnum == 0 { return None; }

        // Get string table offset
        let strtab_sh = shoff + shstrndx * shentsize;
        if strtab_sh + 64 > fw.len() { return None; }
        let strtab_off = u64::from_le_bytes([
            fw[strtab_sh+24], fw[strtab_sh+25], fw[strtab_sh+26], fw[strtab_sh+27],
            fw[strtab_sh+28], fw[strtab_sh+29], fw[strtab_sh+30], fw[strtab_sh+31],
        ]) as usize;

        for i in 0..shnum {
            let off = shoff + i * shentsize;
            if off + 64 > fw.len() { break; }

            let sh_name = u32::from_le_bytes([fw[off], fw[off+1], fw[off+2], fw[off+3]]) as usize;
            let sh_offset = u64::from_le_bytes([
                fw[off+24], fw[off+25], fw[off+26], fw[off+27],
                fw[off+28], fw[off+29], fw[off+30], fw[off+31],
            ]);
            let sh_size = u64::from_le_bytes([
                fw[off+32], fw[off+33], fw[off+34], fw[off+35],
                fw[off+36], fw[off+37], fw[off+38], fw[off+39],
            ]);

            // Compare section name
            let name_off = strtab_off + sh_name;
            if name_off + name.len() <= fw.len() {
                let mut matches = true;
                for j in 0..name.len() {
                    if fw[name_off + j] != name[j] { matches = false; break; }
                }
                // Verify null terminator
                if matches && name_off + name.len() < fw.len() && fw[name_off + name.len()] == 0 {
                    if sh_size > 0 {
                        return Some((sh_offset, sh_size));
                    }
                }
            }
        }
        None
    }

    // ── Boot SEC2 Falcon with booter_load and wait ──
    fn sec2_boot_and_wait(&self, con: &mut Console) -> Result<(), GspLoadError> {
        con.println("  GSP: Booting SEC2 Falcon (booter_load)...");

        // Set boot vector = 0 (booter code starts at beginning of IMEM)
        self.bar0.write32(NV_PSEC2_FALCON_BOOTVEC, 0x0000_0000);
        self.bar0.write32(NV_PSEC2_FALCON_CPUCTL, FALCON_CPUCTL_STARTCPU);

        con.print("  GSP: Waiting for SEC2 booter...");
        let mut completed = false;
        for i in 0..20_000_000u32 {
            let cpuctl = self.bar0.read32(NV_PSEC2_FALCON_CPUCTL);
            let halted = cpuctl & FALCON_CPUCTL_HALTED != 0;

            if halted && i > 1000 {
                con.newline();
                con.print("  GSP: SEC2 Falcon HALTED (cpuctl=0x");
                con.print_hex32(cpuctl);
                con.print(" after ");
                con.print_hex32(i);
                con.println(" loops)");

                let mb0 = self.bar0.read32(NV_PSEC2_FALCON_MAILBOX0);
                let mb1 = self.bar0.read32(NV_PSEC2_FALCON_MAILBOX1);
                con.print("  GSP: SEC2 post-boot MB0=0x");
                con.print_hex32(mb0);
                con.print(" MB1=0x");
                con.print_hex32(mb1);
                con.newline();

                // Check WPR2 status — nonzero means FWSEC/booter set up WPR
                let wpr2_hi = self.bar0.read32(NV_WPR2_HI);
                con.print("  GSP: WPR2_HI=0x");
                con.print_hex32(wpr2_hi);
                con.newline();

                if i > 10_000 {
                    con.print_colored("  GSP: SEC2 booter_load completed!\n", 0x00FF00);
                    completed = true;
                } else if mb0 == 0 {
                    con.print_colored("  GSP: SEC2 halted quickly — may need FWSEC first\n", 0xFFFF00);
                } else {
                    con.print("  GSP: SEC2 booter error code: 0x");
                    con.print_hex32(mb0);
                    con.newline();
                }
                break;
            }

            if i % 4_000_000 == 0 && i > 0 { con.print("."); }
            core::hint::spin_loop();
        }

        if completed { Ok(()) } else { Err(GspLoadError::Sec2BootFailed) }
    }

    /// Run FWSEC-FRTS on PGSP to set up WPR2 before booter_load.
    ///
    /// GA106 RTX 3060 LHR VBIOS offsets (extracted from vbios_rtx3060.rom):
    ///   - Descriptor:      SPI 0x4A410
    ///   - IMEM (code):     57,856 bytes @ SPI 0x4A8BC
    ///   - DMEM (data):      2,048 bytes @ SPI 0x58ABC
    ///   - InterfaceOffset:  0x1C  (within DMEM)
    ///   - EngineIdMask:   0x400 (GSP Falcon)
    ///   - UcodeId:        9
    ///
    /// The old code loaded the *entire* 2 MiB VBIOS into SEC2 IMEM which
    /// overflowed Falcon IMEM and left DMEM untouched.  This version loads
    /// only the exact IMEM/DMEM slices and patches the runtime interface
    /// and correctly runs it on PGSP Falcon.
    fn fwsec_frts(&self, vbios: &[u8], frts_offset: u64, con: &mut Console) -> Result<(), GspLoadError> {
        const FWSEC_IMEM_OFF: usize = 0x4A8BC;
        const FWSEC_IMEM_SIZE: usize = 57856;
        const FWSEC_DMEM_OFF: usize = 0x58ABC;
        const FWSEC_DMEM_SIZE: usize = 2048;

        if FWSEC_IMEM_OFF + FWSEC_IMEM_SIZE > vbios.len() {
            con.println("  GSP: [FWSEC] ERROR — IMEM extends past VBIOS");
            return Err(GspLoadError::NoBooterFound);
        }
        if FWSEC_DMEM_OFF + FWSEC_DMEM_SIZE > vbios.len() {
            con.println("  GSP: [FWSEC] ERROR — DMEM extends past VBIOS");
            return Err(GspLoadError::NoBooterFound);
        }

        con.println("  GSP: [FWSEC] Loading FWSEC-FRTS from VBIOS...");
        con.print("  GSP: [FWSEC] VBIOS=0x");
        con.print_hex32(vbios.len() as u32);
        con.print(" IMEM@0x");
        con.print_hex32(FWSEC_IMEM_OFF as u32);
        con.print("(+0x");
        con.print_hex32(FWSEC_IMEM_SIZE as u32);
        con.print(") DMEM@0x");
        con.print_hex32(FWSEC_DMEM_OFF as u32);
        con.print("(+0x");
        con.print_hex32(FWSEC_DMEM_SIZE as u32);
        con.println(")");

        // ── Reset PGSP clean ──
        self.reset_pgsp_falcon(con);

        // ── 1. DMA IMEM → PGSP Falcon IMEM ──
        let imem_src = vbios.as_ptr() as u64 + FWSEC_IMEM_OFF as u64;
        let imem_chunks = FWSEC_IMEM_SIZE / 256; // 57856 / 256 = 226 exactly
        con.print("  GSP: [FWSEC] DMA IMEM → PGSP IMEM (");
        con.print_hex32(imem_chunks as u32);
        con.println(" chunks)");
        for i in 0..imem_chunks {
            let offset = i * 256;
            self.pgsp_dma_xfer_256(imem_src + offset as u64, offset as u32, true)?;
        }
        con.println("  GSP: [FWSEC] IMEM loaded OK");

        // ── 2. Copy DMEM to stack buffer + patch runtime interface ──
        // Patch FWSEC DMEM to execute FRTS initialization (like nouveau's s_prepareForFwsec_TU102)
        // Set init_cmd = 0x15 (FALCON_APPLICATION_INTERFACE_DMEM_MAPPER_V3_CMD_FRTS)
        // in DMEM_MAPPER_V3 at 0x560 + 0x2C
        let mut dmem_buf = [0u8; FWSEC_DMEM_SIZE];
        unsafe {
            core::ptr::copy_nonoverlapping(
                vbios.as_ptr().add(FWSEC_DMEM_OFF),
                dmem_buf.as_mut_ptr(),
                FWSEC_DMEM_SIZE,
            );
        }

        dmem_buf[0x58C] = 0x15;
        dmem_buf[0x58D] = 0x00;
        dmem_buf[0x58E] = 0x00;
        dmem_buf[0x58F] = 0x00;

        // Write FWSECLIC_FRTS_CMD struct at cmd_in_buffer_offset (0x7C0)
        let cmd_off = 0x7C0;
        let frts_4k = (frts_offset >> 12) as u32;

        // FWSECLIC_READ_VBIOS_DESC
        dmem_buf[cmd_off + 0..cmd_off + 4].copy_from_slice(&1u32.to_le_bytes()); // version = 1
        dmem_buf[cmd_off + 4..cmd_off + 8].copy_from_slice(&24u32.to_le_bytes()); // size = 24
        dmem_buf[cmd_off + 8..cmd_off + 16].copy_from_slice(&0u64.to_le_bytes()); // gfwImageOffset = 0
        dmem_buf[cmd_off + 16..cmd_off + 20].copy_from_slice(&0u32.to_le_bytes()); // gfwImageSize = 0
        dmem_buf[cmd_off + 20..cmd_off + 24].copy_from_slice(&2u32.to_le_bytes()); // flags = 2

        // FWSECLIC_FRTS_REGION_DESC
        dmem_buf[cmd_off + 24..cmd_off + 28].copy_from_slice(&1u32.to_le_bytes()); // version = 1
        dmem_buf[cmd_off + 28..cmd_off + 32].copy_from_slice(&20u32.to_le_bytes()); // size = 20
        dmem_buf[cmd_off + 32..cmd_off + 36].copy_from_slice(&frts_4k.to_le_bytes()); // frtsRegionOffset4K
        dmem_buf[cmd_off + 36..cmd_off + 40].copy_from_slice(&0x100u32.to_le_bytes()); // frtsRegionSize = 0x100 (1MB)
        dmem_buf[cmd_off + 40..cmd_off + 44].copy_from_slice(&2u32.to_le_bytes()); // frtsRegionMediaType = 2 (FB)

        con.print("  GSP: [FWSEC] FRTS interface patched (frts_offset=0x");
        con.print_hex32((frts_offset >> 32) as u32);
        con.print_hex32(frts_offset as u32);
        con.println(")");

        // ── 3. DMA patched DMEM → PGSP Falcon DMEM ──
        let dmem_src = dmem_buf.as_ptr() as u64;
        let dmem_chunks = FWSEC_DMEM_SIZE / 256; // 2048 / 256 = 8 exactly
        con.print("  GSP: [FWSEC] DMA DMEM → PGSP DMEM (");
        con.print_hex32(dmem_chunks as u32);
        con.println(" chunks)");
        for i in 0..dmem_chunks {
            let offset = i * 256;
            self.pgsp_dma_xfer_256(dmem_src + offset as u64, offset as u32, false)?;
        }
        con.println("  GSP: [FWSEC] DMEM loaded OK");

        // ── 4. Boot PGSP Falcon with FWSEC ucode ──
        con.println("  GSP: [FWSEC] Booting PGSP (FWSEC-FRTS)...");
        self.bar0.write32(NV_PGSP_FALCON_BOOTVEC, 0x0);
        self.bar0.write32(NV_PGSP_FALCON_CPUCTL, FALCON_CPUCTL_STARTCPU);

        // ── 5. Wait for WPR2 to be set by FWSEC ──
        con.println("  GSP: [FWSEC] Waiting for WPR2_HI != 0...");
        for _ in 0..5_000_000u32 {
            let wpr2 = self.bar0.read32(NV_WPR2_HI);
            if wpr2 != 0 && wpr2 != 0xBADF_5720 {
                con.print("  GSP: [FWSEC] WPR2_HI=0x");
                con.print_hex32(wpr2);
                con.print_colored(" (WPR2 SET — FWSEC OK)\n", 0x00FF00);
                return Ok(());
            }
            core::hint::spin_loop();
        }

        let wpr2 = self.bar0.read32(NV_WPR2_HI);
        con.print("  GSP: [FWSEC] timeout WPR2_HI=0x");
        con.print_hex32(wpr2);
        con.println(" — continuing anyway");
        Ok(()) // non-fatal: booter_load will retry
    }

    /// Verify GSP registers after boot attempt
    fn verify_gsp_state(&self, con: &mut Console) {
        let cpuctl = self.bar0.read32(NV_PGSP_FALCON_CPUCTL);
        let idle = self.bar0.read32(NV_PGSP_FALCON_IDLESTATE);
        let mb0 = self.bar0.read32(NV_PGSP_FALCON_MAILBOX0);
        let mb1 = self.bar0.read32(NV_PGSP_FALCON_MAILBOX1);

        con.print("  GSP: PGSP CPUCTL=0x");
        con.print_hex32(cpuctl);
        con.print(" IDLE=0x");
        con.print_hex32(idle);
        con.newline();
        con.print("  GSP: PGSP MB0=0x");
        con.print_hex32(mb0);
        con.print(" MB1=0x");
        con.print_hex32(mb1);
        con.newline();

        // Read RISC-V registers — these should be set by booter_load
        let riscv_cpuctl = self.bar0.read32(super::rpc::NV_PGSP_RISCV_CPUCTL);
        let riscv_br = self.bar0.read32(super::rpc::NV_PGSP_RISCV_BR_ADDR);
        con.print("  GSP: RISCV_CPUCTL=0x");
        con.print_hex32(riscv_cpuctl);
        con.print(" RISCV_BR_ADDR=0x");
        con.print_hex32(riscv_br);
        con.newline();

        // Read WPR2 to see if it was programmed
        let wpr2_hi = self.bar0.read32(NV_WPR2_HI);
        con.print("  GSP: WPR2_HI=0x");
        con.print_hex32(wpr2_hi);
        if wpr2_hi != 0 && wpr2_hi != 0xBADF_5720 {
            con.print_colored(" (WPR2 SET — good!)\n", 0x00FF00);
        } else {
            con.print_colored(" (WPR2 not set)\n", 0xFFFF00);
        }

        // Read RISCV mode register
        let riscv_mode = self.bar0.read32(NV_PGSP_RISCV_MODE);
        con.print("  GSP: RISCV_MODE=0x");
        con.print_hex32(riscv_mode);
        con.newline();

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

    // ════════════════════════════════════════════════════════════════
    // PUBLIC: Single-blob GSP load (gsp_ga10x.bin contains everything)
    // ════════════════════════════════════════════════════════════════
    pub fn load(&self, fw_blob: &[u8], con: &mut Console) -> Result<(), GspLoadError> {
        con.print_colored("=== GSP Firmware Load (GA106 — SEC2 Authenticated Boot) ===\n", 0x00FFFF);

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
        con.println("  GSP: [1/11] PRIV Ring + Engine Enable...");
        self.init_priv_ring(con)?;

        // ── 2. Parse ELF — extract booter from .fwimage ──
        con.println("  GSP: [2/11] Parsing firmware ELF...");
        let fw_info = super::elf_parser::parse_firmware(fw_blob, con)
            .ok_or(GspLoadError::BadElfMagic)?;

        if fw_info.booter_data_size == 0 || fw_info.booter_data_offset == 0 {
            con.println("  GSP: ERROR - no nvfw_bin_hdr found in .fwimage");
            return Err(GspLoadError::NoBooterFound);
        }

        let booter_off = fw_info.booter_data_offset as usize;
        let booter_sz = fw_info.booter_data_size as usize;
        con.print("  GSP: Booter: offset=0x");
        con.print_hex32(booter_off as u32);
        con.print(" size=0x");
        con.print_hex32(booter_sz as u32);
        con.newline();

        // ── 3. Find .fwsignature_ga10x section for authenticated boot ──
        con.println("  GSP: [3/11] Looking for .fwsignature_ga10x...");
        let (sig_phys, sig_size) = match Self::find_elf_section(fw_blob, b".fwsignature_ga10x") {
            Some((off, sz)) => {
                let phys = fw_blob.as_ptr() as u64 + off;
                con.print("  GSP: Signature found: off=0x");
                con.print_hex32(off as u32);
                con.print(" size=0x");
                con.print_hex32(sz as u32);
                con.newline();
                (phys, sz)
            }
            None => {
                con.print_colored("  GSP: WARNING - .fwsignature_ga10x not found\n", 0xFFFF00);
                con.println("  GSP: Authenticated boot may fail without signature");
                (0u64, 0u64)
            }
        };

        // ── 4. Build Radix3 page table for full ELF ──
        con.println("  GSP: [4/11] Building Radix3 page table for ELF...");
        let fw_phys = fw_blob.as_ptr() as u64;
        let radix3 = super::radix3::Radix3PageTable::build(fw_phys, fw_blob.len(), con)
            .ok_or(GspLoadError::Radix3Failed)?;

        con.print("  GSP: Radix3 root=0x");
        con.print_hex32(radix3.root_phys() as u32);
        con.newline();

        // ── 5. Build GspFwWprMeta (VRAM layout) ──
        con.println("  GSP: [5/11] Building GspFwWprMeta (VRAM layout)...");
        let booter_phys = fw_phys + booter_off as u64;
        let wpr_meta = self.build_wpr_meta(
            radix3.root_phys(),
            fw_blob.len() as u64,
            booter_phys,
            booter_sz as u64,
            fw_info.booter_header_offset as u64,
            0,
            0, // manifest offset (TODO: extract from HS header)
            sig_phys,
            sig_size,
            con,
        );

        // ── 6. Prepare boot memory (boot args + WPR meta + queues) ──
        con.println("  GSP: [6/11] Preparing boot memory...");
        let boot_mem = self.prepare_boot_mem(&wpr_meta, con)?;

        // ── 7. Reset GSP into RISC-V mode ──
        con.println("  GSP: [7/11] Resetting GSP into RISC-V mode...");
        self.reset_gsp_riscv_mode(con);

        // ── 8. Write GspArgumentsCached PA to PGSP MAILBOX0/1 ──
        // (GSP libos reads this AFTER RISC-V boots)
        con.println("  GSP: [8/11] Writing PGSP MAILBOX0/1 (libos args)...");
        self.bar0.write32(NV_PGSP_FALCON_MAILBOX0, (boot_mem.boot_args_phys & 0xFFFF_FFFF) as u32);
        self.bar0.write32(NV_PGSP_FALCON_MAILBOX1, ((boot_mem.boot_args_phys >> 32) & 0xFFFF_FFFF) as u32);
        let mb0 = self.bar0.read32(NV_PGSP_FALCON_MAILBOX0);
        let mb1 = self.bar0.read32(NV_PGSP_FALCON_MAILBOX1);
        con.print("  GSP: PGSP MB0=0x");
        con.print_hex32(mb0);
        con.print(" MB1=0x");
        con.print_hex32(mb1);
        con.newline();

        // ── 9. Reset SEC2 + Write WPR meta PA to SEC2 MAILBOX0/1 ──
        con.println("  GSP: [9/11] Resetting SEC2 + writing SEC2 MAILBOX...");
        self.reset_sec2_falcon(con);
        self.bar0.write32(NV_PSEC2_FALCON_MAILBOX0, (boot_mem.wpr_meta_phys & 0xFFFF_FFFF) as u32);
        self.bar0.write32(NV_PSEC2_FALCON_MAILBOX1, ((boot_mem.wpr_meta_phys >> 32) & 0xFFFF_FFFF) as u32);
        let sec2_mb0 = self.bar0.read32(NV_PSEC2_FALCON_MAILBOX0);
        let sec2_mb1 = self.bar0.read32(NV_PSEC2_FALCON_MAILBOX1);
        con.print("  GSP: SEC2 MB0=0x");
        con.print_hex32(sec2_mb0);
        con.print(" SEC2 MB1=0x");
        con.print_hex32(sec2_mb1);
        con.newline();

        // ── 10. DMA booter_load to SEC2 IMEM + boot SEC2 ──
        con.println("  GSP: [10/11] DMA booter_load to SEC2 IMEM + boot...");
        self.sec2_dma_load_booter(fw_blob, booter_off, booter_sz, con)?;

        // Boot SEC2 and wait for it to complete
        let sec2_ok = self.sec2_boot_and_wait(con);

        // ── 11. Verify GSP state ──
        con.println("  GSP: [11/11] Verifying GSP state...");
        self.verify_gsp_state(con);

        match sec2_ok {
            Ok(()) => {
                con.print_colored("=== GSP Boot via SEC2 COMPLETE ===\n", 0x00FF00);
                con.println("  GSP: SEC2 booter_load ran → should have set up WPR2 + started RISC-V");
                con.println("  GSP: Next: poll message queue for GSP_INIT_DONE (0x1001)");
            }
            Err(_) => {
                con.print_colored("=== GSP SEC2 Boot DID NOT COMPLETE ===\n", 0xFF4444);
                con.println("  GSP: Possible causes:");
                con.println("    1. FWSEC-FRTS must run before booter_load (VBIOS stage)");
                con.println("    2. booter_load needs HS manifest (nvfw_hs_header_v2)");
                con.println("    3. Firmware version mismatch (gsp_ga10x.bin vs booter)");
                con.println("    4. .fwsignature_ga10x not correctly passed");
            }
        }

        Ok(())
    }

    // ════════════════════════════════════════════════════════════════
    // PUBLIC: Full GA10x boot with 3 separate firmware blobs
    // ════════════════════════════════════════════════════════════════
    pub fn load_full(&self, blobs: &super::GspFirmwareBlobs, con: &mut Console) -> Result<(), GspLoadError> {
        con.print_colored("=== GSP Firmware Load (GA106 - SEC2 3-Blob Boot) ===\n", 0x00FFFF);

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

        // ── Find .fwsignature_ga10x in GSP-RM ELF ──
        let (sig_phys, sig_size) = match Self::find_elf_section(gsp_rm, b".fwsignature_ga10x") {
            Some((off, sz)) => {
                let phys = gsp_rm.as_ptr() as u64 + off;
                con.print("  GSP: Signature: off=0x");
                con.print_hex32(off as u32);
                con.print(" size=0x");
                con.print_hex32(sz as u32);
                con.newline();
                (phys, sz)
            }
            None => {
                con.print_colored("  GSP: WARNING - .fwsignature_ga10x not found\n", 0xFFFF00);
                (0u64, 0u64)
            }
        };

        // ── 1. PRIV Ring init ──
        con.println("  GSP: [1/11] PRIV Ring + Engine Enable...");
        self.init_priv_ring(con)?;

        // ── 2. Build Radix3 page table for GSP-RM ELF ──
        con.println("  GSP: [2/11] Building Radix3 page table for GSP-RM ELF...");
        let gsp_phys = gsp_rm.as_ptr() as u64;
        let radix3 = super::radix3::Radix3PageTable::build(gsp_phys, gsp_rm.len(), con)
            .ok_or(GspLoadError::Radix3Failed)?;
        con.print("  GSP: Radix3 root=0x");
        con.print_hex32(radix3.root_phys() as u32);
        con.newline();

        // ── 3. Build GspFwWprMeta (VRAM layout) ──
        con.println("  GSP: [3/11] Building GspFwWprMeta...");
        let bootloader_phys = bootloader.as_ptr() as u64;
        let wpr_meta = self.build_wpr_meta(
            radix3.root_phys(),
            gsp_rm.len() as u64,
            bootloader_phys,
            bootloader.len() as u64,
            bl_data_off as u64,
            0,
            0, // manifest offset
            sig_phys,
            sig_size,
            con,
        );

        // ── 4. Prepare boot memory ──
        con.println("  GSP: [4/11] Preparing boot memory...");
        let boot_mem = self.prepare_boot_mem(&wpr_meta, con)?;

        // ── 5. Run FWSEC-FRTS on PGSP to set up WPR2 ──
        con.println("  GSP: [5/11] Running FWSEC-FRTS on PGSP...");
        if let Some(vbios) = blobs.vbios_rom {
            self.fwsec_frts(vbios, wpr_meta.frts_offset, con)?;
        }

        // Read FWSEC FRTS Error Code from Scratch 14 (0x1438)
        let scratch_14 = self.bar0.read32(0x00001438);
        let frts_err_code = (scratch_14 >> 16) & 0xFFFF;
        if frts_err_code != 0 {
            con.print("  GSP: [FWSEC ERROR] FRTS error code = 0x");
            con.print_hex32(frts_err_code);
            con.newline();
        }

        // ── 6. Verify WPR2_HI != 0 ──
        con.println("  GSP: [6/11] Checking WPR2_HI...");
        const NV_WPR2_HI: u32 = 0x001FA828;
        let wpr2 = self.bar0.read32(NV_WPR2_HI);
        if wpr2 != 0 && wpr2 != 0xBADF_5720 {
            con.print_colored("  GSP: WPR2 SET OK!\n", 0x00FF00);
        } else {
            con.print_colored("  GSP: WARNING — WPR2 not set by FWSEC!\n", 0xFFFF00);
        }

        // ── 7. Reset GSP into RISC-V mode ──
        con.println("  GSP: [7/11] Resetting GSP into RISC-V mode...");
        self.reset_gsp_riscv_mode(con);

        // ── 8. Write libos args to PGSP MAILBOX0/1 ──
        con.println("  GSP: [8/11] Writing PGSP MAILBOX0/1 (libos args)...");
        self.bar0.write32(NV_PGSP_FALCON_MAILBOX0, (boot_mem.boot_args_phys & 0xFFFF_FFFF) as u32);
        self.bar0.write32(NV_PGSP_FALCON_MAILBOX1, ((boot_mem.boot_args_phys >> 32) & 0xFFFF_FFFF) as u32);
        let mb0 = self.bar0.read32(NV_PGSP_FALCON_MAILBOX0);
        let mb1 = self.bar0.read32(NV_PGSP_FALCON_MAILBOX1);
        con.print("  GSP: PGSP MB0=0x");
        con.print_hex32(mb0);
        con.print(" MB1=0x");
        con.print_hex32(mb1);
        con.newline();

        // ── 9-10. HS-authenticated boot of booter_load on SEC2 ──
        //    Parses nvfw_hs_header_v2 + nvfw_hs_load_header_v2,
        //    splits IMEM/DMEM DMA, patches WPR meta PA at patch_loc,
        //    programs HS registers (dmem_sign, engine_id, ucode_id)
        con.println("  GSP: [9-10/11] HS booter_load on SEC2...");
        let sec2_ok = self.sec2_hs_boot_booter(booter, boot_mem.wpr_meta_phys, con);

        // ── 11. Verify GSP state ──
        con.println("  GSP: [11/11] Verifying GSP state...");
        self.verify_gsp_state(con);

        // ── Report result ──
        match sec2_ok {
            Ok(()) => {
                con.print_colored("=== GSP Boot via SEC2 COMPLETE ===\n", 0x00FF00);
                con.println("  GSP: Next: poll message queue for GSP_INIT_DONE (0x1001)");
            }
            Err(_) => {
                con.print_colored("=== GSP SEC2 Boot DID NOT COMPLETE ===\n", 0xFF4444);
                con.println("  GSP: Check WPR2_HI above — if still 0x0:");
                con.println("    1. FWSEC-FRTS may not have set WPR2 correctly");
                con.println("    2. HS manifest patch_loc/patch_sig values may be wrong");
                con.println("    3. Firmware version mismatch (booter vs bootloader)");
            }
        }

        Ok(())
    }
}
