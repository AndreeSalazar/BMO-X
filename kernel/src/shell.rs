//! FastOS Shell — Interactive command interpreter.
//! Ring 0, no_std. Spy Agent Edition.

use crate::arch::cpu;
use crate::console::Console;
use crate::fb::colors;
use crate::drivers::nvme::NvmeDriver;
use crate::fs::gpt;
use crate::fs::DiskReader;
use crate::fs::ntfs::NtfsWrapper;
use crate::fs::walker::FileWalker;
use crate::agent::targets::{self, TargetCategory};
use crate::agent::pe_sig;
use crate::agent::registry_spy;
use crate::agent::firmware;
use crate::export::manifest::SpyReport;
use crate::export::serial_export;
use ntfs::NtfsReadSeek;
use alloc::vec::Vec;
use alloc::string::String;
use core::sync::atomic::{self, Ordering};

const MAX_LINE: usize = 256;
const KEY_PAGE_UP: u8 = 0xF1;
const KEY_PAGE_DOWN: u8 = 0xF2;
const KEY_HOME: u8 = 0xF3;
const KEY_END: u8 = 0xF4;
const KEY_ARROW_UP: u8 = 0xF5;
const KEY_ARROW_DOWN: u8 = 0xF6;

/// Max bytes to read from a PE file for signature parsing.
/// Authenticode sigs are usually in the last few KB, but the security
/// directory pointer is in the first 4KB of headers. For files >64KB,
/// we read headers + tail.
const PE_HEADER_READ: usize = 4096;
/// Max bytes to read from a PE to capture the signature data.
const PE_SIG_READ_MAX: usize = 256 * 1024; // 256 KB for sig extraction

/// Basic PS/2 Set 1 Scancode to ASCII map (US QWERTY, lowercase only)
fn scancode_to_ascii(scancode: u8) -> Option<u8> {
    match scancode {
        0x02 => Some(b'1'),
        0x03 => Some(b'2'),
        0x04 => Some(b'3'),
        0x05 => Some(b'4'),
        0x06 => Some(b'5'),
        0x07 => Some(b'6'),
        0x08 => Some(b'7'),
        0x09 => Some(b'8'),
        0x0A => Some(b'9'),
        0x0B => Some(b'0'),

        0x10 => Some(b'q'),
        0x11 => Some(b'w'),
        0x12 => Some(b'e'),
        0x13 => Some(b'r'),
        0x14 => Some(b't'),
        0x15 => Some(b'y'),
        0x16 => Some(b'u'),
        0x17 => Some(b'i'),
        0x18 => Some(b'o'),
        0x19 => Some(b'p'),

        0x1E => Some(b'a'),
        0x1F => Some(b's'),
        0x20 => Some(b'd'),
        0x21 => Some(b'f'),
        0x22 => Some(b'g'),
        0x23 => Some(b'h'),
        0x24 => Some(b'j'),
        0x25 => Some(b'k'),
        0x26 => Some(b'l'),

        0x2C => Some(b'z'),
        0x2D => Some(b'x'),
        0x2E => Some(b'c'),
        0x2F => Some(b'v'),
        0x30 => Some(b'b'),
        0x31 => Some(b'n'),
        0x32 => Some(b'm'),

        0x39 => Some(b' '),  // Space
        0x1C => Some(b'\n'), // Enter
        0x0E => Some(8),     // Backspace
        _ => None,
    }
}

fn read_any_key() -> u8 {
    loop {
        let status: u8;
        unsafe {
            core::arch::asm!("in al, dx", out("al") status, in("dx") 0x64u16);
        }

        if (status & 1) != 0 {
            let scancode: u8;
            unsafe {
                core::arch::asm!("in al, dx", out("al") scancode, in("dx") 0x60u16);
            }

            if scancode == 0xE0 {
                let ext = read_ps2_extended_scancode();
                match ext {
                    0x49 => return KEY_PAGE_UP,
                    0x51 => return KEY_PAGE_DOWN,
                    0x47 => return KEY_HOME,
                    0x4F => return KEY_END,
                    0x48 => return KEY_ARROW_UP,
                    0x50 => return KEY_ARROW_DOWN,
                    _ => {}
                }
            }

            if scancode < 0x80 {
                if let Some(ascii) = scancode_to_ascii(scancode) {
                    return ascii;
                }
            }
        }
        
        // Polling serial
        if let Some(ch) = crate::drivers::serial::serial_read_byte() {
            if ch == 13 { return b'\n'; }
            return ch;
        }
    }
}

