//! FastOS Shell — Interactive command interpreter.
//! Ring 0, no_std. Simple and reliable.

use crate::console::Console;
use crate::drivers::keyboard;
use crate::fb::colors;
use crate::arch::cpu;

const MAX_LINE: usize = 256;

/// Run the interactive shell loop (never returns).
pub fn run(con: &mut Console) {
    // Simple welcome
    con.print_colored("FastOS v0.2.0", colors::NV_GREEN);
    con.print(" - Ring 0 | Ryzen 5 5600X + RTX 3060 12G");
    con.newline();
    con.print_colored("Type 'help' for commands.", colors::TEXT_SECONDARY);
    con.newline();
    con.newline();

    let mut line_buf = [0u8; MAX_LINE];

    loop {
        // Prompt
        con.print_colored("fastos", colors::NV_GREEN);
        con.print_colored("> ", colors::ACCENT_CYAN);
        con.draw_cursor(true);

        // Read line interactively
        let len = read_line_interactive(con, &mut line_buf);
        con.draw_cursor(false);
        con.newline();

        // Execute
        if len > 0 {
            execute(con, &line_buf[..len]);
        }
    }
}

fn read_line_interactive(con: &mut Console, buf: &mut [u8]) -> usize {
    let mut len: usize = 0;
    loop {
        let key = keyboard::read_key();
        if key == b'\n' {
            return len;
        } else if key == 8 {
            // Backspace
            if len > 0 {
                len -= 1;
                con.draw_cursor(false);
                con.backspace();
                con.draw_cursor(true);
            }
        } else if key >= 32 && key <= 126 {
            if len < buf.len() - 1 {
                buf[len] = key;
                len += 1;
                con.draw_cursor(false);
                con.put_char(key);
                con.draw_cursor(true);
            }
        }
    }
}

fn execute(con: &mut Console, cmd: &[u8]) {
    if bytes_eq(cmd, b"help") {
        cmd_help(con);
    } else if bytes_eq(cmd, b"clear") {
        con.clear();
    } else if bytes_eq(cmd, b"cpuinfo") {
        cmd_cpuinfo(con);
    } else if bytes_eq(cmd, b"gpuinfo") {
        cmd_gpuinfo(con);
    } else if bytes_eq(cmd, b"pci") {
        cmd_pci(con);
    } else if bytes_eq(cmd, b"meminfo") {
        cmd_meminfo(con);
    } else if bytes_eq(cmd, b"uptime") {
        cmd_uptime(con);
    } else if bytes_eq(cmd, b"reboot") {
        cmd_reboot();
    } else if bytes_eq(cmd, b"halt") {
        con.print_colored("System halted.", colors::ACCENT_ORANGE);
        con.newline();
        loop { unsafe { core::arch::asm!("hlt"); } }
    } else if bytes_eq(cmd, b"ver") {
        con.print_colored("FastOS v0.2.0 (Rust, no_std, Ring 0)", colors::ACCENT_CYAN);
        con.newline();
    } else {
        con.print_colored("Unknown: ", colors::ACCENT_RED);
        for &b in cmd { con.put_char(b); }
        con.newline();
    }
}

// ── Commands (all use simple println, no arrays) ────────────────────────────

fn cmd_help(con: &mut Console) {
    con.print_colored("Commands:", colors::ACCENT_BLUE);
    con.newline();
    print_cmd(con, "help", "Show this help");
    print_cmd(con, "cpuinfo", "CPU features");
    print_cmd(con, "gpuinfo", "GPU information");
    print_cmd(con, "pci", "List PCI devices");
    print_cmd(con, "meminfo", "Memory layout");
    print_cmd(con, "uptime", "System uptime");
    print_cmd(con, "clear", "Clear screen");
    print_cmd(con, "ver", "Kernel version");
    print_cmd(con, "reboot", "Reboot system");
    print_cmd(con, "halt", "Halt CPU");
}

fn print_cmd(con: &mut Console, name: &str, desc: &str) {
    con.print("  ");
    con.print_colored(name, colors::NV_GREEN);
    // Manual padding
    let mut pad = 12usize.saturating_sub(name.len());
    while pad > 0 { con.put_char(b' '); pad -= 1; }
    con.print(desc);
    con.newline();
}

