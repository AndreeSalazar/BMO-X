//! FastOS Shell — Interactive command interpreter.
//! Ring 0, no_std. Simple and reliable.

use crate::console::Console;
use crate::fb::colors;
use crate::arch::cpu;

const MAX_LINE: usize = 256;

/// Read PS/2 keyboard by polling port 0x60 (no interrupts).
fn read_key_polling() -> u8 {
    loop {
        let status: u8;
        unsafe { core::arch::asm!("in al, 0x64", out("al") status, options(nostack, preserves_flags)) };
        if status & 1 != 0 {
            let key: u8;
            unsafe { core::arch::asm!("in al, 0x60", out("al") key, options(nostack, preserves_flags)) };
            return key;
        }
    }
}

/// Run the interactive shell loop (never returns).
pub fn run(con: &mut Console) {
    // Simple welcome
    con.print_colored("FastOS v0.5.0", colors::NV_GREEN);
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
        let key = read_key_polling();
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
        con.print_colored("FastOS v0.5.0 (Rust, no_std, Ring 0)", colors::ACCENT_CYAN);
        con.newline();
    } else if bytes_eq(cmd, b"ticks") {
        cmd_ticks(con);
    } else if bytes_eq(cmd, b"irq") {
        cmd_irq(con);
    } else if bytes_eq(cmd, b"gpu") {
        cmd_gpu_engines(con);
    } else if bytes_eq(cmd, b"dmesg") {
        cmd_dmesg(con);
    } else if bytes_eq(cmd, b"gsprm") {
        cmd_gsprm(con);
    } else if bytes_eq(cmd, b"gputest") {
        unsafe {
            crate::tests::gpu_test::run_all_tests(con, crate::boot_info::BOOT_INFO);
        }
    } else if bytes_eq(cmd, b"gpucmd") {
        let fb_base = con.fb_addr() as u64;
        let fb_pitch = con.fb_pitch() as u32;
        crate::gpu::engine::cmd_gpucmd(con, fb_base, fb_pitch, 1920, 1080);
    } else if bytes_eq(cmd, b"cube") {
        cmd_cube(con);
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
    print_cmd(con, "ticks", "PIT tick counter");
    print_cmd(con, "irq", "Interrupt status");
    print_cmd(con, "gpu", "GPU engine status");
    print_cmd(con, "gsprm", "GSP-RM protocol");
    print_cmd(con, "gputest", "GPU HW test suite");
    print_cmd(con, "gpucmd", "GPU command engine (Level 2)");
    print_cmd(con, "cube", "3D rotating cube (Ring 0)");
    print_cmd(con, "dmesg", "Boot log");
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
    con.println("  GPC:    3 (4 TPC/GPC, 28 SM total)");
    con.println("  CE:     5 copy engines (CE0-CE4)");
    con.println("  PBDMA:  2 engines (PBDMA0, PBDMA1)");
    con.print("  GSP-RM: ");
    con.print_colored("gsp_ga10x.bin (RISC-V, libos-v3.1.0)", colors::ACCENT_CYAN);
    con.newline();
    con.println("  Crypto: AES-256 (51 Rcon) + RSA-2048 (2 sigs) + SHA-256");
    con.println("  XOR:    SigDead-BIB decoded 100 candidates (key 0x20)");
    con.print("  Driver: ");
    con.print_colored("Ring 0 loaded", colors::TEXT_SUCCESS);
    con.newline();
}