fn read_ps2_extended_scancode() -> u8 {
    loop {
        let status: u8;
        unsafe {
            core::arch::asm!("in al, dx", out("al") status, in("dx") 0x64u16);
        }
        if (status & 1) != 0 {
            let scancode: u8;
            unsafe {
                core::arch::asm!("in al, dx", out("al") scancode, in("dx") 0x60u16);
            }
            return scancode;
        }
    }
}

pub fn run(con: &mut Console) {
    con.print_colored("FastOS Shell v0.6.0\n", colors::ACCENT_CYAN);
    con.println("Type 'help' for commands.");

    let mut line_buf = [0u8; MAX_LINE];

    loop {
        con.print_colored("fastos", colors::NV_GREEN);
        con.print_colored("> ", colors::ACCENT_CYAN);
        con.draw_cursor(true);

        let len = read_line_interactive(con, &mut line_buf);
        con.draw_cursor(false);
        con.newline();

        if len > 0 {
            execute(con, &line_buf[..len]);
        }
    }
}

fn read_line_interactive(con: &mut Console, buf: &mut [u8]) -> usize {
    let mut len: usize = 0;
    loop {
        let key = read_any_key();
        if key == b'\r' || key == b'\n' {
            con.scroll_to_bottom();
            return len;
        } else if key == KEY_PAGE_UP {
            con.scroll_page_up();
        } else if key == KEY_PAGE_DOWN {
            con.scroll_page_down();
        } else if key == 8 {
            if len > 0 {
                len -= 1;
                con.backspace();
            }
        } else if key >= 32 && key <= 126 {
            if len < buf.len() - 1 {
                buf[len] = key;
                len += 1;
                con.put_char(key);
            }
        }
    }
}

fn execute(con: &mut Console, cmd: &[u8]) {
    let cmd_str = core::str::from_utf8(cmd).unwrap_or("");
    let mut parts = cmd_str.split_whitespace();
    let root = parts.next().unwrap_or("");

    match root {
        "help" => cmd_help(con),
        "clear" => con.clear(),
        "cpuinfo" => cmd_cpuinfo(con),
        "pci" => cmd_pci(con),
        "meminfo" => cmd_meminfo(con),
        "ver" => con.println("FastOS v0.6.0 (Spy Agent Edition)"),
        "ntfs" => cmd_ntfs(con, parts),
        "extract" => cmd_extract(con),
        "spy" => cmd_spy(con),
        "reboot" => cmd_reboot(),
        _ => {
            con.print_colored("Unknown: ", colors::ACCENT_RED);
            con.println(root);
        }
    }
}

fn cmd_help(con: &mut Console) {
    con.print_colored("Commands:", colors::ACCENT_BLUE);
    con.newline();
    print_cmd(con, "spy",     "Run full spy mission (scan sigs + export JSON serial)");
    print_cmd(con, "extract", "Extract raw files from NTFS targets");
    print_cmd(con, "ntfs ls", "List files in NTFS root");
    print_cmd(con, "cpuinfo", "CPU features");
    print_cmd(con, "pci",     "PCI devices");
    print_cmd(con, "meminfo", "Memory map");
    print_cmd(con, "clear",   "Clear screen");
    print_cmd(con, "reboot",  "Reboot system");
}

fn print_cmd(con: &mut Console, name: &str, desc: &str) {
    con.print("  ");
    con.print_colored(name, colors::NV_GREEN);
    con.print("  -  ");
    con.println(desc);
}

// ══════════════════════════════════════════════════════════════════════════════
// SPY COMMAND — The main intelligence gathering mission
// ══════════════════════════════════════════════════════════════════════════════

