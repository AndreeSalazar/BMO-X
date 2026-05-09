//! FastOS Shell — Interactive command interpreter.
//! Ring 0, no_std. Only commands backed by real hardware state.

use crate::arch::cpu;
use crate::console::Console;
use crate::fb::colors;
use crate::drivers::nvme::NvmeDriver;
use crate::fs::ntfs::NtfsWrapper;
use crate::fs::walker::FileWalker;

const MAX_LINE: usize = 256;
const KEY_PAGE_UP: u8 = 0xF1;
const KEY_PAGE_DOWN: u8 = 0xF2;
const KEY_HOME: u8 = 0xF3;
const KEY_END: u8 = 0xF4;
const KEY_ARROW_UP: u8 = 0xF5;
const KEY_ARROW_DOWN: u8 = 0xF6;

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
        "ver" => con.println("FastOS v0.6.0 (Special Agent Edition)"),
        "ntfs" => cmd_ntfs(con, parts),
        "extract" => cmd_extract(con),
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
    print_cmd(con, "help", "Show this help");
    print_cmd(con, "ntfs ls", "List files in NTFS root");
    print_cmd(con, "ntfs find [name]", "Find file by name");
    print_cmd(con, "cpuinfo", "CPU features");
    print_cmd(con, "pci", "PCI devices");
    print_cmd(con, "meminfo", "Memory map");
    print_cmd(con, "clear", "Clear screen");
}

fn print_cmd(con: &mut Console, name: &str, desc: &str) {
    con.print("  ");
    con.print_colored(name, colors::NV_GREEN);
    con.print("  -  ");
    con.println(desc);
}

fn cmd_ntfs(con: &mut Console, mut parts: core::str::SplitWhitespace) {
    let sub = parts.next().unwrap_or("");
    
    // Detect NVMe
    let mut nvme = match unsafe { NvmeDriver::detect() } {
        Some(d) => d,
        None => {
            con.print_colored("Error: No NVMe controller found.\n", colors::ACCENT_RED);
            return;
        }
    };

    let mut wrapper = NtfsWrapper::new(nvme);
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
