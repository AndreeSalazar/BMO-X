//! FastOS Kernel — Entry Point
//!
//! Receives control from stage2 in 64-bit long mode, Ring 0.
//! SSE/AVX2 initialized. RDI = pointer to BootInfo.

#![no_std]
#![no_main]

mod arch;
mod drivers;
mod fs;
mod vga;
mod panic;
mod platform;

use vga::VgaWriter;

/// Boot info from stage2.asm (at 0x9100).
#[repr(C)]
pub struct BootInfo {
    pub magic: u64,
    pub memory_map_addr: u64,
    pub memory_map_count: u64,
    pub cpu_features_addr: u64,
    pub framebuffer_addr: u64,
    pub kernel_start: u64,
    pub kernel_size: u64,
}

#[unsafe(no_mangle)]
pub extern "C" fn _start(boot_info: *const BootInfo) -> ! {
    let mut vga = VgaWriter::new();
    vga.clear();

    vga.write_str_color("FastOS v0.1.0 — Ryzen 5 5600X + RTX 3060 12G", vga::Color::LightCyan);
    vga.newline();
    vga.write_str("================================================");
    vga.newline();

    // Validate boot info
    let info = unsafe { &*boot_info };
    if info.magic != 0xFA5705 {
        vga.write_str_color("ERROR: Invalid boot magic!", vga::Color::Red);
        halt_loop();
    }
    vga.write_str_color("[OK] ", vga::Color::Green);
    vga.write_str("Boot info valid");
    vga.newline();

    // Memory
    vga.write_str_color("[MEM] ", vga::Color::LightCyan);
    vga.write_str("Memory regions: ");
    vga.write_u64(info.memory_map_count);
    vga.newline();

    // CPU detection
    let cpu = arch::cpu::detect_cpu();

    let features: &[(&str, bool)] = &[
        ("SSE4.2", cpu.has_sse42),
        ("AVX2",   cpu.has_avx2),
        ("FMA3",   cpu.has_fma3),
        ("AES-NI", cpu.has_aes),
        ("SHA",    cpu.has_sha),
        ("BMI2",   cpu.has_bmi2),
    ];

    for &(name, present) in features {
        vga.write_str("[CPU] ");
        vga.write_str(name);
        vga.write_str(": ");
        if present {
            vga.write_str_color("YES", vga::Color::Green);
        } else {
            vga.write_str_color("NO", vga::Color::Red);
        }
        vga.newline();
    }

    // Serial init
    drivers::serial::init_serial();
    drivers::serial::serial_write("[FastOS] Serial output active\n");

    // PCI scan
    vga.write_str_color("[PCI] ", vga::Color::LightCyan);
    vga.write_str("Scanning bus...");
    vga.newline();

    let devices = drivers::pci::scan_pci_bus();
    vga.write_str("[PCI] Devices found: ");
    vga.write_u64(devices.count as u64);
    vga.newline();

    // Check for NVIDIA GPU
    if let Some(gpu_pci) = devices.find_nvidia_gpu() {
        vga.write_str_color("[GPU] NVIDIA GPU detected: ", vga::Color::LightGreen);
        vga.write_str("0x");
        vga.write_hex16(gpu_pci.vendor_id);
        vga.write_str(":0x");
        vga.write_hex16(gpu_pci.device_id);
        vga.newline();

        // Initialize full GPU driver stack (Ring 0)
        vga.write_str_color("[GPU] ", vga::Color::LightCyan);
        vga.write_str("Initializing Driver_Canon GA106...");
        vga.newline();

        match drivers::gpu::rtx3060::init_gpu_driver() {
            Ok(mut driver_state) => {
                let info = drivers::gpu::rtx3060::gpu_info(&driver_state);
                vga.write_str_color("[GPU] ", vga::Color::Green);
                vga.write_str("Driver ready! VRAM: ");
                vga.write_u64(info.vram_size_mb);
                vga.write_str(" MB");
                vga.newline();

                vga.write_str_color("[GPU] ", vga::Color::Green);
                vga.write_str("Chip ID: 0x");
                vga.write_hex32(info.chip_id);
                vga.newline();

                drivers::serial::serial_write("[FastOS] GPU driver initialized (Ring 0)\n");
            }
            Err(_e) => {
                vga.write_str_color("[GPU] Driver init failed", vga::Color::Red);
                vga.newline();
                drivers::serial::serial_write("[FastOS] GPU driver init FAILED\n");
            }
        }
    } else {
        vga.write_str_color("[GPU] No NVIDIA GPU found", vga::Color::Yellow);
        vga.newline();
    }

    vga.newline();
    vga.write_str_color("FastOS kernel initialized!", vga::Color::LightGreen);
    vga.newline();
    vga.write_str("System halted.");

    halt_loop();
}

pub fn halt_loop() -> ! {
    loop {
        unsafe { core::arch::asm!("hlt"); }
    }
}