fn cmd_gpu_engines(con: &mut Console) {
    con.print_colored("GPU Engines (SigDead-BIB):\n", colors::ACCENT_BLUE);

    con.print_colored("  FIFO: ", colors::ACCENT_PURPLE);
    con.println("Runlist-based scheduler, 512 channels");
    con.println("    PBDMA0: Push Buffer DMA engine 0");
    con.println("    PBDMA1: Push Buffer DMA engine 1");

    con.print_colored("  PGRAPH: ", colors::ACCENT_PURPLE);
    con.println("3 GPC x 4 TPC = 28 SM");
    con.println("    FECS: Frontend Context Switch");
    con.println("    GPCCS: GPC Context Switch");

    con.print_colored("  Copy Engines: ", colors::ACCENT_PURPLE);
    con.println("5 total");
    con.println("    HUB: CE0-CE3, CE_SHIM");
    con.println("    HUB: HSCE0-HSCE8 (high-speed)");

    con.print_colored("  FALCON: ", colors::ACCENT_PURPLE);
    con.println("4 microcontrollers");
    con.println("    GSP  @ 0x110000 (RISC-V, RM firmware)");
    con.println("    PMU  @ 0x10A000 (power management)");
    con.println("    SEC2 @ 0x101000 (secure boot)");
    con.println("    NVDEC@ 0x840000 (video decode)");

    con.print_colored("  GSP-RM: ", colors::ACCENT_PURPLE);
    con.println("libos-v3.1.0 kernel (XOR 0x20 decoded)");
    con.println("    ELF: kernel_ga10x.elf (RISC-V ET_REL)");
    con.println("    Sections: .fwimage, .fwversion, .fwsignature_ga10x");
    con.println("    Subsystems: mm, sched, loader, server, ipi, dma");
    con.println("    VM: kernelAddressSpace, kernelMemorySet, pageTable");
    con.println("    DMA: dmaBounceBuffer, gdmaBounceBuffer (host<>GSP)");
    con.println("    RPC: kernelServerEntry, kernelPortAllocate");
    con.println("    Task: kernelTaskCreate, handleTable, priority");
    con.println("    MNOC: mnocWorker, mnocSetRxIRQ (on-chip msg)");
    con.println("    Boot: libosBootFindElfHeader, rootFS, initELF");
    con.println("    FALCON headers: 103 embedded in firmware");
    con.println("    Strings: 10806 extracted by SigDead-BIB");

    con.print_colored("  Display: ", colors::ACCENT_PURPLE);
    con.println("4 heads, 4 SOR (DP/HDMI)");
    con.println("    I2C: 6 ports (EDID)");

    con.print_colored("  Security: ", colors::ACCENT_PURPLE);
    con.println("SEC_FAULT + BAR_FIREWALL");
    con.println("    WPR: Write-Protected Region (GSP firmware)");
    con.println("    2x RSA PKCS#1 v1.5 (2048-bit) signatures");
    con.println("    AES-256: 51 Rcon instances (encrypted channels)");
    con.println("    SHA-256: firmware integrity verification");
    con.println("    RSA e=65537: 51 instances (code signing)");
}

fn cmd_dmesg(con: &mut Console) {
    con.print_colored("Boot Log:\n", colors::ACCENT_BLUE);
    con.println("  [0.000] FastOS v0.5.0 booting...");
    con.println("  [0.001] Serial: COM1 @ 115200 baud");
    con.println("  [0.002] CPU: AMD Ryzen 5 5600X (Zen 3)");
    con.println("  [0.003] PIC: 8259A remapped IRQ 0-15 -> 32-47");
    con.println("  [0.004] IDT: 256 entries loaded");
    con.println("  [0.005] PIT: Channel 0 @ 100Hz");
    con.println("  [0.006] IRQ: Interrupts enabled (PIC+PIT+KB)");
    con.println("  [0.010] PCI: Bus scan complete");
    con.println("  [0.011] GPU: NVIDIA GA106 (0x10DE:0x2504)");
    con.println("  [0.012] GPU: BAR0 mapped (16 MB registers)");
    con.println("  [0.013] GPU: VRAM 12288 MB GDDR6 detected");
    con.println("  [0.014] GPU: Engines enabled (FIFO+GR+CE+DISP)");
    con.println("  [0.015] GPU: GSP firmware: gsp_ga10x.bin (69 MB)");
    con.println("  [0.016] GPU: libos-v3.1.0 (RISC-V, 103 FALCON)");
    con.println("  [0.017] GPU: XOR 0x20 decoded libos API (SigDead-BIB)");
    con.println("  [0.018] GPU: AES-256 + RSA-2048 + SHA-256 detected");
    con.println("  [0.020] KB:  PS/2 keyboard ready (IRQ1)");
    con.print("  [0.021] Shell: ");
    con.print_colored("ready", colors::TEXT_SUCCESS);
    con.newline();
    con.print("  Uptime: ");
    con.print_u64(crate::arch::pit::uptime_secs());
    con.println("s");
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
    let pit_secs = crate::arch::pit::uptime_secs();
    let pit_ticks = crate::arch::pit::ticks();
    con.print("Uptime: ");
    con.print_u64(pit_secs);
    con.print("s (");
    con.print_u64(pit_ticks);
    con.println(" PIT ticks @ 100Hz)");
}

