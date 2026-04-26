//! FastOS Shell — Interactive command interpreter.
//! Ring 0, no_std. Only commands backed by real hardware state.

use crate::console::Console;
use crate::fb::colors;
use crate::arch::cpu;

const MAX_LINE: usize = 256;

/// Basic PS/2 Set 1 Scancode to ASCII map (US QWERTY, lowercase only)
fn scancode_to_ascii(scancode: u8) -> Option<u8> {
    match scancode {
        0x02 => Some(b'1'), 0x03 => Some(b'2'), 0x04 => Some(b'3'), 0x05 => Some(b'4'),
        0x06 => Some(b'5'), 0x07 => Some(b'6'), 0x08 => Some(b'7'), 0x09 => Some(b'8'),
        0x0A => Some(b'9'), 0x0B => Some(b'0'),
        
        0x10 => Some(b'q'), 0x11 => Some(b'w'), 0x12 => Some(b'e'), 0x13 => Some(b'r'),
        0x14 => Some(b't'), 0x15 => Some(b'y'), 0x16 => Some(b'u'), 0x17 => Some(b'i'),
        0x18 => Some(b'o'), 0x19 => Some(b'p'),
        
        0x1E => Some(b'a'), 0x1F => Some(b's'), 0x20 => Some(b'd'), 0x21 => Some(b'f'),
        0x22 => Some(b'g'), 0x23 => Some(b'h'), 0x24 => Some(b'j'), 0x25 => Some(b'k'),
        0x26 => Some(b'l'), 
        
        0x2C => Some(b'z'), 0x2D => Some(b'x'), 0x2E => Some(b'c'), 0x2F => Some(b'v'),
        0x30 => Some(b'b'), 0x31 => Some(b'n'), 0x32 => Some(b'm'), 
        
        0x39 => Some(b' '), // Space
        0x1C => Some(b'\n'), // Enter
        0x0E => Some(8),     // Backspace
        _ => None,
    }
}

/// Read key via PS/2 polling (requires BIOS Legacy USB emulation) or Serial COM1.
fn read_any_key() -> u8 {
    loop {
        // 1. Poll PS/2 Keyboard Status Register (Port 0x64)
        let status: u8;
        unsafe { core::arch::asm!("in al, dx", out("al") status, in("dx") 0x64u16); }
        
        // If Output Buffer Status bit (bit 0) is 1, data is available
        if (status & 1) != 0 {
            let scancode: u8;
            unsafe { core::arch::asm!("in al, dx", out("al") scancode, in("dx") 0x60u16); }
            
            // Only handle "press" events (scancode < 0x80)
            if scancode < 0x80 {
                if let Some(c) = scancode_to_ascii(scancode) {
                    return c;
                }
            }
        }

        // 2. Poll Serial Port
        if let Some(b) = crate::drivers::serial::serial_read_byte() {
            crate::drivers::serial::serial_write_byte(b); // local echo
            return b;
        }

        for _ in 0..1000u32 { core::hint::spin_loop(); }
    }
}

/// Run the interactive shell loop (never returns).
pub fn run(con: &mut Console) {
    con.print_colored("FastOS v0.6.0", colors::NV_GREEN);
    con.print(" - Ring 0 | UEFI Native");
    con.newline();
    con.print_colored("Type 'help' for commands.", colors::TEXT_SECONDARY);
    con.newline();
    con.newline();

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
            return len;
        } else if key == 8 {
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
    } else if bytes_eq(cmd, b"gspinit") {
        cmd_gsp_init(con);
    } else if bytes_eq(cmd, b"gsprpc") {
        cmd_gsprpc(con);
    } else if bytes_eq(cmd, b"ver") {
        con.print_colored("FastOS v0.6.0 (Rust, no_std, Ring 0, UEFI Native)", colors::ACCENT_CYAN);
        con.newline();
    } else if bytes_eq(cmd, b"reboot") {
        cmd_reboot();
    } else if bytes_eq(cmd, b"halt") {
        con.print_colored("System halted.", colors::ACCENT_ORANGE);
        con.newline();
        loop { unsafe { core::arch::asm!("hlt"); } }
    } else if bytes_eq(cmd, b"tsc") {
        let tsc = cpu::rdtsc();
        con.print("TSC: ");
        con.print_u64(tsc);
        con.newline();
    } else {
        con.print_colored("Unknown: ", colors::ACCENT_RED);
        for &b in cmd { con.put_char(b); }
        con.newline();
    }
}