fn cmd_cpuinfo(con: &mut Console) {
    let f = cpu::detect_cpu();
    con.print_colored("CPU: ", colors::ACCENT_PURPLE);
    con.println("AMD Ryzen 5 5600X (Zen 3)");
    con.print("  SSE4.2: "); print_yn(con, f.has_sse42);
    con.print("  AVX2:   "); print_yn(con, f.has_avx2);
    con.print("  FMA3:   "); print_yn(con, f.has_fma3);
    con.print("  AES-NI: "); print_yn(con, f.has_aes);
    con.print("  SHA:    "); print_yn(con, f.has_sha);
    con.print("  BMI2:   "); print_yn(con, f.has_bmi2);
    con.print("  RDRAND: "); print_yn(con, f.has_rdrand);
    con.print("  NX:     "); print_yn(con, f.has_nx);
}

fn cmd_gpuinfo(con: &mut Console) {
    con.print_colored("GPU: ", colors::ACCENT_PURPLE);
    con.println("NVIDIA GeForce RTX 3060 12GB");
    con.println("  Chip:   GA106 (Ampere A1)");
    con.println("  VRAM:   12288 MB GDDR6");
    con.println("  Bus:    PCIe 4.0 x16, bus 41");
    con.println("  VID:    0x10DE:0x2504");
    con.println("  BAR0:   MMIO 16 MB (registers)");
    con.print("  FB:     ");
    con.print_colored("VBE 1920x1080x32 @ 0xD0000000", colors::ACCENT_CYAN);
    con.newline();
    con.print("  Driver: ");
    con.print_colored("Ring 0 loaded", colors::TEXT_SUCCESS);
    con.newline();
}

fn cmd_pci(con: &mut Console) {
    let devs = crate::drivers::pci::scan_pci_bus();
    con.print_colored("PCI: ", colors::ACCENT_BLUE);
    con.print_u64(devs.count as u64);
    con.println(" devices");
    let max = if devs.count > 15 { 15 } else { devs.count };
    let mut i = 0;
    while i < max {
        let d = &devs.devices[i];
        con.print("  ");
        con.print_u64(d.bus as u64);
        con.put_char(b':');
        con.print_u64(d.device as u64);
        con.put_char(b'.');
        con.print_u64(d.function as u64);
        con.print("  ");
        con.print_hex32(((d.vendor_id as u32) << 16) | d.device_id as u32);
        if d.vendor_id == 0x10DE {
            con.print_colored(" NVIDIA", colors::NV_GREEN);
        } else if d.vendor_id == 0x1022 {
            con.print_colored(" AMD", colors::ACCENT_RED);
        }
        con.newline();
        i += 1;
    }
    if devs.count > 15 {
        con.println("  ...");
    }
}

fn cmd_meminfo(con: &mut Console) {
    con.print_colored("Memory:\n", colors::ACCENT_BLUE);
    con.println("  0x007C00  MBR (Stage1)");
    con.println("  0x007E00  Stage2 bootloader");
    con.println("  0x100000  Kernel (1 MB)");
    con.println("  0x400000  DMA buffer pool");
    con.println("  0x800000  Stack top");
    con.print("  0xD0000000  ");
    con.print_colored("GPU Framebuffer", colors::ACCENT_CYAN);
    con.newline();
}

fn cmd_uptime(con: &mut Console) {
    let tsc = cpu::rdtsc();
    let secs = tsc / 3_700_000_000;
    con.print("Uptime: ~");
    con.print_u64(secs);
    con.println(" seconds");
}

fn cmd_reboot() {
    unsafe {
        core::arch::asm!("out dx, al", in("dx") 0x64u16, in("al") 0xFEu8);
    }
}

// ── Helpers ─────────────────────────────────────────────────────────────────

fn print_yn(con: &mut Console, val: bool) {
    if val {
        con.print_colored("YES", colors::TEXT_SUCCESS);
    } else {
        con.print_colored("NO", colors::ACCENT_RED);
    }
    con.newline();
}

fn bytes_eq(a: &[u8], b: &[u8]) -> bool {
    // Trim spaces from a
    let mut start = 0;
    let mut end = a.len();
    while start < end && a[start] == b' ' { start += 1; }
    while end > start && a[end - 1] == b' ' { end -= 1; }
    let a = &a[start..end];
    if a.len() != b.len() { return false; }
    let mut i = 0;
    while i < a.len() {
        if a[i] != b[i] { return false; }
        i += 1;
    }
    true
}