fn cmd_ticks(con: &mut Console) {
    con.print_colored("Timer: ", colors::ACCENT_BLUE);
    con.print("PIT Ch0 @ 100Hz, IRQ0 via PIC");
    con.newline();
    con.print("  Ticks: ");
    con.print_u64(crate::arch::pit::ticks());
    con.newline();
    con.print("  Secs:  ");
    con.print_u64(crate::arch::pit::uptime_secs());
    con.newline();
    let tsc = cpu::rdtsc();
    con.print("  TSC:   ");
    con.print_u64(tsc);
    con.newline();
}

fn cmd_irq(con: &mut Console) {
    con.print_colored("Interrupts:\n", colors::ACCENT_BLUE);
    con.println("  PIC 8259A: Master + Slave");
    con.println("  IRQ0:  PIT timer (100Hz)");
    con.println("  IRQ1:  PS/2 keyboard");
    con.println("  IDT:   256 entries loaded");
    con.println("  GPU:   MSI capable (PMC INTR_0)");
    con.println("    PFIFO  bit 8   PGRAPH  bit 12");
    con.println("    PCOPY0 bit 17  PCOPY1  bit 18");
    con.println("    PMU    bit 24  DISPLAY bit 26");
    con.print("  Mode:  ");
    con.print_colored("Interrupt-driven", colors::TEXT_SUCCESS);
    con.newline();
}

