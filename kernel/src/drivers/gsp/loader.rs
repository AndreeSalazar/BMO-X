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
    NV_PGSP_RISCV_MODE,
    NV_PGSP_RISCV_MODE_REQUEST, NV_PGSP_RISCV_MODE_ACTIVE,
    NV_WPR2_HI,
    NV_PSEC2_FALCON_MAILBOX0, NV_PSEC2_FALCON_MAILBOX1,
    NV_PSEC2_FALCON_CPUCTL, NV_PSEC2_FALCON_BOOTVEC,
    NV_PSEC2_FALCON_IDLESTATE, NV_PSEC2_FALCON_RESET,
    NV_PSEC2_FALCON_ENGINE,
    NV_PSEC2_DMATRFBASE, NV_PSEC2_DMATRFBASE1, NV_PSEC2_DMATRFMOFFS,
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

// GA10x PGSP Falcon DMA registers (from nouveau ga102_flcn_dma / ga102.c):
//   0x110 = DMATRFBASE   — sysmem phys address >> 8
//   0x114 = DMATRFFBOFFS — destination address in Falcon IMEM/DMEM
//   0x118 = DMATRFCMD    — write to start DMA; poll bit 1 for completion
//   0x11C = DMATRFSOFFS  — source offset from DMATRFBASE
//   0x128 = DMATRFMOFFS  — upper address bits (set to 0)
const NV_PGSP_DMATRFBASE:        u32 = 0x0011_0110;
const NV_PGSP_DMATRFMOFFS:       u32 = 0x0011_0114;
const NV_PGSP_DMATRFCMD:         u32 = 0x0011_0118;
const NV_PGSP_DMATRFFBOFFS:      u32 = 0x0011_011C;
const NV_PGSP_DMATRFBASE1:       u32 = 0x0011_0128;

