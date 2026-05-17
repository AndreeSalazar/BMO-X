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

// — Subsistemas BareX y bases del MAPA (esqueletos, no enganchados a init).
//   Implementan `combo_Window_Extractor/MAPA de Window/02_BEF_Format/*` +
//   `03_Kernel_Specs/*`. NO tocan `drivers/gpu/fastgpu` (bridge BMO/GSP en obra).
mod barex;
mod bef;
mod sched;
mod syscall;
mod sandbox;

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

                // ── Detect Disks for Export ──
                con.println("[STORAGE] Detecting NVMe...");
                let mut nvme_opt = unsafe { drivers::nvme::NvmeDriver::detect() };
                if nvme_opt.is_some() {
                    con.println("[STORAGE] NVMe Primary Disk detected & initialized");
                } else {
                    con.println("[STORAGE] No NVMe detected");
                }

                con.println("[STORAGE] Detecting AHCI/SATA...");
                let mut ahci_opt = unsafe { drivers::ahci::AhciDriver::detect() };
                if let Some(ref mut ahci) = ahci_opt {
                    con.println("[STORAGE] AHCI/SATA Secondary Disk detected & initialized");
                    
                    // Dump full HBA diagnostic registers before attempting any I/O
                    unsafe { ahci.diagnose(&mut con); }

                    // Diagnostic: test read sector 1 and dump first 16 bytes
                    {
                        use crate::fs::DiskReader;
                        let mut test_buf = [0u8; 512];
                        con.println("[AHCI-DIAG] Test read LBA 1 (GPT header)...");
                        match ahci.read_sectors(1, 1, &mut test_buf) {
                            Ok(()) => {
                                con.print("  -> Bytes[0..8]:  ");
                                for i in 0..8 {
                                    con.print_hex32(test_buf[i] as u32);
                                    con.print(" ");
                                }
                                con.println("");
                                con.print("  -> Bytes[8..16]: ");
                                for i in 8..16 {
                                    con.print_hex32(test_buf[i] as u32);
                                    con.print(" ");
                                }
                                con.println("");
                                if &test_buf[0..8] == b"EFI PART" {
                                    con.println("  -> GPT signature VALID!");
                                } else {
                                    con.println("  -> GPT signature NOT FOUND in buffer");
                                }
                            },
                            Err(_) => {
                                con.println("  -> Test read FAILED!");
                                unsafe { ahci.diagnose(&mut con); }
                            }
                        }
                    }
                    
                    // Print disk capacity in GB
                    match fs::gpt::get_disk_capacity_lba(ahci) {
                        Ok(lbas) => {
                            let gb = (lbas * 512) / (1024 * 1024 * 1024);
                            con.print("  -> AHCI Disk Size: "); con.print_u64(gb); con.println(" GB");
                        },
                        Err(e) => {
                            con.print("  -> [ERROR] Failed to read GPT header via AHCI: ");
                            match e {
                                fs::DiskError::Timeout => con.println("TIMEOUT (DMA transfer never completed)"),
                                fs::DiskError::IOError => {
                                    con.println("IO_ERROR (ATA error or Task File Error)");
                                    // Dump registers again after failure for post-mortem
                                    unsafe { ahci.diagnose(&mut con); }
                                },
                                _ => con.println("UNKNOWN"),
                            }
                        },
                    }

                    // Scan GPT — find NTFS partition by reading boot sector signatures
                    {
                    use crate::fs::DiskReader;
                    match fs::gpt::scan_all_partitions(ahci) {
                        Ok(parts) => {
                            con.print("  -> GPT: "); con.print_u64(parts.len() as u64); con.println(" partitions found");
                            
                            let mut found = false;
                            for (idx, p) in parts.iter().enumerate() {
                                // Show each partition
                                con.print("     ["); con.print_u64(idx as u64); con.print("] LBA ");
                                con.print_u64(p.first_lba); con.print(" - ");
                                con.print_u64(p.last_lba);
                                
                                // Try to read boot sector to detect filesystem
                                let mut boot_sec = [0u8; 512];
                                let fs_type = match ahci.read_sectors(p.first_lba, 1, &mut boot_sec) {
                                    Ok(()) => {
                                        if &boot_sec[3..7] == b"NTFS" {
                                            "NTFS"
                                        } else if boot_sec[0..4] == [0, 0, 0, 0] {
                                            "RAW"
                                        } else {
                                            "other"
                                        }
                                    },
                                    Err(_) => "err",
                                };
                                con.print(" ("); con.print(fs_type); con.println(")");
                                
                                // Select first NTFS partition as export target
                                if !found && fs_type == "NTFS" {
                                    ahci.export_bounds = Some((p.first_lba, p.last_lba));
                                    con.println("  -> NTFS Export Partition LOCKED.");
                                    found = true;
                                }
                            }
                            
                            if !found {
                                con.println("  -> [ERROR] No NTFS partition found on SATA disk!");
                                con.println("  -> Hint: Format a partition as NTFS and place fastos_boot.bin on it.");
                            }
                        },
                        Err(_) => con.println("  -> [ERROR] Failed to scan GPT partitions via AHCI"),
                    }
                    } // end DiskReader scope

                    // ── Benchmark (with 5s timeout per operation) ──
                    if let Some(ref mut nvme) = nvme_opt {
                        if ahci.export_bounds.is_some() {
                            con.println("\n[BENCHMARK] NVMe Read vs AHCI Write (DryRun)...");
                            
                            use fs::{DiskReader, DiskWriter};
                            
                            // Estimate TSC frequency: ~3.7GHz for Ryzen 5 5600X
                            // 5 seconds timeout = 5 * 3_700_000_000 ≈ 18_500_000_000 cycles
                            let timeout_cycles: u64 = 18_500_000_000;
                            
                            // NVMe Read benchmark: read 1MB chunks, up to 16MB
                            let mut nvme_buf = alloc::vec![0u8; 512 * 8]; // 4KB per read (8 sectors)
                            let t_start_nvme = unsafe { core::arch::x86_64::_rdtsc() };
                            let mut nvme_bytes: u64 = 0;
                            let mut nvme_ok = true;
                            
                            'nvme_bench: for i in 0..4096u64 { // 4096 * 8 sectors = 16MB
                                let now = unsafe { core::arch::x86_64::_rdtsc() };
                                if now.wrapping_sub(t_start_nvme) > timeout_cycles {
                                    con.println("  [NVMe] Timeout after 5s — showing partial result");
                                    nvme_ok = false;
                                    break 'nvme_bench;
                                }
                                let lba = 34816 + (i * 8);
                                match nvme.read_sectors(lba, 8, &mut nvme_buf) {
                                    Ok(()) => { nvme_bytes += 4096; },
                                    Err(_) => {
                                        con.print("  [NVMe] Read error at LBA "); con.print_u64(lba); con.println("");
                                        nvme_ok = false;
                                        break 'nvme_bench;
                                    }
                                }
                            }
                            let t_end_nvme = unsafe { core::arch::x86_64::_rdtsc() };
                            let nvme_cycles = t_end_nvme.wrapping_sub(t_start_nvme);
                            
                            con.print("  NVMe Read: "); con.print_u64(nvme_bytes / 1024); con.print(" KB in ");
                            con.print_u64(nvme_cycles / 1_000_000); con.print("M cycles");
                            if nvme_ok { con.println(" (complete)"); } else { con.println(" (partial)"); }
                            
                            // AHCI Write benchmark (DryRun — no actual writes)
                            let t_start_ahci = unsafe { core::arch::x86_64::_rdtsc() };
                            let mut ahci_bytes: u64 = 0;
                            let mut ahci_ok = true;
                            
                            'ahci_bench: for i in 0..4096u64 {
                                let now = unsafe { core::arch::x86_64::_rdtsc() };
                                if now.wrapping_sub(t_start_ahci) > timeout_cycles {
                                    con.println("  [AHCI] Timeout after 5s — showing partial result");
                                    ahci_ok = false;
                                    break 'ahci_bench;
                                }
                                let lba = 32768 + 2048 + (i * 8);
                                match ahci.write_sectors(lba, 8, &nvme_buf) {
                                    Ok(()) => { ahci_bytes += 4096; },
                                    Err(_) => {
                                        ahci_ok = false;
                                        break 'ahci_bench;
                                    }
                                }
                            }
                            let t_end_ahci = unsafe { core::arch::x86_64::_rdtsc() };
                            let ahci_cycles = t_end_ahci.wrapping_sub(t_start_ahci);
                            
                            con.print("  AHCI Write: "); con.print_u64(ahci_bytes / 1024); con.print(" KB in ");
                            con.print_u64(ahci_cycles / 1_000_000); con.print("M cycles");
                            if ahci_ok { con.println(" (DryRun complete)"); } else { con.println(" (DryRun partial)"); }
                        }
                    }
                } else {
                    con.println("[STORAGE] No AHCI/SATA detected");
                }

                if let Some(gpu) = pci.find_nvidia_gpu() {
                    con.println("[PCI] NVIDIA GPU detected!");
                    con.print("  Vendor: 0x");
                    con.print_hex32(gpu.vendor_id as u32);
                    con.print("  Device: 0x");
                    con.print_hex32(gpu.device_id as u32);
                    con.println("");

                    // Enable PCI Bus Mastering + Memory Space (CRITICAL for DMA!)
                    let pci_cmd = drivers::pci::pci_read32(gpu.bus, gpu.device, gpu.function, 0x04);
                    con.print("  PCI CMD before: 0x");
                    con.print_hex32(pci_cmd);
                    con.println("");
                    // Bit 1 = Memory Space, Bit 2 = Bus Master
                    let pci_cmd_new = pci_cmd | 0x06; // Set bits 1 and 2
                    drivers::pci::pci_write32(gpu.bus, gpu.device, gpu.function, 0x04, pci_cmd_new);
                    let pci_cmd_verify = drivers::pci::pci_read32(gpu.bus, gpu.device, gpu.function, 0x04);
                    con.print("  PCI CMD after:  0x");
                    con.print_hex32(pci_cmd_verify);
                    if (pci_cmd_verify & 0x04) != 0 {
                        con.println(" (Bus Master ENABLED)");
                    } else {
                        con.println(" (Bus Master FAILED!)");
                    }

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
                        
                        // Create Active MMIO (Real reads and real writes)
                        use drivers::gpu::fastgpu::runtime::*;
                        let mut gpu_rt = GpuRuntime::new(GpuRuntimeMode::Active);
                        gpu_rt.advance_to(GpuCapabilityStage::BarMapped);

                        let mut mmio = unsafe {
                            drivers::gpu::fastgpu::hw::mmio::Mmio::new(bar0_full, GpuRuntimeMode::Active)
                        };

                        // Step 1: PMC Enable (Active)
                        use drivers::gpu::fastgpu::intelligence::mmio_map::registers as regs;
                        con.println("[SEC2] Step 1: PMC Enable (Active)");
                        let pmc_val = mmio.read32(regs::PMC_ENABLE);
                        mmio.write32(regs::PMC_ENABLE, pmc_val | (1 << 13));

                        // Real MMIO reads — hardware state observation AFTER PMC enable
                        con.println("[MMIO] Observing real SEC2 hardware registers...");
                        let cpuctl = mmio.read32(regs::CPUCTL);
                        let bootvec = mmio.read32(regs::BOOTVEC);
                        let irqstat = mmio.read32(regs::IRQSTAT);
                        
                        con.print("  -> CPUCTL:  0x"); con.print_hex32(cpuctl); con.println("");
                        con.print("  -> BOOTVEC: 0x"); con.print_hex32(bootvec); con.println("");
                        con.print("  -> IRQSTAT: 0x"); con.print_hex32(irqstat); con.println("");
                        
                        gpu_rt.advance_to(GpuCapabilityStage::MmioAlive);
                        con.println("[MMIO] CPUCTL/BOOTVEC/IRQSTAT read OK");

                        // ── Raw LBA Payload Loader ──
                        // fastos_boot.bin is written to SATA at absolute LBA 2048
                        // (within the RAW partition [0] LBA 34-32767)
                        // Format: [8 bytes "FASTPAY\0"] [4 bytes payload_size LE] [padding to 512] [raw FOSB data...]
                        con.println("");
                        con.println("[SEC2] Loading payload from SATA raw LBA 2048...");
                        
                        let mut loaded = false;
                        if let Some(mut ahci) = ahci_opt.take() {
                            use crate::fs::DiskReader;
                            
                            // Read first sector at LBA 2048 to get header
                            let mut header = [0u8; 512];
                            match ahci.read_sectors(2048, 1, &mut header) {
                                Ok(()) => {
                                    // Check magic "FASTPAY\0"
                                    if &header[0..8] == b"FASTPAY\0" {
                                        let payload_size = u32::from_le_bytes(header[8..12].try_into().unwrap()) as usize;
                                        con.print("  -> Header valid. Payload size: ");
                                        con.print_u64(payload_size as u64);
                                        con.println(" bytes");
                                        
                                        if payload_size > 0 && payload_size < 1024 * 1024 {
                                            // Read payload from LBA 2049 onwards
                                            let sectors_needed = ((payload_size + 511) / 512) as u32;
                                            con.print("  -> Reading "); con.print_u64(sectors_needed as u64);
                                            con.println(" sectors from LBA 2049...");
                                            
                                            let mut payload_buf = alloc::vec![0u8; (sectors_needed as usize) * 512];
                                            match ahci.read_sectors(2049, sectors_needed, &mut payload_buf) {
                                                Ok(()) => {
                                                    con.println("  -> Read OK!");
                                                    payload_buf.truncate(payload_size);
                                                    drivers::gpu::fastgpu::runtime::payload_loader::execute_from_bytes(
                                                        &mut con, &payload_buf, &mut mmio
                                                    );
                                                    loaded = true;
                                                },
                                                Err(_) => con.println("  -> [ERROR] Failed to read payload sectors!"),
                                            }
                                        } else {
                                            con.print("  -> [ERROR] Invalid payload size: ");
                                            con.print_u64(payload_size as u64);
                                            con.println("");
                                        }
                                    } else {
                                        con.print("  -> No payload at LBA 2048 (magic: ");
                                        for i in 0..8 { con.print_hex32(header[i] as u32); con.print(" "); }
                                        con.println(")");
                                        con.println("  -> Run write_payload.ps1 from Windows to write fastos_boot.bin");
                                    }
                                },
                                Err(_) => con.println("  -> [ERROR] AHCI read failed at LBA 2048!"),
                            }
                        } else {
                            con.println("  -> [ERROR] SATA drive not available.");
                        }

                        // We don't need to manually orchestrate sec2_engine here anymore.
                        // The payload completely handled the boot process and GSP_INIT_DONE polling.
                        if loaded {
                            gpu_rt.advance_to(GpuCapabilityStage::GspReady);
                        }

                        con.println("");
                        con.println("========================================");
                        con.println("[GPU] Active COMPLETE - HW Initialization Attempted");
                        con.println("  Mode: Active (real reads, real writes)");
                        con.println("  Result: Hardware state manipulated");
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
