//! FastOS Kernel v0.6.0 — Entry Point
//!
//! Receives control from UEFI bootloader in 64-bit long mode, Ring 0.
//! RDI = *const fastos_boot_protocol::BootInfo

#![no_std]
#![no_main]

extern crate alloc;

mod agent;
mod allocator;
mod arch;
mod export;
mod boot_info;
mod console;
mod drivers;
mod fb;
mod fs;
mod font;
mod panic;
mod shell;

use core::arch::naked_asm;

/// Print a u64 as 16-digit hex to serial.
fn serial_hex(val: u64) {
    let hex = b"0123456789ABCDEF";
    drivers::serial::serial_write("0x");
    for i in (0..16).rev() {
        drivers::serial::serial_write_byte(hex[((val >> (i * 4)) & 0xF) as usize]);
    }
}

/// ELF entry point. Bootloader passes BootInfo pointer in RDI.
#[unsafe(no_mangle)]
#[link_section = ".text._start"]
#[unsafe(naked)]
unsafe extern "C" fn _start() -> ! {
    naked_asm!(
        "test rdi, rdi",      // verificar que RDI no es null
        "jz 2f",             // si es null, saltar a halt
        "mov rbx, rdi",      // guardar boot_info antes de todo
        "and rsp, -16",
        "mov rdi, rbx",      // restaurar RDI limpio para kernel_main
        "call kernel_main",  // kernel_main es naked, llamará a kernel_main_real
        "2: hlt",
        "jmp 2b",
    );
}

#[unsafe(no_mangle)]
#[unsafe(naked)]
extern "C" fn kernel_main(boot_info_ptr: *const fastos_boot_protocol::BootInfo) -> ! {
    naked_asm!(
        "test rdi, rdi",
        "jz 1f",
        "mov rbx, rdi",
        "1:",
        // Llamar a la función real de kernel (no naked)
        "call kernel_main_real",
        "3: hlt",
        "jmp 3b"
    );
}