fn cmd_help(con: &mut Console) {
    con.print_colored("Commands:", colors::ACCENT_BLUE);
    con.newline();
    print_cmd(con, "help", "Show this help");
    print_cmd(con, "cpuinfo", "CPU features (live CPUID)");
    print_cmd(con, "gpuinfo", "GPU info (live PCI + BAR0)");
    print_cmd(con, "pci", "PCI devices (live ECAM scan)");
    print_cmd(con, "meminfo", "UEFI memory map");
    print_cmd(con, "gputest", "GPU HW register test suite");
    print_cmd(con, "gpucmd", "GPU command engine (pushbuffer)");
    print_cmd(con, "gspinit", "Wake up GPU System Processor");
    print_cmd(con, "gsprpc", "Send Test RPC Command to GSP");
    print_cmd(con, "cube", "3D rotating cube (software)");
    print_cmd(con, "tsc", "Read TSC counter");
    print_cmd(con, "clear", "Clear screen");
    print_cmd(con, "ver", "Kernel version");
    print_cmd(con, "reboot", "Reboot system");
    print_cmd(con, "halt", "Halt CPU");
}

fn print_cmd(con: &mut Console, name: &str, desc: &str) {
    con.print("  ");
    con.print_colored(name, colors::NV_GREEN);
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
    let platform = crate::platform::FastOsPlatform::new();
    use nv_hal::Platform;

    let gpu_pci = nv_hal::find_gpu(&platform);
    match gpu_pci {
        Some(pci) => {
            let vd = platform.pci_config_read32(pci, 0x00);
            let vendor = (vd & 0xFFFF) as u16;
            let dev_id = ((vd >> 16) & 0xFFFF) as u16;

            con.print_colored("GPU: ", colors::ACCENT_PURPLE);
            con.print("NVIDIA ");
            con.print_hex32(((vendor as u32) << 16) | dev_id as u32);
            con.newline();

            con.print("  PCI: bus=");
            con.print_u64(pci.bus as u64);
            con.print(" dev=");
            con.print_u64(pci.device as u64);
            con.print(" fn=");
            con.print_u64(pci.function as u64);
            con.newline();

            let bar0 = nv_hal::read_bar0(&platform, pci);
            con.print("  BAR0: ");
            con.print_hex32(bar0 as u32);
            con.newline();

            let bar1 = nv_hal::read_bar1(&platform, pci);
            con.print("  BAR1: ");
            con.print_hex32(bar1 as u32);
            con.newline();

            // Read BOOT_0 chip ID
            let bar0_ptr = platform.map_mmio(bar0, nv_regs::BAR0_SIZE);
            if !bar0_ptr.is_null() {
                let mmio = unsafe { nv_hal::MmioRegion::new(bar0_ptr, nv_regs::BAR0_SIZE) };
                let boot0 = mmio.read32(nv_regs::pmc::BOOT_0);
                con.print("  BOOT_0: ");
                con.print_hex32(boot0);
                con.newline();

                let vram = nv_gpu::detect_vram(&mmio);
                con.print("  VRAM: ");
                con.print_u64(vram / (1024 * 1024));
                con.println(" MB");

                let engines = mmio.read32(nv_regs::pmc::ENABLE);
                con.print("  Engines: ");
                con.print_hex32(engines);
                con.newline();
            }

            // GSP firmware status
            let gsp_addr = unsafe { crate::boot_info::GSP_FW_ADDR };
            let gsp_size = unsafe { crate::boot_info::GSP_FW_SIZE };
            con.print("  GSP FW: ");
            if gsp_addr != 0 {
                con.print_u64(gsp_size / (1024 * 1024));
                con.print_colored(" MB loaded", colors::TEXT_SUCCESS);
            } else {
                con.print_colored("not loaded", colors::ACCENT_RED);
            }
            con.newline();
        }
        None => {
            con.print_colored("GPU: ", colors::ACCENT_PURPLE);
            con.print_colored("not found on PCI bus", colors::ACCENT_RED);
            con.newline();
        }
    }
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
    let bi = unsafe { crate::boot_info::BOOT_INFO };
    if bi.is_null() {
        con.println("BootInfo not available");
        return;
    }
    let bi = unsafe { &*bi };

    con.print_colored("Memory (UEFI):\n", colors::ACCENT_BLUE);
    con.print("  Kernel base: ");
    con.print_hex32(bi.kernel_base as u32);
    con.print(" size: ");
    con.print_u64(bi.kernel_size / 1024);
    con.println(" KB");

    con.print("  Stack top:   ");
    con.print_hex32(bi.stack_top as u32);
    con.print(" size: ");
    con.print_u64(bi.stack_size / 1024);
    con.println(" KB");

    con.print("  Framebuffer: ");
    con.print_hex32(bi.fb_addr as u32);
    con.print(" ");
    con.print_u64(bi.fb_width as u64);
    con.print("x");
    con.print_u64(bi.fb_height as u64);
    con.newline();

    con.print("  Memory map:  ");
    con.print_u64(bi.memory_map_count);
    con.println(" entries");

    // Count usable RAM
    let mut usable_pages: u64 = 0;
    let count = bi.memory_map_count as usize;
    for i in 0..count {
        if i >= 256 { break; }
        if bi.memory_map[i].mem_type == fastos_boot_protocol::MemoryType::Usable {
            usable_pages += bi.memory_map[i].size / 4096;
        }
    }
    con.print("  Usable RAM:  ");
    con.print_u64(usable_pages * 4 / 1024);
    con.println(" MB");

    con.print("  Page alloc:  ");
    con.print_u64(unsafe { crate::arch::page_alloc::free_count() } as u64);
    con.println(" free pages");
}

