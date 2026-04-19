//! FastOS Shell — Interactive command interpreter.
//! Ring 0, no_std. Runs in kernel space.

use crate::console::Console;
use crate::drivers::keyboard;
use crate::fb::colors;
use crate::arch::cpu;

const MAX_LINE: usize = 512;
const VERSION: &str = "FastOS v0.2.0";

/// Run the interactive shell loop (never returns).
pub fn run(con: &mut Console) {
    // Welcome banner
    con.set_color(colors::NV_GREEN);
    con.println("  _____          _    ___  ____  ");
    con.println(" |  ___|_ _ ___|| |_ / _ \\/ ___| ");
    con.println(" | |_ / _` / __|| __| | | \\___ \\ ");
    con.println(" |  _| (_| \\__ \\| |_| |_| |___) |");
    con.println(" |_|  \\__,_|___/ \\__|\\___/|____/ ");
    con.set_color(colors::TEXT_PRIMARY);
    con.println("");
    con.print_colored(VERSION, colors::ACCENT_CYAN);
    con.print(" - Bare Metal OS | Ring 0 | ");
    con.print_colored("Ryzen 5 5600X", colors::ACCENT_PURPLE);
    con.print(" + ");
    con.print_colored("RTX 3060 12G", colors::NV_GREEN);
    con.println("");
    con.print_colored("Type 'help' for commands.", colors::TEXT_SECONDARY);
    con.println("");
    con.println("");

    let mut line_buf = [0u8; MAX_LINE];

    loop {
        // Prompt
        con.print_colored("fastos", colors::NV_GREEN);
        con.print_colored("> ", colors::ACCENT_CYAN);
        con.draw_cursor(true);

        // Read line
        let len = read_line_interactive(con, &mut line_buf);

        // Parse and execute
        let cmd = &line_buf[..len];
        execute_command(con, cmd);
    }
}

/// Interactive line reading with echo and backspace.
fn read_line_interactive(con: &mut Console, buf: &mut [u8]) -> usize {
    let mut len = 0;

    loop {
        let key = keyboard::read_key();

        match key {
            b'\n' => {
                con.draw_cursor(false);
                con.newline();
                return len;
            }
            8 => {
                // Backspace
                if len > 0 {
                    len -= 1;
                    con.draw_cursor(false);
                    con.backspace();
                    con.draw_cursor(true);
                }
            }
            32..=126 => {
                // Printable character
                if len < buf.len() - 1 {
                    buf[len] = key;
                    len += 1;
                    con.draw_cursor(false);
                    con.put_char(key);
                    con.draw_cursor(true);
                }
            }
            _ => {} // Ignore non-printable
        }
    }
}

/// Execute a command.
fn execute_command(con: &mut Console, cmd: &[u8]) {
    // Skip empty
    let trimmed = trim(cmd);
    if trimmed.is_empty() { return; }

    // Match commands
    if eq(trimmed, b"help") {
        cmd_help(con);
    } else if eq(trimmed, b"clear") {
        con.clear();
    } else if eq(trimmed, b"cpuinfo") {
        cmd_cpuinfo(con);
    } else if eq(trimmed, b"gpuinfo") {
        cmd_gpuinfo(con);
    } else if eq(trimmed, b"pci") {
        cmd_pci(con);
    } else if eq(trimmed, b"meminfo") {
        cmd_meminfo(con);
    } else if eq(trimmed, b"uptime") {
        cmd_uptime(con);
    } else if eq(trimmed, b"reboot") {
        cmd_reboot();
    } else if eq(trimmed, b"halt") {
        cmd_halt(con);
    } else if eq(trimmed, b"ver") || eq(trimmed, b"version") {
        con.print_colored(VERSION, colors::ACCENT_CYAN);
        con.println("");
    } else {
        con.print_colored("Unknown command: ", colors::ACCENT_RED);
        print_bytes(con, trimmed);
        con.println("");
        con.print_colored("Type 'help' for commands.", colors::TEXT_SECONDARY);
        con.println("");
    }
}

// ── Commands ────────────────────────────────────────────────────────────────

fn cmd_help(con: &mut Console) {
    con.print_colored("FastOS Commands:\n", colors::ACCENT_BLUE);
    let cmds: &[(&str, &str)] = &[
        ("help",    "Show this help"),
        ("clear",   "Clear screen"),
        ("cpuinfo", "CPU features (CPUID)"),
        ("gpuinfo", "GPU info (NVIDIA GA106)"),
        ("pci",     "List PCI devices"),
        ("meminfo", "Memory information"),
        ("uptime",  "System uptime"),
        ("ver",     "Kernel version"),
        ("reboot",  "Reboot system"),
        ("halt",    "Halt CPU"),
    ];
    for &(name, desc) in cmds {
        con.print("  ");
        con.print_colored(name, colors::NV_GREEN);
        // Pad to 12 chars
        for _ in 0..(12 - name.len().min(12)) { con.put_char(b' '); }
        con.print_colored(desc, colors::TEXT_SECONDARY);
        con.println("");
    }
}

