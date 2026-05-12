use crate::console::Console;
use crate::drivers::gpu::fastgpu::hw::mmio::Mmio;
use alloc::vec::Vec;

const OP_WRITE32: u8 = 0x01;
const OP_POLL32: u8 = 0x02;
const OP_WRITE_BLOCK: u8 = 0x03;
const OP_SETUP_WPR2: u8 = 0x04;
const OP_READ32: u8 = 0x05;
const OP_FALCON_DMA: u8 = 0x06;

/// Execute a FOSB payload from raw bytes (no NTFS, no filesystem).
/// `payload_data` must contain the full fastos_boot.bin content.
pub fn execute_from_bytes(
    con: &mut Console,
    payload_data: &[u8],
    mmio: &mut Mmio,
) {
    if payload_data.len() < 12 {
        con.println("[PAYLOAD] ERROR: Data too small for FOSB header.");
        return;
    }

    // Validate Header: "FOSB"
    if &payload_data[0..4] != b"FOSB" {
        con.print("[PAYLOAD] ERROR: Invalid magic: ");
        for i in 0..4 { con.print_hex32(payload_data[i] as u32); con.print(" "); }
        con.println("");
        return;
    }

    let version = u32::from_le_bytes(payload_data[4..8].try_into().unwrap());
    let num_entries = u32::from_le_bytes(payload_data[8..12].try_into().unwrap());

    con.print("  -> Valid FOSB (v");
    con.print_u64(version as u64);
    con.print(", ");
    con.print_u64(num_entries as u64);
    con.println(" entries)");

    let mut offset = 12;
    for i in 0..num_entries {
        if offset >= payload_data.len() {
            con.println("[PAYLOAD] ERROR: Unexpected EOF!");
            break;
        }

        let opcode = payload_data[offset];
        offset += 1;

        if offset + 8 > payload_data.len() {
            con.println("[PAYLOAD] ERROR: Truncated entry!");
            break;
        }

        let reg = u32::from_le_bytes(payload_data[offset..offset+4].try_into().unwrap());
        offset += 4;
        let size = u32::from_le_bytes(payload_data[offset..offset+4].try_into().unwrap());
        offset += 4;

        if offset + size as usize > payload_data.len() {
            con.println("[PAYLOAD] ERROR: Truncated data!");
            break;
        }

        con.print("  ["); con.print_u64(i as u64 + 1); con.print("/"); con.print_u64(num_entries as u64); con.print("] ");

        match opcode {
            OP_WRITE32 => {
                if size != 4 {
                    con.println("ERROR: WRITE32 requires size=4");
                    break;
                }
                let val = u32::from_le_bytes(payload_data[offset..offset+4].try_into().unwrap());
                con.print("WRITE32 0x"); con.print_hex32(reg);
                con.print(" <- 0x"); con.print_hex32(val); con.println("");
                mmio.write32(reg, val);
            }
            OP_POLL32 => {
                if size != 8 {
                    con.println("ERROR: POLL32 requires size=8");
                    break;
                }
                let mask = u32::from_le_bytes(payload_data[offset..offset+4].try_into().unwrap());
                let expected = u32::from_le_bytes(payload_data[offset+4..offset+8].try_into().unwrap());
                
                con.print("POLL32 0x"); con.print_hex32(reg);
                con.print(" & 0x"); con.print_hex32(mask);
                con.print(" == 0x"); con.print_hex32(expected); con.println("");
                
                let mut success = false;
                // 5 second rdtsc timeout (~3.7GHz Ryzen 5 5600X)
                let timeout_cycles: u64 = 18_500_000_000;
                let t0 = unsafe { core::arch::x86_64::_rdtsc() };
                let mut last_print = t0;
                loop {
                    let current_val = mmio.read32(reg);
                    if (current_val & mask) == expected {
                        success = true;
                        con.print("      -> SUCCESS (read: 0x"); con.print_hex32(current_val); con.println(")");
                        break;
                    }
                    let now = unsafe { core::arch::x86_64::_rdtsc() };
                    if now.wrapping_sub(t0) > timeout_cycles {
                        let final_val = mmio.read32(reg);
                        con.print("      -> TIMEOUT 5s (last: 0x"); con.print_hex32(final_val); con.println(")");
                        break;
                    }
                    if now.wrapping_sub(last_print) > 3_700_000_000 {
                        con.print("      -> waiting (read: 0x"); con.print_hex32(current_val); con.println(")...");
                        last_print = now;
                    }
                    core::hint::spin_loop();
                }
                if !success {
                    con.println("[PAYLOAD] Aborting — POLL32 timeout (PKC auth may have failed).");
                    break;
                }
            }
            OP_WRITE_BLOCK => {
                con.print("WRITE_BLOCK 0x"); con.print_hex32(reg);
                con.print(" <- "); con.print_u64(size as u64); con.println(" bytes");
                
                let mut data_offset = offset;
                let end_offset = offset + size as usize;
                
                while data_offset + 4 <= end_offset {
                    let val = u32::from_le_bytes(payload_data[data_offset..data_offset+4].try_into().unwrap());
                    mmio.write32(reg, val);
                    data_offset += 4;
                }
                if data_offset < end_offset {
                    con.println("      -> Warning: block not aligned to 4 bytes");
                }
            }
            OP_SETUP_WPR2 => {
                con.println("SETUP_WPR2 Macro");
                
                let vram_mb = mmio.read32(0x100800);
                con.print("  -> VRAM Size: "); con.print_u64(vram_mb as u64); con.println(" MB");
                
                let wpr2_size_mb = 128;
                let wpr2_start_mb = vram_mb.saturating_sub(wpr2_size_mb);
                let wpr2_start_64k = wpr2_start_mb << 4;
                let wpr2_end_64k = vram_mb << 4;
                
                con.print("  -> WPR2 Start: 0x"); con.print_hex32(wpr2_start_64k); con.println("");
                con.print("  -> WPR2 End:   0x"); con.print_hex32(wpr2_end_64k); con.println("");
                
                mmio.write32(0x100cd4, wpr2_start_64k);
                mmio.write32(0x100cd8, wpr2_end_64k);
            }
            OP_READ32 => {
                let val = mmio.read32(reg);
                con.print("READ32  0x"); con.print_hex32(reg);
                con.print(" => 0x"); con.print_hex32(val); con.println("");
            }
            OP_FALCON_DMA => {
                // DMA transfer firmware to Falcon IMEM or DMEM
                // GA102 correct bit encoding from dev_falcon_v4.h:
                //   FULL = bit 0 (read-only, queue full)
                //   IDLE = bit 1 (read-only, engine idle)
                //   SEC  = bits 3:2 (security mode)
                //   IMEM = bit 4 (1=IMEM, 0=DMEM)
                //   WRITE = bit 5 (0=to falcon, 1=from falcon)
                //   SIZE  = bits 10:8 (0x6 = 256 bytes)
                //   CTXDMA = bits 14:12
                //   DMATRFCMD at offset 0x118 (not 0x11C!)
                if (size as usize) < 8 {
                    con.println("ERROR: FALCON_DMA too small");
                    break;
                }
                let engine_base = u32::from_le_bytes(
                    payload_data[offset..offset+4].try_into().unwrap());
                let dma_cmd = u32::from_le_bytes(
                    payload_data[offset+4..offset+8].try_into().unwrap());
                let fw_data = &payload_data[offset+8..offset+(size as usize)];
                let fw_len = fw_data.len();

                let target_name = if (dma_cmd & 0x10) != 0 { "IMEM" } else { "DMEM" };
                con.print("FLC_DMA 0x"); con.print_hex32(engine_base);
                con.print(" "); con.print(target_name);
                con.print(" cmd=0x"); con.print_hex32(dma_cmd);
                con.print(" <- "); con.print_u64(fw_len as u64); con.println(" bytes");

                // Allocate page-aligned DMA buffer
                let page_size = 4096usize;
                let buf_size = (fw_len + page_size - 1) & !(page_size - 1);
                let layout = core::alloc::Layout::from_size_align(buf_size, page_size)
                    .expect("DMA layout");
                let dma_ptr = unsafe { alloc::alloc::alloc_zeroed(layout) };
                if dma_ptr.is_null() {
                    con.println("  -> ERROR: DMA alloc failed!");
                    break;
                }

                unsafe {
                    core::ptr::copy_nonoverlapping(fw_data.as_ptr(), dma_ptr, fw_len);
                }

                let phys_addr = dma_ptr as u64;
                con.print("  -> buf: 0x"); con.print_hex32((phys_addr >> 32) as u32);
                con.print_hex32(phys_addr as u32);
                con.print(" ("); con.print_u64(buf_size as u64); con.println(" bytes)");

                // GA102 register layout (dev_falcon_v4.h):
                let dmatrfbase   = engine_base + 0x110; // DMATRFBASE (phys addr >> 8)
                let dmatrfbase1  = engine_base + 0x128; // DMATRFBASE1 (high bits)
                let dmatrfmoffs  = engine_base + 0x114; // DMATRFMOFFS (falcon dest offset)
                let dmatrfcmd    = engine_base + 0x118; // DMATRFCMD (command, triggers DMA)
                let dmatrffboffs = engine_base + 0x11C; // DMATRFFBOFFS (source offset)

                let mut dma_ok = true;
                let timeout_cycles: u64 = 3_700_000_000;

                // Set DMA base address (physical_addr >> 8)
                mmio.write32(dmatrfbase, (phys_addr >> 8) as u32);
                mmio.write32(dmatrfbase1, ((phys_addr >> 40) & 0x1FF) as u32);

                // Initial poll: ensure request queue has space before first write
                let t0 = unsafe { core::arch::x86_64::_rdtsc() };
                loop {
                    let s = mmio.read32(dmatrfcmd);
                    if (s & 0x01) == 0 { break; } // FULL bit clear
                    if unsafe { core::arch::x86_64::_rdtsc() }.wrapping_sub(t0) > timeout_cycles {
                        con.print("  -> INITIAL FULL TIMEOUT cmd=0x"); con.print_hex32(s); con.println("");
                        dma_ok = false;
                        break;
                    }
                    core::hint::spin_loop();
                }

                let num_chunks = (fw_len + 255) / 256;
                con.print("  -> "); con.print_u64(num_chunks as u64);
                con.println(" chunks...");

                for chunk_idx in 0..num_chunks {
                    if !dma_ok { break; }
                    let falcon_offset = (chunk_idx * 256) as u32;

                    // Poll FULL=FALSE before each chunk (nvidia-open s_dmaTransfer_GA102)
                    let t0 = unsafe { core::arch::x86_64::_rdtsc() };
                    loop {
                        let status = mmio.read32(dmatrfcmd);
                        if (status & 0x01) == 0 { break; } // NOT_FULL
                        let now = unsafe { core::arch::x86_64::_rdtsc() };
                        if now.wrapping_sub(t0) > timeout_cycles {
                            con.print("  -> FULL TIMEOUT chunk ");
                            con.print_u64(chunk_idx as u64);
                            con.print(" cmd=0x"); con.print_hex32(status); con.println("");
                            dma_ok = false;
                            break;
                        }
                        core::hint::spin_loop();
                    }
                    if !dma_ok { break; }

                    // Write MOFFS (falcon dest), FBOFFS (source), CMD (trigger)
                    // Exact order from nvidia-open s_dmaTransfer_GA102
                    mmio.write32(dmatrfmoffs, falcon_offset);
                    mmio.write32(dmatrffboffs, falcon_offset);
                    mmio.write32(dmatrfcmd, dma_cmd);

                    // Progress every 64 chunks
                    if chunk_idx > 0 && chunk_idx % 64 == 0 {
                        con.print("  -> "); con.print_u64(chunk_idx as u64);
                        con.print("/"); con.print_u64(num_chunks as u64); con.println("");
                    }
                }

                // Poll IDLE (bit 1) after all chunks (nvidia-open requirement)
                if dma_ok {
                    let t0 = unsafe { core::arch::x86_64::_rdtsc() };
                    loop {
                        let status = mmio.read32(dmatrfcmd);
                        if (status & 0x02) != 0 { break; } // IDLE = true
                        let now = unsafe { core::arch::x86_64::_rdtsc() };
                        if now.wrapping_sub(t0) > timeout_cycles {
                            con.print("  -> IDLE TIMEOUT cmd=0x");
                            con.print_hex32(status); con.println("");
                            dma_ok = false;
                            break;
                        }
                        core::hint::spin_loop();
                    }
                }

                if dma_ok {
                    con.print("  -> DMA OK: "); con.print_u64(fw_len as u64);
                    con.print(" bytes -> "); con.print(target_name); con.println("");
                }

                unsafe { alloc::alloc::dealloc(dma_ptr, layout); }
            }
            _ => {
                con.print("UNKNOWN OPCODE: 0x"); con.print_hex32(opcode as u32); con.println("");
                break;
            }
        }

        offset += size as usize;
    }

    con.println("[PAYLOAD] Sequence execution finished.");
}