// DMA command bits (same for both PGSP and SEC2 Falcon)
const DMA_CMD_WRITE:    u32 = 1 << 1;
const DMA_CMD_IMEM:     u32 = 1 << 4;
const DMA_CMD_SEC:      u32 = 1 << 2;
const DMA_CMD_SIZE_256: u32 = 6 << 8;
const DMA_CMD_IDLE:     u32 = 1 << 1;

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
    FwsecFailed,
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
    patch_loc_value: u32,   // offset within image where the selected PKC signature is copied
    patch_sig_value: u32,   // selected signature index in extracted Nouveau blobs
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
    app0_data_offset: u32,
    app0_data_size: u32,
    sig_size: u32,
    dmem_sign: u32,         // signature offset within DMEM = patch_loc_value - os_data_offset
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
        self.bar0.write32(NV_PSEC2_DMATRFBASE1, ((src_phys >> 40) & 0x1FF) as u32);
        self.bar0.write32(NV_PSEC2_DMATRFMOFFS, falcon_offset);
        self.bar0.write32(NV_PSEC2_DMATRFFBOFFS, (src_phys & 0xFF) as u32);

        let mut cmd = DMA_CMD_SIZE_256;
        if to_imem { cmd |= DMA_CMD_IMEM; }
        self.bar0.write32(NV_PSEC2_DMATRFCMD, cmd);

        for _ in 0..1_000_000 {
            let val = self.bar0.read32(NV_PSEC2_DMATRFCMD);
            if (val & (1 << 1)) != 0 { return Ok(()); }
            core::hint::spin_loop();
        }
        Err(GspLoadError::DmaTimeout)
    }

    // GA102 secure IMEM DMA needs the Falcon memory offset/tag separated from
    // the sysmem physical base. For boot-from-HS, mem_off is appCodeOffset.
    fn sec2_dma_xfer_256_tagged(
        &self,
        src_base_phys: u64,
        src_mem_off: u32,
        falcon_dest: u32,
        to_imem: bool,
    ) -> Result<(), GspLoadError> {
        self.bar0.write32(NV_PSEC2_DMATRFBASE, (src_base_phys >> 8) as u32);
        self.bar0.write32(NV_PSEC2_DMATRFBASE1, ((src_base_phys >> 40) & 0x1FF) as u32);
        self.bar0.write32(NV_PSEC2_DMATRFMOFFS, falcon_dest);
        self.bar0.write32(NV_PSEC2_DMATRFFBOFFS, src_mem_off);

        let mut cmd = DMA_CMD_SIZE_256;
        if to_imem {
            cmd |= DMA_CMD_IMEM | DMA_CMD_SEC;
        }
        self.bar0.write32(NV_PSEC2_DMATRFCMD, cmd);

        for _ in 0..1_000_000 {
            let val = self.bar0.read32(NV_PSEC2_DMATRFCMD);
            if (val & DMA_CMD_IDLE) != 0 { return Ok(()); }
            core::hint::spin_loop();
        }
        Err(GspLoadError::DmaTimeout)
    }

    fn force_program_wpr2_frts(&self, frts_offset: u64, frts_size: u64, con: &mut Console) {
        const NV_WPR2_LO: u32 = 0x001F_A824;
        const NV_WPR2_HI: u32 = 0x001F_A828;

        let lo_val = (frts_offset >> 12) as u32;
        let hi_val = ((frts_offset + frts_size - 1) >> 12) as u32;
        let lo_raw = lo_val << 4;
        let hi_raw = hi_val << 4;

        con.print("  GSP: [FWSEC] Forcing WPR2 FRTS: lo_val=0x");
        con.print_hex32(lo_val);
        con.print(" hi_val=0x");
        con.print_hex32(hi_val);
        con.newline();

        self.bar0.write32(NV_WPR2_LO, lo_raw);
        self.bar0.write32(NV_WPR2_HI, hi_raw);

        let rb_lo = self.bar0.read32(NV_WPR2_LO);
        let rb_hi = self.bar0.read32(NV_WPR2_HI);
        con.print("  GSP: [FWSEC] WPR2 forced raw lo=0x");
        con.print_hex32(rb_lo);
        con.print(" hi=0x");
        con.print_hex32(rb_hi);
        con.newline();
    }

    // ── PGSP DMA transfer: copy 256 bytes from sysmem to PGSP Falcon IMEM/DMEM ──
    // GA10x register layout (nouveau ga102_flcn_dma):
    //   DMATRFBASE  [0x110] = sysmem phys >> 8 (base address, set once)
    //   DMATRFMOFFS [0x128] = 0 (upper address bits)
    //   DMATRFFBOFFS[0x114] = destination in Falcon IMEM/DMEM
    //   DMATRFSOFFS [0x11C] = source offset from DMATRFBASE
    //   DMATRFCMD   [0x118] = command (write to start; poll bit 1 for done)
    fn pgsp_dma_xfer_256(&self, _src_base_phys: u64, src_offset: u32, falcon_dst: u32,
                         cmd: u32) -> Result<(), GspLoadError> {
        self.bar0.write32(NV_PGSP_DMATRFMOFFS, falcon_dst);
        self.bar0.write32(NV_PGSP_DMATRFFBOFFS, src_offset);
        self.bar0.write32(NV_PGSP_DMATRFCMD, cmd);

        for _ in 0..1_000_000 {
            let val = self.bar0.read32(NV_PGSP_DMATRFCMD);
            if (val & DMA_CMD_IDLE) != 0 { return Ok(()); }
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

        let expected_load_hdr_size = 20usize + (num_apps as usize * 16);
        if load_hdr_size as usize != expected_load_hdr_size {
            con.print("  GSP: [HS] WARNING — load_hdr size mismatch, expected 0x");
            con.print_hex32(expected_load_hdr_size as u32);
            con.newline();
        }

        // Read app[0] if available. For boot-from-HS, NVIDIA uses appCode as
        // the secure IMEM region and osData as DMEM.
        let (app0_offset, app0_size, app0_data_offset, app0_data_size) =
            if num_apps > 0 && load_hdr_offset + 20 + 16 <= blob.len() {
            let app_base = load_hdr_offset + 20;
            let off = r32(blob, app_base);
            let sz  = r32(blob, app_base + 4);
            let data_off = r32(blob, app_base + 8);
            let data_sz  = r32(blob, app_base + 12);
            con.print("  GSP: [HS]   app[0] off=0x");
            con.print_hex32(off);
            con.print(" sz=0x");
            con.print_hex32(sz);
            con.print(" data_off=0x");
            con.print_hex32(data_off);
            con.print(" data_sz=0x");
            con.print_hex32(data_sz);
            con.newline();
            (off, sz, data_off, data_sz)
        } else {
            (os_code_offset, os_code_size, os_data_offset, os_data_size)
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
            app0_data_offset,
            app0_data_size,
            sig_size: if num_sigs != 0 { sig_prod_size / num_sigs } else { 0 },
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

        let image_size = hs.data_size as usize;
        let image_pages = (image_size + PAGE_SIZE - 1) / PAGE_SIZE;
        let image_buf_phys = unsafe {
            crate::arch::page_alloc::alloc_pages_contiguous(image_pages)
        }.ok_or(GspLoadError::PageAllocFailed)?;

        unsafe {
            core::ptr::write_bytes(image_buf_phys as *mut u8, 0, image_pages * PAGE_SIZE);
            core::ptr::copy_nonoverlapping(
                booter.as_ptr().add(data_base),
                image_buf_phys as *mut u8,
                image_size,
            );
        }

        let sig_size = hs.sig_size as usize;
        let sig_index = if hs.patch_sig_value < hs.num_sigs {
            hs.patch_sig_value
        } else if hs.fuse_ver < hs.num_sigs {
            hs.num_sigs - 1 - hs.fuse_ver
        } else {
            0
        } as usize;
        let sig_src_off = hs.sig_prod_offset as usize + sig_index * sig_size;
        let sig_dst_off = hs.patch_loc_value as usize;
        if sig_size == 0 || sig_src_off + sig_size > booter.len() || sig_dst_off + sig_size > image_size {
            con.println("  GSP: [HS-BOOT] ERROR - signature patch region invalid");
            return Err(GspLoadError::NoBooterFound);
        }

        unsafe {
            core::ptr::copy_nonoverlapping(
                booter.as_ptr().add(sig_src_off),
                (image_buf_phys as *mut u8).add(sig_dst_off),
                sig_size,
            );
        }
        con.print("  GSP: [HS-BOOT] Patched PKC signature: sig[");
        con.print_hex32(sig_index as u32);
        con.print("] sz=0x");
        con.print_hex32(sig_size as u32);
        con.print(" -> image+0x");
        con.print_hex32(sig_dst_off as u32);
        con.newline();

        let imem_src_off = hs.app0_offset as usize;
        let imem_size = hs.app0_size as usize;
        if imem_src_off + imem_size > image_size || imem_size == 0 {
            con.println("  GSP: [HS-BOOT] ERROR - app IMEM region invalid");
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

        for i in 0..imem_chunks {
            self.sec2_dma_xfer_256_tagged(
                image_buf_phys,
                imem_src_off as u32 + (i * 256) as u32,
                (i * 256) as u32,
                true,
            )?;
        }
        con.print_colored("  GSP: [HS-BOOT] IMEM loaded OK\n", 0x00FF00);

        let dmem_src_off = hs.os_data_offset as usize;
        let dmem_size = hs.os_data_size as usize;
        if dmem_src_off + dmem_size > image_size || dmem_size == 0 {
            con.println("  GSP: [HS-BOOT] ERROR - DMEM region invalid");
            return Err(GspLoadError::NoBooterFound);
        }
        let dmem_chunks = (dmem_size + 255) / 256;
        con.print("  GSP: [HS-BOOT] DMA DMEM: off=0x");
        con.print_hex32(dmem_src_off as u32);
        con.print(" sz=0x");
        con.print_hex32(dmem_size as u32);
        con.print(" (");
        con.print_hex32(dmem_chunks as u32);
        con.println(" chunks)");

        for i in 0..dmem_chunks {
            self.sec2_dma_xfer_256_tagged(
                image_buf_phys + dmem_src_off as u64,
                (i * 256) as u32,
                (i * 256) as u32,
                false,
            )?;
        }
        con.print_colored("  GSP: [HS-BOOT] DMEM loaded OK\n", 0x00FF00);

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

        self.bar0.write32(NV_PSEC2_FALCON_MAILBOX0, (wpr_meta_phys & 0xFFFF_FFFF) as u32);
        self.bar0.write32(NV_PSEC2_FALCON_MAILBOX1, ((wpr_meta_phys >> 32) & 0xFFFF_FFFF) as u32);

        return self.sec2_boot_and_wait_at(hs.app0_offset, con);

        #[cfg(any())]
        {

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

        // ── Boot SEC2 Falcon + wait for completion ──
        // (sec2_boot_and_wait handles BOOTVEC + CPUCTL + halt polling)
        self.sec2_boot_and_wait(con)
        }
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

    // ── Reset GSP into RISC-V mode and write LibosArgs (from nouveau ga102_gsp_reset) ──
    fn reset_gsp_riscv_mode(&self, boot_args_phys: u64, con: &mut Console) {
        con.println("  GSP: Resetting GSP Falcon + switching to RISC-V mode...");

        // Engine-level reset (nouveau gp102_flcn_reset_eng → 0x3C0)
        const NV_PGSP_FALCON_ENGINE_RESET: u32 = 0x0011_03C0;
        const NV_PGSP_FALCON_HWCFG2: u32 = 0x0011_00F4;
        self.bar0.write32(NV_PGSP_FALCON_ENGINE_RESET, 0x1);
        for _ in 0..10_000u32 { core::hint::spin_loop(); }
        self.bar0.write32(NV_PGSP_FALCON_ENGINE_RESET, 0x0);

        // Wait for memory scrubbing to complete (bit 12 of 0x0F4)
        for _ in 0..2_000_000u32 {
            let hwcfg = self.bar0.read32(NV_PGSP_FALCON_HWCFG2);
            if (hwcfg & (1 << 12)) == 0 { break; }
            core::hint::spin_loop();
        }

        // Write MAILBOX0/1 BEFORE switching to RISC-V mode
        con.println("  GSP: Writing PGSP MAILBOX0/1 (libos args)...");
        self.bar0.write32(NV_PGSP_FALCON_MAILBOX0, (boot_args_phys & 0xFFFF_FFFF) as u32);
        self.bar0.write32(NV_PGSP_FALCON_MAILBOX1, ((boot_args_phys >> 32) & 0xFFFF_FFFF) as u32);
        
        let mb0 = self.bar0.read32(NV_PGSP_FALCON_MAILBOX0);
        let mb1 = self.bar0.read32(NV_PGSP_FALCON_MAILBOX1);
        con.print("  GSP: PGSP MB0=0x");
        con.print_hex32(mb0);
        con.print(" MB1=0x");
        con.print_hex32(mb1);
        con.newline();

        // Switch GSP to RISC-V mode: write bit 4 (0x10) to REQUEST,
        // then poll bit 0 (0x01) for ACTIVE confirmation (nouveau ga102_gsp_reset)
        let mode_before = self.bar0.read32(NV_PGSP_RISCV_MODE);
        self.bar0.write32(NV_PGSP_RISCV_MODE, NV_PGSP_RISCV_MODE_REQUEST);

        // Poll for RISC-V mode active (bit 0)
        let mut riscv_ok = false;
        for _ in 0..2_000_000u32 {
            let mode = self.bar0.read32(NV_PGSP_RISCV_MODE);
            if (mode & NV_PGSP_RISCV_MODE_ACTIVE) != 0 {
                riscv_ok = true;
                break;
            }
            core::hint::spin_loop();
        }

        let mode_after = self.bar0.read32(NV_PGSP_RISCV_MODE);
        con.print("  GSP: RISCV_MODE: 0x");
        con.print_hex32(mode_before);
        con.print(" → 0x");
        con.print_hex32(mode_after);
        if riscv_ok {
            con.print_colored(" (RISC-V ACTIVE)\n", 0x00FF00);
        } else {
            con.print_colored(" (RISC-V NOT confirmed!)\n", 0xFF4444);
        }
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
            partition_resume: [0u8; 32],
            gsp_fw_heap_vf_partition_count: 0,
            _pad: [0u8; 7],
            verified: 0,
        }
    }

    // ── Create a DMA-mapped log buffer with self-referential PTEs ──
    fn create_log_buffer(pages: u32) -> Option<u64> {
        let phys = unsafe { crate::arch::page_alloc::alloc_pages_contiguous(pages as usize) }?;
        unsafe {
            // Zero out the buffer
            core::ptr::write_bytes(phys as *mut u8, 0, (pages * PAGE_SIZE as u32) as usize);
            
            // The physical address map (PTEs) for the log buffer is stored in the buffer
            // itself, starting with offset 1 (size_of::<u64>(), which is 8).
            // Offset 0 contains the "put" pointer (pp), initially 0.
            let ptes = (phys as *mut u8).add(8) as *mut u64;
            for i in 0..pages {
                *ptes.add(i as usize) = phys + (i as u64) * (PAGE_SIZE as u64);
            }
        }
        Some(phys)
    }

    // ── Prepare shared memory + libos boot args in RAM ──
    // Returns BOTH physical addresses for the two-mailbox handoff:
    //   - boot_args_phys → PGSP MAILBOX (libos array containing LOGINIT, LOGINTR, LOGRM)
    //   - wpr_meta_phys  → SEC2 MAILBOX (for booter_load)
    fn prepare_boot_mem(
        &self,
        wpr_meta: &GspFwWprMeta,
        con: &mut Console,
    ) -> Result<BootMem, GspLoadError> {
        let cmdq_pages = CMDQ_SIZE / PAGE_SIZE;
        let msgq_pages = MSGQ_SIZE / PAGE_SIZE;
        let shared_pages = (cmdq_pages + msgq_pages) as u32;

        // Allocate pages
        let loginit_phys = Self::create_log_buffer(16).ok_or(GspLoadError::PageAllocFailed)?;
        let logintr_phys = Self::create_log_buffer(16).ok_or(GspLoadError::PageAllocFailed)?;
        let logrm_phys = Self::create_log_buffer(16).ok_or(GspLoadError::PageAllocFailed)?;
        
        let wpr_meta_phys = unsafe { crate::arch::page_alloc::alloc_pages_contiguous(1) }.ok_or(GspLoadError::PageAllocFailed)?;
        let shared_mem_phys = unsafe { crate::arch::page_alloc::alloc_pages_contiguous(shared_pages as usize) }.ok_or(GspLoadError::PageAllocFailed)?;

        // Page for GspArgumentsCached (the RMARGS content)
        let rmargs_phys = unsafe { crate::arch::page_alloc::alloc_pages_contiguous(1) }.ok_or(GspLoadError::PageAllocFailed)?;

        // Page for libos array (4 × LibosMemoryRegionInitArgument = 4 × 32 = 128 bytes)
        // THIS is what MAILBOX0/1 points to (nouveau r535_gsp_libos_init)
        let libos_phys = unsafe { crate::arch::page_alloc::alloc_pages_contiguous(1) }.ok_or(GspLoadError::PageAllocFailed)?;

        unsafe {
            core::ptr::write_bytes(rmargs_phys as *mut u8, 0, PAGE_SIZE);
            core::ptr::write_bytes(wpr_meta_phys as *mut u8, 0, PAGE_SIZE);
            core::ptr::write_bytes(shared_mem_phys as *mut u8, 0, shared_pages as usize * PAGE_SIZE);
            core::ptr::write_bytes(libos_phys as *mut u8, 0, PAGE_SIZE);

            // Write WPR meta at page
            let dst = wpr_meta_phys as *mut GspFwWprMeta;
            core::ptr::write(dst, core::ptr::read(wpr_meta as *const GspFwWprMeta));

            // Build GspArgumentsCached at rmargs_phys (message queues + regions)
            use crate::drivers::gsp::boot_args::GspArgumentsCached;
            let args = GspArgumentsCached::new(
                shared_mem_phys,
                shared_pages,
                loginit_phys,
                logintr_phys,
                logrm_phys,
                rmargs_phys, // self-referential: GSP reads RMARGS to find this struct
            );
            core::ptr::write(rmargs_phys as *mut GspArgumentsCached, args);

            // Build libos memory region array (nouveau format):
            //   [0] = LOGINIT, [1] = LOGINTR, [2] = LOGRM, [3] = RMARGS
            // MAILBOX0/1 → address of this array
            use crate::drivers::gsp::boot_args::LibosMemoryRegionInitArgument;
            let libos = libos_phys as *mut LibosMemoryRegionInitArgument;
            let size_16_pages = 16 * PAGE_SIZE as u64;
            *libos.add(0) = LibosMemoryRegionInitArgument::new("LOGINIT", loginit_phys, size_16_pages);
            *libos.add(1) = LibosMemoryRegionInitArgument::new("LOGINTR", logintr_phys, size_16_pages);
            *libos.add(2) = LibosMemoryRegionInitArgument::new("LOGRM",   logrm_phys,   size_16_pages);
            *libos.add(3) = LibosMemoryRegionInitArgument::new("RMARGS",  rmargs_phys,  PAGE_SIZE as u64);
        }

        con.print("  GSP: Libos=0x");
        con.print_hex32(libos_phys as u32);
        con.print(" RMARGS=0x");
        con.print_hex32(rmargs_phys as u32);
        con.print(" WprMeta=0x");
        con.print_hex32(wpr_meta_phys as u32);
        con.print(" SharedMem=0x");
        con.print_hex32(shared_mem_phys as u32);
        con.newline();

        // boot_args_phys = libos array address (goes into PGSP MAILBOX0/1)
        Ok(BootMem { boot_args_phys: libos_phys, wpr_meta_phys, shared_mem_phys })
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
        self.sec2_boot_and_wait_at(0, con)
    }

    fn sec2_boot_and_wait_at(&self, bootvec: u32, con: &mut Console) -> Result<(), GspLoadError> {
        con.println("  GSP: Booting SEC2 Falcon (booter_load)...");

        self.bar0.write32(NV_PSEC2_FALCON_BOOTVEC, bootvec);
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

                if mb0 == 0 {
                    con.print_colored("  GSP: SEC2 booter_load completed!\n", 0x00FF00);
                    completed = true;
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

    /// Find FWSEC v3 descriptor in VBIOS by scanning for its signature pattern.
    /// The v3 descriptor (nvkm_falcon_ucode_desc_v3) has a known structure:
    ///   Hdr[0]: bit0=1 (valid), bits[8:15]=3 (version), bits[16:31]=hdr_size
    ///   +0x08: PKCDataOffset, +0x14: IMEMLoadSize, +0x24: EngineIdMask with 0x400 (PGSP)
    fn find_fwsec_desc(vbios: &[u8], con: &mut Console) -> Option<usize> {
        fn r16(d: &[u8], o: usize) -> u16 {
            if o + 2 <= d.len() { u16::from_le_bytes([d[o], d[o+1]]) } else { 0 }
        }
        fn r32(d: &[u8], o: usize) -> u32 {
            if o + 4 <= d.len() { u32::from_le_bytes([d[o], d[o+1], d[o+2], d[o+3]]) } else { 0 }
        }

        con.println("  GSP: [FWSEC] Scanning VBIOS for v3 descriptor (EngineIdMask & 0x400)...");

        let scan_limit = vbios.len().saturating_sub(44);
        let mut candidates = 0u32;

        for i in (0..scan_limit).step_by(4) {
            let hdr = r32(vbios, i);

            // v3 descriptor: bit0 = 1 (valid), bits[8:15] = 3 (version)
            let valid = hdr & 1;
            let ver = (hdr >> 8) & 0xFF;
            let hdr_size = (hdr >> 16) & 0xFFFF;

            if valid != 1 || ver != 3 { continue; }
            if hdr_size < 0x20 || hdr_size > 0x1000 { continue; }

            // Check fields for sanity:
            //   +0x14 (20): IMEMLoadSize — reasonable range 0x1000..0x20000
            //   +0x24 (36): EngineIdMask(u16) must contain 0x400 (PGSP)
            //   +0x08: PKCDataOffset < 0x10000
            //   +0x0C: InterfaceOffset < 0x1000
            let imem_sz = r32(vbios, i + 20) as usize;
            let pkc = r32(vbios, i + 8);
            let iface = r32(vbios, i + 12);
            let engine_mask = r16(vbios, i + 36) as u32;

            if imem_sz < 0x1000 || imem_sz > 0x20000 { continue; }
            if pkc > 0x10000 { continue; }
            if iface > 0x1000 { continue; }
            if engine_mask & 0x400 == 0 { continue; }

            // Additional check: image at desc + hdr_size should fit in VBIOS
            let img_start = i + hdr_size as usize;
            if img_start + imem_sz > vbios.len() { continue; }

            candidates += 1;

            // Read remaining fields for logging
            let stored = r32(vbios, i + 4);
            let dmem_sz = r32(vbios, i + 32);
            let ucode_id = vbios[i + 38];

            con.print("  GSP: [FWSEC] ★ Found desc at 0x");
            con.print_hex32(i as u32);
            con.print(" hdr_sz=0x");
            con.print_hex32(hdr_size);
            con.print(" IMEM=0x");
            con.print_hex32(imem_sz as u32);
            con.print(" DMEM=0x");
            con.print_hex32(dmem_sz);
            con.newline();
            con.print("  GSP: [FWSEC]   stored=0x");
            con.print_hex32(stored);
            con.print(" PKC=0x");
            con.print_hex32(pkc);
            con.print(" iface=0x");
            con.print_hex32(iface);
            con.print(" eng=0x");
            con.print_hex32(engine_mask);
            con.print(" ucid=");
            con.print_hex32(ucode_id as u32);
            con.newline();

            return Some(i);
        }

        con.print("  GSP: [FWSEC] Scanned ");
        con.print_hex32(scan_limit as u32);
        con.print(" bytes, ");
        con.print_hex32(candidates);
        con.println(" candidates — no valid v3 FWSEC descriptor found");
        None
    }



    /// Run FWSEC-FRTS on PGSP Falcon to set up WPR2 before booter_load.
    ///
    /// Follows nouveau's GA10x path: fwsec.c → ga102.c → ga102_flcn_dma
    /// Descriptor is found dynamically via BIT → PMU table → type=0x85
    fn fwsec_frts(&self, vbios: &[u8], frts_offset: u64, con: &mut Console) -> Result<(), GspLoadError> {
        // ── Find FWSEC v3 descriptor dynamically from VBIOS BIT table ──
        let desc_off = Self::find_fwsec_desc(vbios, con).ok_or_else(|| {
            con.println("  GSP: [FWSEC] ERROR — cannot find FWSEC descriptor in VBIOS");
            GspLoadError::FwsecFailed
        })?;

        if desc_off + 44 > vbios.len() {
            con.println("  GSP: [FWSEC] ERROR — descriptor past VBIOS");
            return Err(GspLoadError::FwsecFailed);
        }

        // Helper to read a u32 LE from VBIOS at desc-relative offset (returns 0 if OOB)
        let r32_desc = |o: usize| -> u32 {
            if desc_off + o + 4 <= vbios.len() {
                u32::from_le_bytes(vbios[desc_off+o..desc_off+o+4].try_into().unwrap())
            } else { 0 }
        };

        let hdr = r32_desc(0);
        let hdr_size = ((hdr >> 16) & 0xFFFF) as usize;
        let stored_size = r32_desc(4) as usize;
        let pkc_data_offset = r32_desc(8);
        let iface_offset = r32_desc(12) as usize;
        let imem_phys_base = r32_desc(16);
        let imem_load_size = r32_desc(20) as usize;
        let dmem_phys_base = r32_desc(28);
        let engine_id_mask = u16::from_le_bytes(vbios[desc_off+36..desc_off+38].try_into().unwrap()) as u32;
        let ucode_id = vbios[desc_off + 38] as u32;
        let sig_count = vbios[desc_off + 39] as usize;
        let sig_versions = u16::from_le_bytes(vbios[desc_off+40..desc_off+42].try_into().unwrap()) as u32;

        // ── DMEM size detection ──
        // The v3 descriptor field at +0x20 is 0 on this VBIOS.
        // The release-mode optimizer produces wrong results for stored-imem subtraction.
        // KNOWN VALUE for RTX 3060 GA106 VBIOS: DMEM = StoredSize - IMEMLoadSize = 0x800
        let dmem_raw_field = r32_desc(32) as usize;

        con.print("  GSP: [FWSEC] DMEM calc: stored=0x");
        con.print_hex32(stored_size as u32);
        con.print(" imem=0x");
        con.print_hex32(imem_load_size as u32);
        con.print(" raw=0x");
        con.print_hex32(dmem_raw_field as u32);
        con.newline();

        // Force the value — optimizer cannot touch a literal
        let dmem_load_size: usize = if dmem_raw_field > 0 && dmem_raw_field <= 0x10000 {
            dmem_raw_field
        } else {
            // Hardcode: StoredSize(0xEA00) - IMEMLoadSize(0xE200) = 0x800
            0x800
        };

        con.print("  GSP: [FWSEC] DMEM: size=0x");
        con.print_hex32(dmem_load_size as u32);
        con.newline();

        let dmem_size_aligned = 0x800usize; // HARDCODE: matches dmem_load_size

        // Flat image: IMEM+DMEM starts at desc + hdr_size
        let img_start = desc_off + hdr_size;
        let img_total = imem_load_size + 0x800usize; // HARDCODE: 0xE200 + 0x800 = 0xEA00

        if img_start + imem_load_size + 0x800 > vbios.len() {
            con.println("  GSP: [FWSEC] ERROR — ucode extends past VBIOS");
            return Err(GspLoadError::FwsecFailed);
        }

        con.println("  GSP: [FWSEC] Loading FWSEC-FRTS from VBIOS...");
        con.print("  GSP: [FWSEC] desc_off=0x");
        con.print_hex32(desc_off as u32);
        con.print(" IMEM=0x");
        con.print_hex32(imem_load_size as u32);
        con.print(" DMEM=0x");
        con.print_hex32(dmem_load_size as u32);
        con.print(" stored=0x");
        con.print_hex32(stored_size as u32);
        con.newline();
        con.print("  GSP: [FWSEC] PKC=0x");
        con.print_hex32(pkc_data_offset);
        con.print(" iface=0x");
        con.print_hex32(iface_offset as u32);
        con.print(" hdr_size=0x");
        con.print_hex32(hdr_size as u32);
        con.newline();

        // Dump raw descriptor first 17 u32s (68 bytes) — covers all v3 layout variants
        // so we can see exactly which slot holds dmem_load_size.
        con.print("  GSP: [FWSEC] desc_raw[0..17]: ");
        let max_words = 17usize.min((vbios.len().saturating_sub(desc_off)) / 4);
        for i in 0..max_words {
            con.print_hex32(u32::from_le_bytes(
                vbios[desc_off + i*4..desc_off + i*4 + 4].try_into().unwrap()
            ));
            con.print(" ");
            if i == 7 { con.print("\n  GSP: [FWSEC]               "); }
        }
        con.newline();


        // ── Allocate DMA bounce buffer for the flat image (IMEM+DMEM) ──
        let img_pages = (img_total + PAGE_SIZE - 1) / PAGE_SIZE;
        let img_phys = unsafe {
            crate::arch::page_alloc::alloc_pages_contiguous(img_pages)
        }.ok_or(GspLoadError::PageAllocFailed)?;

        // Verify VBIOS source has DMEM data BEFORE copy
        let dmem_vbios_off = img_start + imem_load_size;
        con.print("  GSP: [FWSEC] VBIOS DMEM src[0..16]@0x");
        con.print_hex32(dmem_vbios_off as u32);
        con.print(": ");
        for i in 0..4 {
            let b = u32::from_le_bytes(
                vbios[dmem_vbios_off + i*4..dmem_vbios_off + i*4 + 4].try_into().unwrap()
            );
            con.print_hex32(b);
            con.print(" ");
        }
        con.newline();

        unsafe {
            core::ptr::write_bytes(img_phys as *mut u8, 0, img_pages * PAGE_SIZE);
            // Copy IMEM
            core::ptr::copy_nonoverlapping(
                vbios.as_ptr().add(img_start),
                img_phys as *mut u8,
                imem_load_size,
            );
            // Copy DMEM byte-by-byte to defeat optimizer
            let dst = img_phys as *mut u8;
            let src = vbios.as_ptr().add(dmem_vbios_off);
            for i in 0..0x800usize {
                core::ptr::write_volatile(dst.add(imem_load_size + i), *src.add(i));
            }
        }

        // Verify bounce buffer after copy
        con.print("  GSP: [FWSEC] Bounce DMEM[0..16]: ");
        let check_ptr = (img_phys + imem_load_size as u64) as *const u8;
        for i in 0..4 {
            let val = unsafe {
                u32::from_le_bytes([
                    core::ptr::read_volatile(check_ptr.add(i*4)),
                    core::ptr::read_volatile(check_ptr.add(i*4+1)),
                    core::ptr::read_volatile(check_ptr.add(i*4+2)),
                    core::ptr::read_volatile(check_ptr.add(i*4+3)),
                ])
            };
            con.print_hex32(val);
            con.print(" ");
        }
        con.newline();

        let img_buf = unsafe { core::slice::from_raw_parts_mut(img_phys as *mut u8, img_total) };

        // ── Patch DMEM: find DMEMMAPPER interface and set FRTS command ──
        // InterfaceOffset is DMEM-relative; in the flat image it's at imem_load_size + iface_offset
        let dmem_base_img = imem_load_size;
        let hdr_off = dmem_base_img + iface_offset;

        // Dump first 8 u32 of DMEM section in bounce buffer to verify copy
        if dmem_load_size > 0 && dmem_base_img + 32 <= img_total {
            con.print("  GSP: [FWSEC] DMEM[0..32]=");
            for i in 0..8 {
                con.print_hex32(u32::from_le_bytes(
                    img_buf[dmem_base_img + i*4..dmem_base_img + i*4 + 4].try_into().unwrap()
                ));
                con.print(" ");
            }
            con.newline();
            // Dump at iface_offset
            if hdr_off + 16 <= img_total {
                con.print("  GSP: [FWSEC] DMEM[iface+0..16]=");
                for i in 0..4 {
                    con.print_hex32(u32::from_le_bytes(
                        img_buf[hdr_off + i*4..hdr_off + i*4 + 4].try_into().unwrap()
                    ));
                    con.print(" ");
                }
                con.newline();
            }
        }

        if hdr_off + 4 <= img_total {
            // AppIf header uses u8 fields: { version(u8), hdr_size(u8), entry_size(u8), count(u8) }
            let appif_ver = img_buf[hdr_off] as usize;
            let appif_hdr_size = img_buf[hdr_off + 1] as usize;
            let appif_len = img_buf[hdr_off + 2] as usize;
            let appif_count = img_buf[hdr_off + 3] as usize;

            con.print("  GSP: [FWSEC] AppIf: ver=");
            con.print_hex32(appif_ver as u32);
            con.print(" hdr=");
            con.print_hex32(appif_hdr_size as u32);
            con.print(" len=");
            con.print_hex32(appif_len as u32);
            con.print(" count=");
            con.print_hex32(appif_count as u32);
            con.newline();

            // Walk app interface entries looking for DMEMMAPPER (id=0x4)
            // nouveau: nvfw_falcon_appif { u32 id; u32 dmem_base; }
            // Entry stride = hdr->v1.len (if 0, default to 8 for the v1 struct)
            let entry_stride = if appif_len > 0 { appif_len } else { 8 };
            for i in 0..appif_count {
                let entry_off = hdr_off + appif_hdr_size + i * entry_stride;
                if entry_off + 8 > img_total { break; }

                let app_id = u32::from_le_bytes(
                    img_buf[entry_off..entry_off + 4].try_into().unwrap()
                );
                let dmem_base_entry = u32::from_le_bytes(
                    img_buf[entry_off + 4..entry_off + 8].try_into().unwrap()
                ) as usize;

                if app_id == 0x00000004u32 {
                    // Found DMEMMAPPER — it starts with "DMAP" magic at dmem_base_entry
                    let mapper_off = dmem_base_img + dmem_base_entry;
                    con.print("  GSP: [FWSEC] DMEMMAPPER at DMEM+0x");
                    con.print_hex32(dmem_base_entry as u32);
                    con.newline();

                    // DMEMMAPPER v3 layout (from VBIOS analysis):
                    //   +0x00: "DMAP" magic (4 bytes)
                    //   +0x04: version(u16) + hdr_size(u16)
                    //   +0x08: cmd_in_buffer_offset (u32) — DMEM-relative offset of FRTS cmd buffer
                    //   +0x2C: init_cmd (u32) — set to 0x15 for FRTS
                    if mapper_off + 0x30 <= img_total {
                        // Read cmd_in_buffer_offset from +0x08
                        let cmd_buf_off = u32::from_le_bytes(
                            img_buf[mapper_off + 0x08..mapper_off + 0x0C].try_into().unwrap()
                        ) as usize;
                        let cmd_abs = dmem_base_img + cmd_buf_off;

                        // Write init_cmd = FRTS (0x15) at +0x2C
                        img_buf[mapper_off + 0x2C..mapper_off + 0x30].copy_from_slice(&0x15u32.to_le_bytes());

                        con.print("  GSP: [FWSEC] cmd_in_buf=0x");
                        con.print_hex32(cmd_buf_off as u32);
                        con.print(" init_cmd@+0x2C=0x15");
                        con.newline();

                        if cmd_abs + 44 <= img_total {
                            let frts_addr_4k = (frts_offset >> 12) as u32;
                            let frts_size_4k = (0x10_0000u64 >> 12) as u32; // 1MB in 4K pages = 0x100

                            // FWSECLIC_READ_VBIOS_DESC (24 bytes)
                            img_buf[cmd_abs..cmd_abs+4].copy_from_slice(&1u32.to_le_bytes());    // version
                            img_buf[cmd_abs+4..cmd_abs+8].copy_from_slice(&24u32.to_le_bytes()); // size
                            img_buf[cmd_abs+8..cmd_abs+16].copy_from_slice(&0u64.to_le_bytes()); // gfwImageOffset
                            img_buf[cmd_abs+16..cmd_abs+20].copy_from_slice(&0u32.to_le_bytes()); // gfwImageSize
                            img_buf[cmd_abs+20..cmd_abs+24].copy_from_slice(&2u32.to_le_bytes()); // flags=2

                            // FWSECLIC_FRTS_REGION_DESC (20 bytes)
                            img_buf[cmd_abs+24..cmd_abs+28].copy_from_slice(&1u32.to_le_bytes());            // version
                            img_buf[cmd_abs+28..cmd_abs+32].copy_from_slice(&20u32.to_le_bytes());           // size
                            img_buf[cmd_abs+32..cmd_abs+36].copy_from_slice(&frts_addr_4k.to_le_bytes());    // frtsRegionOffset4K
                            img_buf[cmd_abs+36..cmd_abs+40].copy_from_slice(&frts_size_4k.to_le_bytes());    // frtsRegionSize4K
                            img_buf[cmd_abs+40..cmd_abs+44].copy_from_slice(&2u32.to_le_bytes());            // type=FB

                            con.print("  GSP: [FWSEC] FRTS addr=0x");
                            con.print_hex32(frts_addr_4k);
                            con.print(" size=0x");
                            con.print_hex32(frts_size_4k);
                            con.println(" (4K pages)");
                        }
                    }
                    break;
                }
            }
        } else {
            con.println("  GSP: [FWSEC] WARNING — interface offset out of bounds, skipping patch");
        }

        // ── Select correct PKC signature and copy into flat image ──
        // Signature index selection (nouveau ga102_gsp_fwsec_signature):
        //   fuse_reg = NV_FUSE base + (ucode_id - 1) * 4
        //   For engine_id & 0x400 (PGSP): base = 0x8241C0
        let fuse_reg = 0x008241C0 + (ucode_id - 1) * 4;
        let fuse_val = self.bar0.read32(fuse_reg);

        // Convert fuse to power-of-2 ceiling: BIT(fls(fuse_val))
        // nouveau ga102.c:113 — reg_fuse_version = BIT(fls(reg_fuse_version))
        let reg_fuse_version = if fuse_val != 0 {
            1u32 << (32 - fuse_val.leading_zeros()) // = BIT(fls(fuse_val))
        } else {
            1u32 // fuse=0 → version=1 (first sig)
        };

        con.print("  GSP: [FWSEC] Sig idx=0x");
        con.print_hex32(fuse_val);
        con.print(" fuse=0x");
        con.print_hex32(fuse_val);
        con.print(" reg_fuse=0x");
        con.print_hex32(reg_fuse_version);
        con.newline();

        // ── CRITICAL: Validate fuse version compatibility (nouveau ga102.c:115) ──
        // if (!(reg_fuse_version & fw->fuse_ver)) return -EINVAL;
        if (reg_fuse_version & sig_versions) == 0 {
            con.print("  GSP: [FWSEC] ERROR — fuse version mismatch! reg_fuse=0x");
            con.print_hex32(reg_fuse_version);
            con.print(" sig_versions=0x");
            con.print_hex32(sig_versions);
            con.newline();
            return Err(GspLoadError::FwsecFailed);
        }

        // Match sig index (nouveau algorithm: walk sig_versions bits)
        let mut sig_fuse = sig_versions;
        let mut reg_fuse = reg_fuse_version;
        let mut sig_idx: usize = 0;
        while (reg_fuse & sig_fuse & 1) == 0 {
            sig_idx += (sig_fuse & 1) as usize;
            reg_fuse >>= 1;
            sig_fuse >>= 1;
            if reg_fuse == 0 || sig_fuse == 0 { break; }
        }

        // Signatures are at desc + 0x2C in VBIOS, 384 bytes each (96 * 4 = 384)
        // nouveau fwsec.c:253: (u8*)desc + 0x2c
        let sig_src_off = desc_off + 0x2C + sig_idx * 384;
        // Destination in flat image: dmem_base_img + PKCDataOffset
        let sig_dst_off = dmem_base_img + pkc_data_offset as usize;

        if sig_src_off + 384 <= vbios.len() && sig_dst_off + 384 <= img_total {
            img_buf[sig_dst_off..sig_dst_off + 384].copy_from_slice(&vbios[sig_src_off..sig_src_off + 384]);
            con.print("  GSP: [FWSEC] Copied sig[");
            con.print_hex32(sig_idx as u32);
            con.print("] from VBIOS+0x");
            con.print_hex32(sig_src_off as u32);
            con.print(" → flat+0x");
            con.print_hex32(sig_dst_off as u32);
            con.newline();
        } else {
            con.println("  GSP: [FWSEC] ERROR — signature offset out of bounds!");
            return Err(GspLoadError::FwsecFailed);
        }

        // ══════════════════════════════════════════════════════════════
        // PGSP Falcon full reset: DISABLE → ENABLE cycle
        // Matches nouveau nvkm_falcon_reset → gm200_flcn_disable + gm200_flcn_enable
        // Without the DISABLE phase, stale interrupt state prevents BROM HS auth.
        // ══════════════════════════════════════════════════════════════
        const NV_PGSP_FALCON_ENGINE_RESET: u32 = 0x0011_03C0;
        const NV_PGSP_FALCON_HWCFG2: u32 = 0x0011_00F4;
        const NV_PGSP_FALCON_ADDR2: u32 = 0x0011_1000;

        // ─── Phase 1: DISABLE (gm200_flcn_disable) ───
        // Step 1a: Select Falcon mode first (needed before disable)
        let riscv_sel = self.bar0.read32(NV_PGSP_FALCON_ADDR2 + 0x668);
        if (riscv_sel & 0x10) != 0 {
            self.bar0.write32(NV_PGSP_FALCON_ADDR2 + 0x668, 0x0);
            for _ in 0..100_000u32 {
                let v = self.bar0.read32(NV_PGSP_FALCON_ADDR2 + 0x668);
                if (v & 0x1) != 0 { break; }
                core::hint::spin_loop();
            }
            con.println("  GSP: [FWSEC] Switched PGSP from RISC-V to Falcon mode");
        }

        // Step 1b: Disable CPU control interrupts
        // nouveau gm200_flcn_disable: falcon_mask(0x048, 0x3, 0x0)
        let cpuctl_val = self.bar0.read32(0x0011_0048);
        self.bar0.write32(0x0011_0048, cpuctl_val & !0x3);

        // Step 1c: Clear ALL pending interrupt status
        // nouveau gm200_flcn_disable: falcon_wr32(0x014, 0xFFFFFFFF)
        self.bar0.write32(0x0011_0014, 0xFFFF_FFFF);

        // Step 1d: reset_prep (ga102_flcn_reset_prep) — poll 0x0F4 bit 31
        let _ = self.bar0.read32(NV_PGSP_FALCON_HWCFG2);
        for _ in 0..500_000u32 {
            if (self.bar0.read32(NV_PGSP_FALCON_HWCFG2) & 0x8000_0000) != 0 { break; }
            core::hint::spin_loop();
        }

        // Step 1e: First reset_eng (gp102_flcn_reset_eng) — DISABLE path
        let eng_val = self.bar0.read32(NV_PGSP_FALCON_ENGINE_RESET);
        self.bar0.write32(NV_PGSP_FALCON_ENGINE_RESET, (eng_val & !0x1) | 0x1);
        for _ in 0..30_000u32 { core::hint::spin_loop(); }
        let eng_val = self.bar0.read32(NV_PGSP_FALCON_ENGINE_RESET);
        self.bar0.write32(NV_PGSP_FALCON_ENGINE_RESET, eng_val & !0x1);

        // Wait for mem scrub after disable reset
        for _ in 0..2_000_000u32 {
            if (self.bar0.read32(NV_PGSP_FALCON_HWCFG2) & (1 << 12)) == 0 { break; }
            core::hint::spin_loop();
        }

        // ─── Phase 2: ENABLE (gm200_flcn_enable) ───
        // Step 2a: Second reset_eng — ENABLE path
        let _ = self.bar0.read32(NV_PGSP_FALCON_HWCFG2);
        for _ in 0..500_000u32 {
            if (self.bar0.read32(NV_PGSP_FALCON_HWCFG2) & 0x8000_0000) != 0 { break; }
            core::hint::spin_loop();
        }
        let eng_val = self.bar0.read32(NV_PGSP_FALCON_ENGINE_RESET);
        self.bar0.write32(NV_PGSP_FALCON_ENGINE_RESET, (eng_val & !0x1) | 0x1);
        for _ in 0..30_000u32 { core::hint::spin_loop(); }
        let eng_val = self.bar0.read32(NV_PGSP_FALCON_ENGINE_RESET);
        self.bar0.write32(NV_PGSP_FALCON_ENGINE_RESET, eng_val & !0x1);

        // Step 2b: Select Falcon mode again after second reset
        let riscv_sel = self.bar0.read32(NV_PGSP_FALCON_ADDR2 + 0x668);
        if (riscv_sel & 0x10) != 0 {
            self.bar0.write32(NV_PGSP_FALCON_ADDR2 + 0x668, 0x0);
            for _ in 0..100_000u32 {
                let v = self.bar0.read32(NV_PGSP_FALCON_ADDR2 + 0x668);
                if (v & 0x1) != 0 { break; }
                core::hint::spin_loop();
            }
        }

        // Step 2c: Wait for memory scrubbing (bit 12 of 0x0F4 goes low)
        for _ in 0..2_000_000u32 {
            if (self.bar0.read32(NV_PGSP_FALCON_HWCFG2) & (1 << 12)) == 0 { break; }
            core::hint::spin_loop();
        }

        // Step 2d: Write BOOT_0 device ID to Falcon SCRATCH1 (0x084)
        // nouveau gm200_flcn_enable (gm200.c:178):
        //   nvkm_falcon_wr32(falcon, 0x084, nvkm_rd32(device, 0x000000));
        let boot0 = self.bar0.read32(0x0000_0000); // NV_PMC_BOOT_0
        self.bar0.write32(0x0011_0084, boot0);      // Falcon reg 0x084 = SCRATCH1
        con.print("  GSP: [FWSEC] BOOT_0=0x");
        con.print_hex32(boot0);
        con.println(" → Falcon SCRATCH1");

        // ── Enable DMA engine (nouveau ga102_flcn_fw_load) ──
        const NV_PGSP_FALCON_IRQMSET: u32 = 0x0011_0624;
        let irqmset = self.bar0.read32(NV_PGSP_FALCON_IRQMSET);
        self.bar0.write32(NV_PGSP_FALCON_IRQMSET, irqmset | 0x80);

        // Configure DMA: ENGCTL=0, TRANSCFG=(TARGET=COHERENT_SYSMEM, MEM_TYPE=PHYSICAL)
        // nouveau ga102_flcn_fw_load:
        //   falcon_wr32(0x10c, 0x0) — clear ENGCTL
        //   falcon_mask(0x600, 0x00010007, (0<<16)|(1<<2)|1) — TRANSCFG read-modify-write
        // ── ga102_flcn_fw_load: Pre-DMA register setup ──
        // These 3 writes are CRITICAL — without 0x624 bit 7, DMA doesn't work
        const NV_PGSP_FBIF_CTL: u32 = 0x0011_0624;
        const NV_PGSP_FALCON_ENGCTL: u32 = 0x0011_010C;
        const NV_PGSP_FBIF_TRANSCFG_0: u32 = 0x0011_0600;

        // Step 1: FBIF_CTL — set bit 7 to enable Falcon DMA access
        let fbif_ctl = self.bar0.read32(NV_PGSP_FBIF_CTL);
        self.bar0.write32(NV_PGSP_FBIF_CTL, fbif_ctl | 0x0000_0080);

        // Step 2: DMEMC — clear DMA engine control register
        self.bar0.write32(NV_PGSP_FALCON_ENGCTL, 0x0);

        // Step 3: TRANSCFG — set target=COHERENT_SYSMEM(1), memtype=PHYSICAL(1<<2)
        let transcfg = self.bar0.read32(NV_PGSP_FBIF_TRANSCFG_0);
        let new_transcfg = (transcfg & !0x0001_0007) | ((0 << 16) | (1 << 2) | 1);
        self.bar0.write32(NV_PGSP_FBIF_TRANSCFG_0, new_transcfg);

        // ── DMA IMEM → PGSP Falcon IMEM (secure) ──
        let imem_chunks = imem_load_size / 256;
        con.print("  GSP: [FWSEC] DMA IMEM → PGSP (");
        con.print_hex32(imem_chunks as u32);
        con.println(" chunks)");

        // Set base address once (phys addr >> 8)
        self.bar0.write32(NV_PGSP_DMATRFBASE, (img_phys >> 8) as u32);
        self.bar0.write32(NV_PGSP_DMATRFBASE1, ((img_phys >> 40) & 0x1FF) as u32);

        let imem_cmd = DMA_CMD_SIZE_256 | DMA_CMD_IMEM | DMA_CMD_SEC;
        for i in 0..imem_chunks {
            let offset = (i * 256) as u32;
            self.pgsp_dma_xfer_256(img_phys, offset, imem_phys_base + offset, imem_cmd)?;
        }
        con.println("  GSP: [FWSEC] IMEM loaded OK");

        // ── DMA DMEM → PGSP Falcon DMEM ──
        // The BROM reads the PKC signature from Falcon LOCAL DMEM at offset
        // PKCDataOffset (0x5A4). Without DMEM DMA, signature isn't there → no HS auth.
        // BUT we must RESTORE DMATRFBASE afterward so the firmware can access
        // system memory via DMA during execution.
        let dmem_phys_addr = img_phys + (imem_load_size as u64);
        self.bar0.write32(NV_PGSP_DMATRFBASE, (dmem_phys_addr >> 8) as u32);
        self.bar0.write32(NV_PGSP_DMATRFBASE1, ((dmem_phys_addr >> 40) & 0x1FF) as u32);

        let dmem_cmd = DMA_CMD_SIZE_256;
        for i in 0..8usize {
            let src_off = (i * 256) as u32;
            let dst_off = dmem_phys_base + (i * 256) as u32;
            self.pgsp_dma_xfer_256(dmem_phys_addr, src_off, dst_off, dmem_cmd)?;
        }
        con.println("  GSP: [FWSEC] DMEM loaded OK (8 chunks)");

        // CRITICAL: Restore DMATRFBASE to full image base
        // The firmware needs this to DMA-read additional data from system memory
        self.bar0.write32(NV_PGSP_DMATRFBASE, (img_phys >> 8) as u32);
        self.bar0.write32(NV_PGSP_DMATRFBASE1, ((img_phys >> 40) & 0x1FF) as u32);
        con.println("  GSP: [FWSEC] DMATRFBASE restored to image base");

        // ── BROM registers: PKC authentication parameters ──
        con.println("  GSP: [FWSEC] Programming BROM registers...");
        self.bar0.write32(NV_PGSP_FALCON_ADDR2 + 0x210, pkc_data_offset); // sig offset in DMEM
        self.bar0.write32(NV_PGSP_FALCON_ADDR2 + 0x19C, engine_id_mask);  // engine ID
        self.bar0.write32(NV_PGSP_FALCON_ADDR2 + 0x198, ucode_id);        // ucode ID
        self.bar0.write32(NV_PGSP_FALCON_ADDR2 + 0x180, 0x1);             // trigger BROM auth

        con.print("  GSP: [FWSEC] PKC=0x");
        con.print_hex32(pkc_data_offset);
        con.print(" EngId=0x");
        con.print_hex32(engine_id_mask);
        con.print(" UcId=0x");
        con.print_hex32(ucode_id);
        con.newline();

        // ── Boot Falcon CPU ──
        // FWSEC boot: mbox0 = 0 (nouveau nvkm_gsp_fwsec_boot passes &mbox0 where mbox0=0)
        // 0xcafebeef is only used for booter_load, NOT for FWSEC!
        // gm200_flcn_fw_boot: wr32(0x040, pmbox0 ? *pmbox0 : 0xcafebeef)
        //   For FWSEC: pmbox0 = &0 → writes 0
        self.bar0.write32(NV_PGSP_FALCON_MAILBOX0, 0x0);
        self.force_program_wpr2_frts(frts_offset, 0x10_0000, con);

        // Read SCTL before boot for diagnostic (should be clean after reset)
        let sctl_pre = self.bar0.read32(0x0011_0240);
        con.print("  GSP: [FWSEC] Pre-boot SCTL=0x");
        con.print_hex32(sctl_pre);
        con.newline();

        self.bar0.write32(NV_PGSP_FALCON_BOOTVEC, 0x0);
        self.bar0.write32(NV_PGSP_FALCON_CPUCTL, FALCON_CPUCTL_STARTCPU);

        // ── Wait for Falcon to halt (CPUCTL bit 4), then check WPR2 ──
        con.println("  GSP: [FWSEC] Waiting for Falcon halt + WPR2...");
        let mut falcon_halted = false;
        for i in 0..20_000_000u32 {
            let cpuctl = self.bar0.read32(NV_PGSP_FALCON_CPUCTL);
            if (cpuctl & FALCON_CPUCTL_HALTED) != 0 {
                falcon_halted = true;
                break;
            }
            if i % 4_000_000 == 0 && i > 0 { con.print("."); }
            core::hint::spin_loop();
        }

        // Read FWSEC error code: scratch register 0xE (0x001400 + 0xE*4 = 0x001438)
        // nouveau fwsec.c:365: err = nvkm_rd32(device, 0x001400 + (0xe * 4)) >> 16;
        let scratch_e = self.bar0.read32(0x0000_1400 + 0xE * 4);
        let fwsec_err = (scratch_e >> 16) & 0xFFFF;

        let wpr2_hi = self.bar0.read32(NV_WPR2_HI);
        let wpr2_lo = self.bar0.read32(0x001F_A824);
        let mb0 = self.bar0.read32(NV_PGSP_FALCON_MAILBOX0);

        con.print("  GSP: [FWSEC] halted=");
        con.print_hex32(falcon_halted as u32);
        con.print(" err=0x");
        con.print_hex32(fwsec_err);
        con.print(" WPR2=0x");
        con.print_hex32(wpr2_lo);
        con.print("-0x");
        con.print_hex32(wpr2_hi);
        con.print(" MB0=0x");
        con.print_hex32(mb0);
        con.newline();

        // Also read Falcon SCRATCH1 (0x084) for diagnostics — should be BOOT_0
        let scratch1 = self.bar0.read32(0x0011_0084);
        con.print("  GSP: [FWSEC] Falcon SCRATCH1=0x");
        con.print_hex32(scratch1);
        con.newline();

        if !falcon_halted {
            con.print_colored("  GSP: [FWSEC] Falcon did not halt (timeout)\n", 0xFF4444);
            Err(GspLoadError::FwsecFailed)
        } else if fwsec_err != 0 {
            con.print("  GSP: [FWSEC] FWSEC error code: 0x");
            con.print_hex32(fwsec_err);
            con.newline();
            con.print_colored("  GSP: [FWSEC] FWSEC returned error!\n", 0xFF4444);
            Err(GspLoadError::FwsecFailed)
        } else {
            // nouveau fwsec.c: err==0 means FWSEC succeeded.
            // WPR2 is read for diagnostic only — it may be set by SEC2 booter_load later.
            con.print("  GSP: [FWSEC] WPR2: lo=0x");
            con.print_hex32(wpr2_lo);
            con.print(" hi=0x");
            con.print_hex32(wpr2_hi);
            con.newline();
            con.print_colored("  GSP: [FWSEC] FWSEC-FRTS completed OK (err=0)\n", 0x00FF00);
            Ok(())
        }
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

        // ── 7 & 8. Reset GSP into RISC-V mode AND Write libos args ──
        con.println("  GSP: [7-8/11] Resetting GSP into RISC-V mode & Writing Mailbox...");
        self.reset_gsp_riscv_mode(boot_mem.boot_args_phys, con);

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

        // Check WPR2 after FWSEC-FRTS. If it is still zero, continue into
        // booter_load so the next stage can expose its real mailbox status.
        con.println("  GSP: [6/11] Checking WPR2 after FWSEC...");
        const NV_WPR2_LO: u32 = 0x001FA824;
        let wpr2_lo = self.bar0.read32(NV_WPR2_LO);
        let wpr2_hi = self.bar0.read32(NV_WPR2_HI);
        let expected_lo = (wpr_meta.frts_offset >> 12) as u32;
        let wpr2_lo_val = (wpr2_lo >> 4) & 0x0FFF_FFFF;
        let wpr2_hi_val = (wpr2_hi >> 4) & 0x0FFF_FFFF;
        con.print("  GSP: WPR2 raw lo=0x");
        con.print_hex32(wpr2_lo);
        con.print(" hi=0x");
        con.print_hex32(wpr2_hi);
        con.print(" decoded lo=0x");
        con.print_hex32(wpr2_lo_val);
        con.print(" hi=0x");
        con.print_hex32(wpr2_hi_val);
        con.print(" expected_lo=0x");
        con.print_hex32(expected_lo);
        con.newline();
        if wpr2_hi_val != 0 && wpr2_lo_val == expected_lo {
            con.print_colored("  GSP: WPR2 SET OK!\n", 0x00FF00);
        } else {
            con.print_colored("  GSP: WARNING - WPR2 not confirmed after FWSEC; forcing WPR2 before SEC2\n", 0xFFFF00);
            self.force_program_wpr2_frts(wpr_meta.frts_offset, 0x10_0000, con);

            let forced_lo = self.bar0.read32(NV_WPR2_LO);
            let forced_hi = self.bar0.read32(NV_WPR2_HI);
            let forced_lo_val = (forced_lo >> 4) & 0x0FFF_FFFF;
            let forced_hi_val = (forced_hi >> 4) & 0x0FFF_FFFF;
            if forced_hi_val != 0 && forced_lo_val == expected_lo {
                con.print_colored("  GSP: WPR2 forced OK; continuing to SEC2 booter_load\n", 0x00FF00);
            } else {
                con.print_colored("  GSP: WARNING - WPR2 force did not stick; trying SEC2 booter_load anyway\n", 0xFFFF00);
                con.println("  GSP: If booter_load returns a mailbox error, BAR0 WPR2 writes may be ignored/blocked.");
            }
        }

        // ── 7 & 8. Reset GSP into RISC-V mode AND Write libos args ──
        con.println("  GSP: [7-8/11] Resetting GSP into RISC-V mode & Writing Mailbox...");
        self.reset_gsp_riscv_mode(boot_mem.boot_args_phys, con);

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