fn cmd_cpuinfo(con: &mut Console) {
    con.print_colored("CPU: ", colors::ACCENT_PURPLE);
    con.println("AMD Ryzen 5 5600X (Zen 3, Vermeer)");

    let cpu = cpu::detect_cpu();
    let features: &[(&str, bool)] = &[
        ("SSE4.2", cpu.has_sse42),
        ("AVX2",   cpu.has_avx2),
        ("FMA3",   cpu.has_fma3),
        ("AES-NI", cpu.has_aes),
        ("SHA",    cpu.has_sha),
        ("BMI2",   cpu.has_bmi2),
        ("RDRAND", cpu.has_rdrand),
        ("RDSEED", cpu.has_rdseed),
        ("NX",     cpu.has_nx),
    ];

    con.print("Features: ");
    for &(name, has) in features {
        if has {
            con.print_colored(name, colors::TEXT_SUCCESS);
        } else {
            con.print_colored(name, colors::ACCENT_RED);
        }
        con.print(" ");
    }
    con.println("");

    con.print("Cores: ");
    con.print_colored("6C/12T", colors::TEXT_PRIMARY);
    con.print("  Base: ");
    con.print_colored("3.7 GHz", colors::TEXT_PRIMARY);
    con.print("  Boost: ");
    con.print_colored("4.6 GHz", colors::TEXT_PRIMARY);
    con.println("");
}

fn cmd_gpuinfo(con: &mut Console) {
    con.print_colored("GPU: ", colors::ACCENT_PURPLE);
    con.println("NVIDIA GeForce RTX 3060 12GB");

    con.print("  Chip:     ");
    con.print_colored("GA106 (Ampere)", colors::TEXT_PRIMARY);
    con.println("");

    con.print("  VRAM:     ");
    con.print_colored("12288 MB GDDR6", colors::NV_GREEN);
    con.println("");

    con.print("  Bus:      ");
    con.println("PCIe 4.0 x16 @ bus 41");

    con.print("  VID:DID:  ");
    con.println("0x10DE:0x2504");

    con.print("  BAR0:     ");
    con.println("MMIO register space (16 MB)");

    con.print("  Display:  ");
    con.print_colored("VBE 1920x1080x32bpp @ 0xD0000000", colors::ACCENT_CYAN);
    con.println("");

    con.print("  Driver:   ");
    con.print_colored("Ring 0 loaded (SigDead-BIB nv_kernel)", colors::TEXT_SUCCESS);
    con.println("");
}

fn cmd_pci(con: &mut Console) {
    let devices = crate::drivers::pci::scan_pci_bus();
    con.print_colored("PCI Devices: ", colors::ACCENT_BLUE);
    con.print_u64(devices.count as u64);
    con.println("");

    // Show first devices with details
    for i in 0..devices.count.min(20) {
        let d = &devices.devices[i];
        con.print("  ");
        con.print_u64(d.bus as u64);
        con.print(":");
        con.print_u64(d.device as u64);
        con.print(".");
        con.print_u64(d.function as u64);
        con.print("  ");
        con.print_hex32(((d.vendor_id as u32) << 16) | d.device_id as u32);

        // Identify known vendors
        match d.vendor_id {
            0x10DE => con.print_colored("  NVIDIA", colors::NV_GREEN),
            0x1022 => con.print_colored("  AMD", colors::ACCENT_RED),
            0x1002 => con.print_colored("  AMD/ATI", colors::ACCENT_RED),
            0x8086 => con.print_colored("  Intel", colors::ACCENT_BLUE),
            0x1B21 => con.print_colored("  ASMedia", colors::TEXT_SECONDARY),
            0x1987 => con.print_colored("  Phison", colors::TEXT_SECONDARY),
            _ => {}
        }
        con.println("");
    }
    if devices.count > 20 {
        con.print_colored("  ... and more\n", colors::TEXT_SECONDARY);
    }
}

fn cmd_meminfo(con: &mut Console) {
    con.print_colored("Memory Layout:\n", colors::ACCENT_BLUE);
    con.println("  0x000000 - 0x007BFF  Reserved (IVT, BDA)");
    con.println("  0x007C00 - 0x007DFF  MBR (Stage1)");
    con.println("  0x007E00 - 0x00BDFF  Stage2 bootloader");
    con.println("  0x010000 - 0x0FFFFF  Kernel load buffer");
    con.println("  0x100000 - 0x3FFFFF  Kernel (1 MB - 4 MB)");
    con.println("  0x400000 - 0x7FFFFF  DMA buffer pool");
    con.println("  0x800000             Stack top (grows down)");
    con.print("  0xD0000000           ");
    con.print_colored("GPU Framebuffer (VBE LFB)", colors::ACCENT_CYAN);
    con.println("");
}

fn cmd_uptime(con: &mut Console) {
    let tsc = cpu::rdtsc();
    // Ryzen 5 5600X base clock ~3.7 GHz, estimate seconds
    let approx_secs = tsc / 3_700_000_000;
    con.print("Uptime: ~");
    con.print_u64(approx_secs);
    con.print(" seconds (TSC: ");
    con.print_u64(tsc);
    con.println(")");
}

fn cmd_reboot() {
    // Triple fault or keyboard controller reset
    unsafe {
        // Method 1: keyboard controller reset
        core::arch::asm!("out dx, al", in("dx") 0x64u16, in("al") 0xFEu8);
        // Method 2: triple fault (fallback)
        core::arch::asm!("lidt [{}]", in(reg) &0u64, options(nostack));
        core::arch::asm!("int3");
    }
}

fn cmd_halt(con: &mut Console) {
    con.print_colored("System halted.", colors::ACCENT_ORANGE);
    con.println("");
    loop { unsafe { core::arch::asm!("hlt"); } }
}

// ── Utilities ──────────────────────────────────────────────────────────────

fn trim(s: &[u8]) -> &[u8] {
    let mut start = 0;
    let mut end = s.len();
    while start < end && s[start] == b' ' { start += 1; }
    while end > start && s[end - 1] == b' ' { end -= 1; }
    &s[start..end]
}

fn eq(a: &[u8], b: &[u8]) -> bool {
    if a.len() != b.len() { return false; }
    for i in 0..a.len() {
        if a[i] != b[i] { return false; }
    }
    true
}

fn print_bytes(con: &mut Console, s: &[u8]) {
    for &b in s { con.put_char(b); }
}