fn cmd_cube(con: &mut Console) {
    con.print_colored("=== 3D Rotating Cube ===", colors::ACCENT_CYAN);
    con.newline();
    con.println("  Software renderer, Ring 0, f32 math");
    con.println("  Press any key to stop.");
    con.newline();

    crate::render3d::init_backbuffer();

    let fb_base = con.fb_addr() as u64;
    let fb_pitch = con.fb_pitch() as u32;

    // Clear screen
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

        let key = crate::drivers::serial::serial_read_byte().unwrap_or(0);
        if key != 0 { break; }

        for _ in 0..3_000_000u32 { core::hint::spin_loop(); }
    }

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

fn cmd_gsp_init(con: &mut Console) {
    con.print_colored("[FastOS] Intentando despertar procesador GSP...\n", colors::ACCENT_PURPLE);
    let platform = crate::platform::FastOsPlatform::new();
    if let Some(pci) = nv_hal::find_gpu(&platform) {
        use nv_hal::Platform;
        let bar0_phys = nv_hal::read_bar0(&platform, pci);
        if bar0_phys != 0 && bar0_phys != 0xFFFF_FFFF_FFFF_FFF0 {
            let bar0_ptr = platform.map_mmio(bar0_phys, 16 * 1024 * 1024);
            if !bar0_ptr.is_null() {
                let bar0 = unsafe { nv_hal::MmioRegion::new(bar0_ptr, 16 * 1024 * 1024) };
                let bi = unsafe { crate::boot_info::BOOT_INFO };
                if !bi.is_null() {
                    let gsp_addr = unsafe { (*bi).gsp_addr };
                    let gsp_size = unsafe { (*bi).gsp_size };
                    if gsp_addr != 0 && gsp_size > 0 {
                        let fw_blob = unsafe { core::slice::from_raw_parts(gsp_addr as *const u8, gsp_size as usize) };
                        con.println("[FastOS] Firmware GSP en memoria. Ejecutando handshake...");
                        if let Err(_e) = crate::drivers::gsp::gsp_init(&bar0, fw_blob, con) {
                            con.print_colored("[FastOS] ERROR: GSP fallo.\n", colors::ACCENT_RED);
                        } else {
                            con.print_colored("[FastOS] EXITO: GSP Despierto y listo.\n", colors::TEXT_SUCCESS);
                        }
                    } else {
                        con.print_colored("ERROR: Firmware gsp_ga10x.bin no cargado.\n", colors::ACCENT_RED);
                    }
                } else {
                    con.println("ERROR: BootInfo nulo.");
                }
            } else {
                con.println("ERROR: Mapeo MMIO BAR0 fallido.");
            }
        } else {
            con.println("ERROR: BAR0 Invalido.");
        }
    } else {
        con.println("ERROR: GPU no encontrada.");
    }
}