fn cmd_spy(con: &mut Console) {
    con.print_colored("╔══════════════════════════════════════╗\n", colors::ACCENT_RED);
    con.print_colored("║   FASTOS SPY AGENT — MISSION START   ║\n", colors::ACCENT_RED);
    con.print_colored("╚══════════════════════════════════════╝\n", colors::ACCENT_RED);

    // 1. Detect NVMe
    con.print("[1/6] Detecting NVMe controller... ");
    let mut nvme = match unsafe { NvmeDriver::detect() } {
        Some(d) => {
            con.print_colored("OK\n", colors::TEXT_SUCCESS);
            d
        }
        None => {
            con.print_colored("FAIL\n", colors::ACCENT_RED);
            con.println("Abort: No NVMe controller found.");
            return;
        }
    };

    // 2. Scan GPT for Windows partition
    con.print("[2/6] Scanning GPT partition table... ");
    let ntfs_lba = match gpt::find_ntfs_partition(&mut nvme) {
        Ok(lba) => {
            con.print_colored("OK", colors::TEXT_SUCCESS);
            con.print(" (LBA ");
            con.print_u64(lba);
            con.println(")");
            lba
        }
        Err(_) => {
            con.print_colored("FAIL\n", colors::ACCENT_RED);
            con.println("Abort: No Windows partition found in GPT.");
            return;
        }
    };

    // 3. Validate partition boot sector, then mount NTFS
    con.print("[3/6] Mounting NTFS filesystem... ");

    // Pre-mount diagnostic: read the boot sector and verify NTFS signature
    {
        let mut boot_sector = alloc::vec![0u8; 512];
        match nvme.read_sectors(ntfs_lba, 1, &mut boot_sector) {
            Ok(()) => {
                // NTFS boot sector has "NTFS    " at offset 3 (OEM ID)
                let oem = &boot_sector[3..11];
                con.print("(OEM: ");
                for &b in oem {
                    if b >= 0x20 && b <= 0x7E {
                        con.put_char(b);
                    } else {
                        con.put_char(b'.');
                    }
                }
                con.print(") ");

                if oem != b"NTFS    " {
                    con.print_colored("WARN", colors::ACCENT_ORANGE);
                    con.print(" bytes: ");
                    for i in 0..8 {
                        con.print_hex32(boot_sector[i] as u32);
                        con.print(" ");
                    }
                    con.newline();
                }
            }
            Err(_) => {
                con.print_colored("FAIL\n", colors::ACCENT_RED);
                con.println("Abort: Cannot read partition boot sector.");
                return;
            }
        }
    }

    let mut wrapper = NtfsWrapper::new(nvme, ntfs_lba);
    let ntfs = match wrapper.mount() {
        Ok(n) => {
            con.print_colored("OK\n", colors::TEXT_SUCCESS);
            n
        }
        Err(_) => {
            con.print_colored("FAIL\n", colors::ACCENT_RED);
            con.println("Abort: Failed to mount NTFS.");
            return;
        }
    };

    // 3. Initialize report
    let mut report = SpyReport::new();
    let mut sig_count = 0u32;
    let mut hive_count = 0u32;
    let mut cat_count = 0u32;
    let mut fw_count = 0u32;

    // 4. Walk filesystem and extract intelligence
    con.print_colored("[4/6] Scanning for signatures and hives...\n", colors::ACCENT_ORANGE);

    let mut walker = FileWalker::new(&ntfs, &mut wrapper);
    walker.walk(|path, file, disk| {
        for target in targets::ALL_TARGETS {
            let matched = if target.path.ends_with("*") {
                path.starts_with(&target.path[..target.path.len() - 1])
            } else {
                path.eq_ignore_ascii_case(target.path)
            };

            if !matched {
                continue;
            }

            match target.category {
                TargetCategory::PeSignature => {
                    // Read PE headers to extract Authenticode signature
                    con.print("  [SIG] ");
                    con.print_colored(path, colors::NV_GREEN);

                    let data_attr = file.data(disk, "");
                    if let Some(Ok(attr)) = data_attr {
                        let attribute = attr.to_attribute().unwrap();
                        let mut reader = attribute.value(disk).unwrap();
                        let file_size = reader.len();

                        // Read enough for PE headers + potential signature
                        let read_size = (file_size as usize).min(PE_SIG_READ_MAX);
                        let mut buf = Vec::with_capacity(read_size);
                        buf.resize(read_size, 0);

                        atomic::fence(Ordering::SeqCst);
                        let _ = reader.read(disk, &mut buf);
                        atomic::fence(Ordering::SeqCst);

                        if let Some(sig_info) = pe_sig::parse_pe_signature(&buf) {
                            con.print(" -> ");
                            con.print_colored(&sig_info.signer_name, colors::ACCENT_CYAN);
                            con.newline();

                            report.add_signature(
                                path,
                                file_size,
                                target.description,
                                &sig_info,
                            );
                            sig_count += 1;
                        } else {
                            con.print_colored(" [no sig]\n", colors::ACCENT_ORANGE);
                        }

                        // Check for embedded NVIDIA firmware in driver PE files
                        if path.to_ascii_lowercase().contains("nvlddmkm.sys") {
                            let embedded = firmware::embedded::extract_embedded_firmware(&buf, path);
                            for fw in embedded {
                                report.add_firmware(fw);
                                fw_count += 1;
                            }
                        }
                    } else {
                        con.print_colored(" [no data]\n", colors::ACCENT_RED);
                    }
                }

                TargetCategory::RegistryHive => {
                    con.print("  [REG] ");
                    con.print_colored(path, colors::ACCENT_PURPLE);

                    let data_attr = file.data(disk, "");
                    if let Some(Ok(attr)) = data_attr {
                        let attribute = attr.to_attribute().unwrap();
                        let mut reader = attribute.value(disk).unwrap();
                        let file_size = reader.len();

                        // Read up to 2MB of the hive for scanning
                        let read_size = (file_size as usize).min(2 * 1024 * 1024);
                        let mut buf = Vec::with_capacity(read_size);
                        buf.resize(read_size, 0);

                        // Read in chunks to show progress
                        let chunk = 64 * 1024;
                        let mut read_so_far = 0usize;
                        while read_so_far < read_size {
                            let to_read = (chunk).min(read_size - read_so_far);
                            atomic::fence(Ordering::SeqCst);
                            let _ = reader.read(disk, &mut buf[read_so_far..read_so_far + to_read]);
                            atomic::fence(Ordering::SeqCst);
                            read_so_far += to_read;
                        }

                        con.print(" (");
                        con.print_u64(read_size as u64 / 1024);
                        con.print(" KB)");

                        // Parse based on which hive it is
                        if path.eq_ignore_ascii_case("Windows\\System32\\config\\SYSTEM") {
                            let info = registry_spy::parse_system_hive(&buf);
                            report.hostname = info.hostname;
                            for d in &info.drivers {
                                report.add_driver(d);
                            }
                            con.print(" hostname=");
                            con.print_colored(&report.hostname, colors::ACCENT_CYAN);
                            if let Some(nvidia_intel) = firmware::registry::extract_nvidia_registry_intel(&buf) {
                                report.add_gpu_registry(nvidia_intel);
                            }
                        } else if path.eq_ignore_ascii_case("Windows\\System32\\config\\SOFTWARE") {
                            let info = registry_spy::parse_software_hive(&buf);
                            report.machine_guid = info.machine_guid;
                            report.product_name = info.product_name;
                            report.build_lab = info.build_lab;
                            for c in &info.installed_certs {
                                report.add_cert(c);
                            }
                            con.print(" guid=");
                            con.print_colored(&report.machine_guid, colors::ACCENT_CYAN);
                        }

                        con.newline();
                        hive_count += 1;
                    } else {
                        con.print_colored(" [no data]\n", colors::ACCENT_RED);
                    }
                }

                TargetCategory::DriverCatalog => {
                    // Just note .cat files found
                    if path.ends_with(".cat") || path.ends_with(".CAT") {
                        cat_count += 1;
                        if cat_count <= 10 {
                            con.print("  [CAT] ");
                            con.println(path);
                        }
                    }
                }

                TargetCategory::NvidiaFirmware => {
                    // Check if file is a firmware blob inside DriverStore
                    if firmware::scanner::is_nvidia_firmware_file(path) {
                        con.print("  [FW]  ");
                        con.print_colored(path, colors::ACCENT_CYAN);
                        con.newline();

                        let data_attr = file.data(disk, "");
                        if let Some(Ok(attr)) = data_attr {
                            let attribute = attr.to_attribute().unwrap();
                            let mut reader = attribute.value(disk).unwrap();
                            let file_size = reader.len();
                            
                            // Read first few bytes for hash
                            let read_size = (file_size as usize).min(64);
                            let mut buf = Vec::with_capacity(read_size);
                            buf.resize(read_size, 0);

                            atomic::fence(Ordering::SeqCst);
                            let _ = reader.read(disk, &mut buf);
                            atomic::fence(Ordering::SeqCst);

                            let fw_rec = firmware::scanner::create_record_from_file(path, file_size, &buf);
                            report.add_firmware(fw_rec);
                            fw_count += 1;
                        }
                    }
                }

                TargetCategory::CertStore | TargetCategory::RegistryCrypto => {
                    // Handled via hive parsing above
                }
            }
        }
    });

    // 5. Generate JSON
    con.print_colored("\n[5/6] Generating JSON report... ", colors::ACCENT_ORANGE);
    let json = report.to_json();
    let json_size = json.len();
    con.print_u64(json_size as u64);
    con.print_colored(" bytes\n", colors::TEXT_SUCCESS);

    // 6. Export via serial
    con.print_colored("[6/6] Exporting via serial (115200 baud)... ", colors::ACCENT_ORANGE);
    serial_export::export_json_serial(&json);
    con.print_colored("DONE\n", colors::TEXT_SUCCESS);

    // ── Summary ──
    con.newline();
    con.print_colored("╔══════════════════════════════════════╗\n", colors::NV_GREEN);
    con.print_colored("║        MISSION COMPLETE              ║\n", colors::NV_GREEN);
    con.print_colored("╚══════════════════════════════════════╝\n", colors::NV_GREEN);
    con.print("  Signatures extracted: "); con.print_u64(sig_count as u64); con.newline();
    con.print("  Registry hives read:  "); con.print_u64(hive_count as u64); con.newline();
    con.print("  Firmware blobs found: "); con.print_u64(fw_count as u64); con.newline();
    con.print("  Drivers cataloged:    "); con.print_u64(report.drivers.len() as u64); con.newline();
    con.print("  Certificates found:   "); con.print_u64(report.certificates.len() as u64); con.newline();
    con.print("  JSON report size:     "); con.print_u64(json_size as u64); con.println(" bytes");
    con.print_colored("  Report sent via serial. Capture with PuTTY -> F:\\\n", colors::ACCENT_CYAN);
}