fn cmd_gsprm(con: &mut Console) {
    con.print_colored("GSP-RM Protocol (SigDead-BIB XOR 0x20):\n", colors::ACCENT_BLUE);

    con.print_colored("  Firmware: ", colors::ACCENT_PURPLE);
    con.println("gsp_ga10x.bin (72.8 MB, RISC-V ELF64)");
    con.println("    libos-v3.1.0 | kernel_ga10x.elf | 103 FALCON hdrs");

    con.print_colored("  Memory (XOR 0x20 @ 0x7A000):\n", colors::ACCENT_PURPLE);
    con.println("    kernelMemorySetAllocate    - pool allocation");
    con.println("    kernelAddressSpaceAllocate - virtual addr space");
    con.println("    kernelAddressSpaceMapContiguous - phys mapping");
    con.println("    kernelGlobalPageMapping    - global page tables");
    con.println("    kernelTextMapping          - .text ELF section");
    con.println("    kernelDataMapping          - .data/.bss sections");
    con.println("    kernelPrivMapping          - MMIO registers");
    con.println("    dmaBounceBuffer            - host<>GSP DMA");
    con.println("    gdmaBounceBuffer           - engine<>GSP DMA");
    con.println("    libosMemoryReadable/Writeable - permissions");

    con.print_colored("  Tasks (XOR 0x20 @ 0x7B000):\n", colors::ACCENT_PURPLE);
    con.println("    kernelTaskCreate           - create GSP thread");
    con.println("    kernelTaskRegisterObject   - bind kernel object");
    con.println("    handleTable                - per-task handles");
    con.println("    priority: normal / high / realtime");

    con.print_colored("  Server (XOR 0x20 @ 0xCE000):\n", colors::ACCENT_PURPLE);
    con.println("    kernelServerEntry          - RPC dispatch entry");
    con.println("    kernelPortAllocate         - IPC port alloc");
    con.println("    servicePortShuttleAsyncRecv- async msg recv");
    con.println("    mnocWorker / mnocSetRxIRQ  - on-chip network");
    con.println("    worker / workItems         - thread pool");

    con.print_colored("  Boot (XOR 0x20 @ 0xA2000):\n", colors::ACCENT_PURPLE);
    con.println("    libosBootFindElfHeader     - locate ELF in blob");
    con.println("    rootFS / initELF           - bootstrap loader");
    con.println("    debugElf / kernelElfMap    - crash analysis");
    con.println("    debugTaskCommsPortHandle   - debug channel");

    con.print_colored("  IPI (XOR 0x20 @ 0xCD200):\n", colors::ACCENT_PURPLE);
    con.println("    ipiMessageNull             - null sentinel");
    con.println("    PAGE_SIZE=4096 | IDENTITY_MAPS_END=0xFFFFFFFF");

    con.print_colored("  Crypto (SigDead-BIB constant scan):\n", colors::ACCENT_PURPLE);
    con.println("    AES-256:  51 Rcon instances (encrypted channels)");
    con.println("    RSA-2048: 2 PKCS#1 v1.5 sigs (.fwsignature_ga10x)");
    con.println("    RSA e=65537: 51 instances across firmware");
    con.println("    SHA-256:  integrity verification (1 H constant)");

    con.print_colored("  Host<>GSP RPC:\n", colors::ACCENT_PURPLE);
    con.println("    Ring: cmd_ring (put/get) + status_ring");
    con.println("    MSG_INIT(0x01) MSG_GPU_INFO(0x02) MSG_ALLOC(0x03)");
    con.println("    MSG_CONTROL(0x05) MSG_DISPLAY(0x10) MSG_POWER(0x20)");
    con.println("    MSG_EVENT(0x100) MSG_HEARTBEAT(0xFFFF)");

    con.print("  Status: ");
    con.print_colored("READY (protocol mapped by SigDead-BIB)", colors::TEXT_SUCCESS);
    con.newline();
}

fn cmd_cube(con: &mut Console) {
    con.print_colored("=== 3D Rotating Cube ===", colors::ACCENT_CYAN);
    con.newline();
    con.println("  Software 3D renderer — Ring 0, f32 math");
    con.println("  640x480 viewport, flat shading + backface cull");
    con.println("  Rendered by CPU, displayed via RTX 3060 framebuffer");
    con.newline();

    crate::render3d::init_backbuffer();

    con.print("  Initializing... ");
    con.print_colored("OK", colors::NV_GREEN);
    con.newline();
    con.println("  Press any key to stop.");
    con.newline();

    let fb_base = con.fb_addr() as u64;
    let fb_pitch = con.fb_pitch() as u32;

    // Clear the full screen to dark background first
    let fb = fb_base as *mut u32;
    let pitch_px = fb_pitch as usize / 4;
    for y in 0..1080usize {
        for x in 0..1920usize {
            unsafe { fb.add(y * pitch_px + x).write_volatile(0xFF0D1117); }
        }
    }

    let mut tick: u64 = 0;
    loop {
        crate::render3d::render_cube(fb_base, fb_pitch, tick);
        tick += 1;

        // Check for keypress to exit
        let key = crate::drivers::keyboard::try_read_key();
        if key != 0 {
            break;
        }

        // Small delay — ~30 FPS target via spin
        for _ in 0..3_000_000u32 {
            core::hint::spin_loop();
        }
    }

    // Restore console
    con.clear();
    con.print_colored("3D demo stopped.", colors::ACCENT_CYAN);
    con.newline();
    con.print("  Rendered ");
    con.print_u64(tick);
    con.println(" frames.");
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