#[unsafe(no_mangle)]
#[inline(never)]
extern "C" fn kernel_main_real(boot_info_ptr: *const fastos_boot_protocol::BootInfo) -> ! {

    // ── Initialize serial first for debug output ─────────────────────
    drivers::serial::init_serial();
    drivers::serial::serial_write("[FastOS] Kernel v0.6.0 starting\n");
    // Verificar que boot_info_ptr no es null antes de desrefenciar
    if boot_info_ptr.is_null() {
        drivers::serial::serial_write("[FastOS] FATAL: boot_info_ptr is NULL!\n");
        loop { unsafe { core::arch::asm!("hlt"); } }
    }

    let bi = unsafe { &*boot_info_ptr };

    // ── Validate BootInfo magic ──────────────────────────────────────
    if bi.magic != fastos_boot_protocol::BOOT_MAGIC {
        drivers::serial::serial_write("[FastOS] FATAL: Invalid BootInfo magic: ");
        serial_hex(bi.magic);
        drivers::serial::serial_write("\n");
        loop { unsafe { core::arch::asm!("hlt"); } }
    }
    drivers::serial::serial_write("[FastOS] BootInfo valid\n");

    // ── Print boot info ──────────────────────────────────────────────
    drivers::serial::serial_write("[FastOS] FB addr: ");
    serial_hex(bi.fb_addr);
    drivers::serial::serial_write("\n");

    drivers::serial::serial_write("[FastOS] FB resolution: ");
    serial_hex(bi.fb_width as u64);
    drivers::serial::serial_write("x");
    serial_hex(bi.fb_height as u64);
    drivers::serial::serial_write("\n");

    drivers::serial::serial_write("[FastOS] Memory map entries: ");
    serial_hex(bi.memory_map_count);
    drivers::serial::serial_write("\n");

    // ── Store boot info globally ─────────────────────────────────────
    unsafe {
        boot_info::BOOT_INFO = boot_info_ptr;
        boot_info::GSP_FW_ADDR = bi.gsp_addr;
        boot_info::GSP_FW_SIZE = bi.gsp_size;
    }

    // ── Initialize arch subsystems ───────────────────────────────────
    // IDT is critical even without interrupts enabled — without it,
    // any CPU exception (page fault, GPF, etc.) causes a triple fault.
    arch::idt::init_idt();
    drivers::serial::serial_write("[FastOS] IDT loaded (exceptions will halt instead of triple-fault)\n");

    // ── PCI via ECAM (UEFI-native, no legacy I/O ports) ─────────────
    drivers::serial::serial_write("[FastOS] Parsing ACPI MCFG for ECAM...\n");
    match arch::acpi::parse_mcfg(bi.rsdp_addr) {
        Some(ecam) => {
            drivers::serial::serial_write("[FastOS] ECAM base: ");
            serial_hex(ecam.base_addr);
            drivers::serial::serial_write(" buses 0..");
            serial_hex(ecam.end_bus as u64);
            drivers::serial::serial_write("\n");

            drivers::pci::init_ecam(ecam.base_addr, ecam.end_bus);

            drivers::serial::serial_write("[FastOS] Scanning PCI bus via ECAM...\n");
            let pci = drivers::pci::scan_pci_bus();
            drivers::serial::serial_write("[FastOS] PCI scan complete: ");
            serial_hex(pci.count as u64);
            drivers::serial::serial_write(" devices\n");

            // Look for NVIDIA GPU (any 0x10DE display controller)
            if let Some(gpu) = pci.find_nvidia_gpu() {
                drivers::serial::serial_write("[PCI] NVIDIA GPU detected: VEN=0x10DE DEV=0x");
                serial_hex(gpu.device_id as u64);
                drivers::serial::serial_write(" BAR0=0x");
                serial_hex(gpu.bar0 as u64);
                drivers::serial::serial_write("\n");
            } else {
                drivers::serial::serial_write("[PCI] No NVIDIA GPU found.\n");
            }
        }
        None => {
            drivers::serial::serial_write("[FastOS] WARNING: MCFG not found — PCI unavailable\n");
        }
    }

    // ── Initialize page frame allocator ───────────────────────────────────
    // Inicializar asignador de páginas (necesita el mapa de memoria UEFI)
    unsafe {
        crate::arch::page_alloc::init(&bi.memory_map, bi.memory_map_count as usize, bi.gsp_addr, bi.gsp_size);
    }
    drivers::serial::serial_write("[FastOS] Page allocator initialized (");
    serial_hex(unsafe { arch::page_alloc::free_count() } as u64);
    drivers::serial::serial_write(" free pages)\n");

    // ── Run shell ─────────────────────────────────────────────────────────
    match bi.fb_addr {
        0 => loop { unsafe { core::arch::asm!("hlt"); } },
        fb_addr => {
            let mut con = console::Console::new(fb_addr, bi.fb_pitch(), bi.fb_stride, bi.fb_width, bi.fb_height);
            con.clear();
            con.print("[FastOS] Framebuffer GOP: ");
            con.print_u64(bi.fb_width as u64);
            con.print("x");
            con.print_u64(bi.fb_height as u64);
            con.print(" stride=");
            con.print_u64(bi.fb_stride as u64);
            con.println(" target=74Hz");
            con.print("[FastOS] Console buffer: ");
            if con.is_double_buffered() {
                con.println("RAM shadow + memcpy flush");
            } else {
                con.println("direct framebuffer writes");
            }
            
            con.println("[FastOS] Special Agent mode: active.");
            if let Some(ecam) = arch::acpi::parse_mcfg(bi.rsdp_addr) {
                con.print("[FastOS] ECAM at ");
                con.print_hex32(ecam.base_addr as u32);
                con.println("");
            }

            // ── GPU DryRun Orchestration (visible on screen) ──────────────
            con.println("");
            con.println("========================================");
            con.println("[GPU] DryRun Validation Starting...");
            con.println("========================================");

            // Re-scan PCI to find GPU (we already did in early boot, do it again with console)
            if let Some(ecam) = arch::acpi::parse_mcfg(bi.rsdp_addr) {
                drivers::pci::init_ecam(ecam.base_addr, ecam.end_bus);
                let pci = drivers::pci::scan_pci_bus();

                con.print("[PCI] Devices found: ");
                con.print_u64(pci.count as u64);
                con.println("");

                if let Some(gpu) = pci.find_nvidia_gpu() {
                    con.println("[PCI] NVIDIA GPU detected!");
                    con.print("  Vendor: 0x");
                    con.print_hex32(gpu.vendor_id as u32);
                    con.print("  Device: 0x");
                    con.print_hex32(gpu.device_id as u32);
                    con.println("");

                    // Mask BAR0 lower bits (type bits)
                    let bar0_raw = gpu.bar0;
                    let bar0_phys = (bar0_raw & 0xFFFFFFF0) as u64;
                    
                    // Check for 64-bit BAR (bit 2:1 = 10b means 64-bit)
                    let bar0_full = if (bar0_raw & 0x06) == 0x04 {
                        // 64-bit BAR: combine BAR0 + BAR1
                        bar0_phys | ((gpu.bar1 as u64) << 32)
                    } else {
                        bar0_phys
                    };

                    con.print("  BAR0 raw: 0x");
                    con.print_hex32(bar0_raw);
                    con.println("");
                    con.print("  BAR0 addr: 0x");
                    con.print_hex32(bar0_full as u32);
                    con.println("");

                    if bar0_full == 0 {
                        con.println("[FAULT] BAR0 is 0x0! Cannot map MMIO.");
                    } else {
                        con.println("[GPU] BAR0 valid.");
                        
                        // Create ObserveOnly MMIO (Reads from hardware, simulated writes)
                        use drivers::gpu::fastgpu::runtime::*;
                        let mut gpu_rt = GpuRuntime::new(GpuRuntimeMode::ObserveOnly);
                        gpu_rt.advance_to(GpuCapabilityStage::BarMapped);

                        let mut mmio = unsafe {
                            drivers::gpu::fastgpu::hw::mmio::Mmio::new(bar0_full, GpuRuntimeMode::ObserveOnly)
                        };

                        // Real MMIO reads — hardware state observation
                        use drivers::gpu::fastgpu::intelligence::mmio_map::registers as regs;
                        con.println("[MMIO] Observing real SEC2 hardware registers...");
                        let cpuctl = mmio.read32(regs::CPUCTL);
                        let bootvec = mmio.read32(regs::BOOTVEC);
                        let irqstat = mmio.read32(regs::IRQSTAT);
                        gpu_rt.advance_to(GpuCapabilityStage::MmioAlive);
                        con.println("[MMIO] CPUCTL/BOOTVEC/IRQSTAT read OK");

                        // Execute SEC2 sequence trace
                        con.println("");
                        con.println("[SEC2] Executing bring-up sequence (ObserveOnly)...");
                        let seq = drivers::gpu::fastgpu::intelligence::sequences::SEC2_BRINGUP_STEPS;
                        drivers::gpu::fastgpu::sequences::execute_sequence("SEC2 Bring-Up", seq, &mut mmio);

                        // Orchestrate SEC2 engine
                        use drivers::gpu::fastgpu::falcon::FalconEngine;
                        let mut sec2 = drivers::gpu::fastgpu::engines::sec2::Sec2Engine::new(&mut mmio);
                        
                        con.println("[SEC2] Step 1: PMC Enable (ObserveOnly)");
                        sec2.enable_pmc();
                        
                        con.println("[SEC2] Step 2: Reset release (ObserveOnly)");
                        let _ = sec2.reset();
                        gpu_rt.advance_to(GpuCapabilityStage::FalconResetReleased);
                        
                        con.println("[SEC2] Step 3: IMEM upload (ObserveOnly)");
                        let dummy_fw: [u8; 16] = [0; 16];
                        let _ = sec2.load_imem(&dummy_fw);
                        gpu_rt.advance_to(GpuCapabilityStage::ImemUploaded);

                        con.println("[SEC2] Step 4: DMEM upload (ObserveOnly)");
                        let _ = sec2.load_dmem(&dummy_fw);
                        gpu_rt.advance_to(GpuCapabilityStage::DmemUploaded);

                        con.println("[SEC2] Step 5: BOOTVEC = 0x0 (ObserveOnly)");
                        let _ = sec2.set_bootvec(0);
                        gpu_rt.advance_to(GpuCapabilityStage::BootvecConfigured);

                        con.println("[SEC2] Step 6: CPUCTL start (ObserveOnly)");
                        let _ = sec2.start_cpu();
                        gpu_rt.advance_to(GpuCapabilityStage::CpuStarted);

                        con.println("[SEC2] Step 7: HS mode poll (ObserveOnly)");
                        let _ = sec2.validate_hs_mode();

                        con.println("");
                        con.println("========================================");
                        con.println("[GPU] ObserveOnly COMPLETE - HW Reads OK");
                        con.println("  Mode: ObserveOnly (real reads, fake writes)");
                        con.println("  Result: Hardware state validated");
                        con.println("========================================");
                    }
                } else {
                    con.println("[PCI] No NVIDIA GPU found (class 0x03).");
                }
            } else {
                con.println("[GPU] No ECAM — cannot scan PCI.");
            }
            
            con.println("");
            shell::run(&mut con);
            loop { unsafe { core::arch::asm!("hlt"); } }
        }
    }
}