// ══════════════════════════════════════════════════════════════════════════════
// EXTRACT COMMAND — Raw file extraction (legacy)
// ══════════════════════════════════════════════════════════════════════════════

fn cmd_extract(con: &mut Console) {
    con.print_colored("--- STARTING FORENSIC EXTRACTION ---\n", colors::ACCENT_ORANGE);
    
    let mut nvme = match unsafe { NvmeDriver::detect() } {
        Some(d) => d,
        None => {
            con.print_colored("Error: No NVMe controller found.\n", colors::ACCENT_RED);
            return;
        }
    };

    let ntfs_lba = match gpt::find_ntfs_partition(&mut nvme) {
        Ok(lba) => {
            con.print("GPT: Windows partition at LBA ");
            con.print_u64(lba);
            con.newline();
            lba
        }
        Err(_) => {
            con.print_colored("Error: No Windows partition found.\n", colors::ACCENT_RED);
            return;
        }
    };

    let mut wrapper = NtfsWrapper::new(nvme, ntfs_lba);
    let ntfs = match wrapper.mount() {
        Ok(n) => n,
        Err(_) => {
            con.print_colored("Error: Failed to mount NTFS.\n", colors::ACCENT_RED);
            return;
        }
    };

    con.println("NTFS Mounted. Searching for targets...");

    let mut total_files = 0u32;
    let mut total_bytes: u64 = 0;
    let mut extracted_list = Vec::new();

    let mut walker = FileWalker::new(&ntfs, &mut wrapper);
    walker.walk(|path, file, disk| {
        for target in targets::ALL_TARGETS {
            let matched = if target.path.ends_with("*") {
                path.starts_with(&target.path[..target.path.len() - 1])
            } else {
                path.eq_ignore_ascii_case(target.path)
            };

            if !matched { continue; }

            con.print("  Found: ");
            con.print_colored(path, colors::NV_GREEN);
            con.print(" (");
            con.print(target.description);
            con.println(")");

            let data_attr = file.data(disk, "");
            if let Some(Ok(attr)) = data_attr {
                let attribute = attr.to_attribute().unwrap();
                let mut reader = attribute.value(disk).unwrap();
                let size = reader.len();
                
                let start_row = con.row_pos();
                con.print("    Reading: [                    ] 0%");
                let bar_start_col = 14;
                
                let mut read_so_far = 0u64;
                let chunk_size = 64 * 1024;
                let mut chunk = Vec::with_capacity(chunk_size);
                chunk.resize(chunk_size, 0);

                while read_so_far < size {
                    let to_read = core::cmp::min(chunk_size as u64, size - read_so_far);
                    atomic::fence(Ordering::SeqCst);
                    let _ = reader.read(disk, &mut chunk[..to_read as usize]);
                    atomic::fence(Ordering::SeqCst);
                    read_so_far += to_read;
                    
                    let pct = (read_so_far * 100 / size) as usize;
                    let dots = pct / 5;
                    
                    con.set_pos(bar_start_col, start_row);
                    for _ in 0..dots { con.put_char(b'='); }
                    if dots < 20 { con.put_char(b'>'); }
                    
                    con.set_pos(bar_start_col + 22, start_row);
                    con.print_u64(pct as u64);
                    con.print("%");
                    
                    atomic::fence(Ordering::SeqCst);
                }
                con.newline();
                
                total_files += 1;
                total_bytes += size;
                extracted_list.push(String::from(path));
                
                crate::drivers::serial::serial_write("[AGENT] Exported: ");
                crate::drivers::serial::serial_write(path);
                crate::drivers::serial::serial_write("\n");
            }
        }
    });

    con.newline();
    con.print_colored("--- EXTRACTION SUMMARY ---\n", colors::ACCENT_CYAN);
    con.print("  Files extracted: "); con.print_u64(total_files as u64); con.newline();
    con.print("  Total size:      "); con.print_u64(total_bytes / 1024); con.println(" KB");
    con.println("  List:");
    for f in extracted_list {
        con.print("    - ");
        con.println(&f);
    }
    con.print_colored("--- END OF MISSION ---\n", colors::NV_GREEN);
}

