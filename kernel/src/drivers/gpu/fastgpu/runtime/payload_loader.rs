use crate::console::Console;
use crate::fs::ntfs::NtfsWrapper;
use crate::drivers::ahci::AhciDriver;
use crate::drivers::gpu::fastgpu::hw::mmio::Mmio;
use crate::fs::walker::FileWalker;
use ntfs::{Ntfs, NtfsReadSeek};
use alloc::vec::Vec;
use core::sync::atomic::{self, Ordering};

const OP_WRITE32: u8 = 0x01;
const OP_POLL32: u8 = 0x02;
const OP_WRITE_BLOCK: u8 = 0x03;
const OP_SETUP_WPR2: u8 = 0x04;

pub fn execute_payload(
    con: &mut Console,
    ntfs: &Ntfs,
    wrapper: &mut NtfsWrapper<AhciDriver>,
    mmio: &mut Mmio,
) {
    con.println("[PAYLOAD] Searching for fastos_boot.bin on SATA NTFS...");

    let mut found = false;
    let mut payload_data = Vec::new();

    let mut walker = FileWalker::new(ntfs, wrapper);
    walker.walk(|path, file, disk| {
        if path.eq_ignore_ascii_case("fastos_boot.bin") {
            found = true;
            let data_attr = file.data(disk, "");
            if let Some(Ok(attr)) = data_attr {
                let attribute = attr.to_attribute().unwrap();
                let mut reader = attribute.value(disk).unwrap();
                let size = reader.len() as usize;

                con.print("  -> Found fastos_boot.bin (");
                con.print_u64(size as u64);
                con.println(" bytes)");

                payload_data.resize(size, 0);
                atomic::fence(Ordering::SeqCst);
                let _ = reader.read(disk, &mut payload_data);
                atomic::fence(Ordering::SeqCst);
            }
        }
    });

    if !found || payload_data.is_empty() {
        con.println("[PAYLOAD] ERROR: fastos_boot.bin not found or empty!");
        return;
    }

    if payload_data.len() < 12 {
        con.println("[PAYLOAD] ERROR: File too small for FOSB header.");
        return;
    }

    // Validate Header: "FOSB"
    if &payload_data[0..4] != b"FOSB" {
        con.println("[PAYLOAD] ERROR: Invalid FOSB magic signature!");
        return;
    }

    let version = u32::from_le_bytes(payload_data[4..8].try_into().unwrap());
    let num_entries = u32::from_le_bytes(payload_data[8..12].try_into().unwrap());

    con.print("  -> Valid FOSB Header (v");
    con.print_u64(version as u64);
    con.print(", ");
    con.print_u64(num_entries as u64);
    con.println(" entries)");

    let mut offset = 12;
    for i in 0..num_entries {
        if offset >= payload_data.len() {
            con.println("[PAYLOAD] ERROR: Unexpected EOF in payload!");
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
            con.println("[PAYLOAD] ERROR: Truncated payload data!");
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
                // Timeout logic: 5 seconds timeout. 
                // Since we don't have a reliable timer in `no_std`, we'll do an iteration limit.
                // Assuming ~10M iterations per second.
                let max_iters = 50_000_000;
                for iter in 0..max_iters {
                    let current_val = mmio.read32(reg);
                    if (current_val & mask) == expected {
                        success = true;
                        con.print("      -> SUCCESS (read: 0x"); con.print_hex32(current_val); con.println(")");
                        break;
                    }
                    if iter % 10_000_000 == 0 && iter > 0 {
                        con.print("      -> waiting (read: 0x"); con.print_hex32(current_val); con.println(")...");
                    }
                }
                if !success {
                    let final_val = mmio.read32(reg);
                    con.print("      -> TIMEOUT (last read: 0x"); con.print_hex32(final_val); con.println(")");
                    con.println("[PAYLOAD] Aborting payload sequence due to POLL timeout.");
                    break;
                }
            }
            OP_WRITE_BLOCK => {
                con.print("WRITE_BLOCK 0x"); con.print_hex32(reg);
                con.print(" <- "); con.print_u64(size as u64); con.println(" bytes");
                
                let mut data_offset = offset;
                let end_offset = offset + size as usize;
                
                // Write block in chunks of 4 bytes
                while data_offset + 4 <= end_offset {
                    let val = u32::from_le_bytes(payload_data[data_offset..data_offset+4].try_into().unwrap());
                    mmio.write32(reg, val);
                    data_offset += 4;
                }
                // Handle remaining bytes if size is not multiple of 4 (rare for hardware)
                if data_offset < end_offset {
                    con.println("      -> Warning: block not perfectly aligned to 4 bytes");
                }
            }
            OP_SETUP_WPR2 => {
                con.println("SETUP_WPR2 Macro");
                
                // 1. Read VRAM size from 0x100800 (usually in MB)
                let vram_mb = mmio.read32(0x100800);
                con.print("  -> VRAM Size from 0x100800: "); con.print_u64(vram_mb as u64); con.println(" MB");
                
                // 2. Calculate WPR2 (reserve last 128MB)
                let wpr2_size_mb = 128;
                let wpr2_start_mb = vram_mb.saturating_sub(wpr2_size_mb);
                
                // PGC6 WPR2 registers expect addresses shifted by 16 (64KB pages)
                let wpr2_start_64k = wpr2_start_mb << 4; // MB * 1024 * 1024 / 65536 = MB * 16 = MB << 4
                let wpr2_end_64k = vram_mb << 4;
                
                con.print("  -> WPR2 Start (64K pages): 0x"); con.print_hex32(wpr2_start_64k); con.println("");
                con.print("  -> WPR2 End (64K pages):   0x"); con.print_hex32(wpr2_end_64k); con.println("");
                
                // 3. Write PGC6 limits
                mmio.write32(0x100cd4, wpr2_start_64k);
                mmio.write32(0x100cd8, wpr2_end_64k);
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