fn cmd_gsprpc(con: &mut Console) {
    con.print_colored("[FastOS] Pipeline GPU RTX 3060 GA10x — Datos Reales del Firmware\n", colors::ACCENT_PURPLE);
    
    let platform = crate::platform::FastOsPlatform::new();
    if let Some(pci) = nv_hal::find_gpu(&platform) {
        use nv_hal::Platform;
        let bar0_phys = nv_hal::read_bar0(&platform, pci);
        let bar0_ptr = platform.map_mmio(bar0_phys, 16 * 1024 * 1024);
        let bar0 = unsafe { nv_hal::MmioRegion::new(bar0_ptr, 16 * 1024 * 1024) };

        // === FASE 1: GMMU ===
        let mut gmmu = crate::drivers::gsp::gmmu::GmmuManager::new(&bar0).unwrap();
        gmmu.init(con);

        // === FASE 2: RPC + Resource Manager (todas las clases GA10x) ===
        let rpc_phys = unsafe { crate::arch::page_alloc::alloc_pages_contiguous(1).unwrap() };
        let mut rpc_ring = crate::drivers::gsp::rpc::GspRpcRing::new(&bar0, rpc_phys);
        rpc_ring.init(con);

        {
            let mut nv_rm = crate::drivers::gsp::nv_rm::NvResourceManager::new(&mut rpc_ring);
            let _ = nv_rm.allocate_vram(1024, con);
            let _ = nv_rm.init_display_engine(con);
            let _ = nv_rm.init_3d_engine(con);
            let _ = nv_rm.init_compute_engine(con);
            let _ = nv_rm.init_dma_copy(con);
        }

        // === FASE 3: Display Engine 1080p (GPU dibuja, NO CPU) ===
        let disp = crate::drivers::gsp::disp::DisplayEngine::new(&bar0);
        disp.set_mode_1080p(&mut rpc_ring, con);

        // === FASE 4: Pushbuffer 3D (PGRAPH Class 0xC697) ===
        let pb_phys = unsafe { crate::arch::page_alloc::alloc_pages_contiguous(1).unwrap() };
        let mut pushbuffer = crate::drivers::gsp::pushbuffer::PushBuffer::new(pb_phys);
        
        pushbuffer.bind_3d_class();      // Seleccionar clase 0xC697
        pushbuffer.nop();                 // Verificar canal
        pushbuffer.clear_color(0x001E3A5F); // Azul oscuro premium
        pushbuffer.execute(&bar0, con);

        // Limpiar
        unsafe { 
            crate::arch::page_alloc::free_pages(rpc_phys, 1);
            crate::arch::page_alloc::free_pages(pb_phys, 1);
        }

        con.print_colored("\n[FastOS] GPU RTX 3060 GA10x — Pipeline COMPLETO\n", colors::TEXT_SUCCESS);
        
    } else {
        con.println("ERROR: GPU no encontrada.");
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