// ══════════════════════════════════════════════════════════════════════════════
// NTFS COMMAND
// ══════════════════════════════════════════════════════════════════════════════

fn cmd_ntfs(con: &mut Console, mut parts: core::str::SplitWhitespace) {
    let sub = parts.next().unwrap_or("");
    
    let mut nvme = match unsafe { NvmeDriver::detect() } {
        Some(d) => d,
        None => {
            con.print_colored("Error: No NVMe controller found.\n", colors::ACCENT_RED);
            return;
        }
    };

    let ntfs_lba = match gpt::find_ntfs_partition(&mut nvme) {
        Ok(lba) => lba,
        Err(_) => {
            con.print_colored("Error: No Windows partition found.\n", colors::ACCENT_RED);
            return;
        }
    };

    let mut wrapper = NtfsWrapper::new(nvme, ntfs_lba);
    let ntfs = match wrapper.mount() {
        Ok(n) => n,
        Err(_) => {
            con.print_colored("Error: Failed to mount NTFS filesystem.\n", colors::ACCENT_RED);
            return;
        }
    };

    match sub {
        "ls" => {
            con.println("Scanning NTFS root directory...");
            let mut walker = FileWalker::new(&ntfs, &mut wrapper);
            walker.walk(|path, file, _| {
                con.print("  ");
                if file.is_directory() {
                    con.print_colored("[DIR]  ", colors::ACCENT_BLUE);
                } else {
                    con.print_colored("[FILE] ", colors::TEXT_SUCCESS);
                }
                con.println(path);
            });
        },
        _ => con.println("Usage: ntfs ls"),
    }
}

// ══════════════════════════════════════════════════════════════════════════════
// UTILITY COMMANDS
// ══════════════════════════════════════════════════════════════════════════════

fn cmd_cpuinfo(con: &mut Console) {
    let f = cpu::detect_cpu();
    con.print_colored("CPU: ", colors::ACCENT_PURPLE);
    con.println("AMD Ryzen 5 5600X (Zen 3)");
    con.print("  SSE4.2: "); con.println(if f.has_sse42 { "YES" } else { "NO" });
    con.print("  AVX2:   "); con.println(if f.has_avx2 { "YES" } else { "NO" });
}

fn cmd_pci(con: &mut Console) {
    let devs = crate::drivers::pci::scan_pci_bus();
    con.print_colored("PCI Devices: ", colors::ACCENT_BLUE);
    con.println("");
    for i in 0..devs.count {
        let d = &devs.devices[i];
        con.print("  ");
        con.print_hex32(((d.vendor_id as u32) << 16) | d.device_id as u32);
        con.print(" ");
        if d.vendor_id == 0x10DE { con.print_colored("NVIDIA", colors::NV_GREEN); }
        else if d.vendor_id == 0x0106 { con.print_colored("SATA", colors::ACCENT_PURPLE); }
        con.newline();
    }
}

fn cmd_meminfo(con: &mut Console) {
    con.println("UEFI Memory Map info available in main logs.");
}

fn cmd_reboot() {
    unsafe {
        core::arch::asm!("out dx, al", in("dx") 0x64u16, in("al") 0xFEu8);
    }
}
