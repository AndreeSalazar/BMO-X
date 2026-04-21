//! FastOS Kernel v0.5.0 — Entry Point
//!
//! Receives control from UEFI bootloader in 64-bit long mode, Ring 0.
//! RDI = *const fastos_boot_protocol::BootInfo

#![no_std]
#![no_main]
#![allow(dead_code, unused_imports, unused_variables)]

mod arch;
mod boot_info;
mod console;
mod drivers;
mod fb;
mod fs;
mod gpu;
mod render3d;
mod vga;
mod panic;
mod platform;
mod shell;
mod tests;
mod crypto;

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
        "mov rax, [rbx + 8]",  // fb_addr desde rbx, no rdi
        "test rax, rax",
        "jz 1f",
        "mov rcx, 2073600",
        "mov edx, 0x0000FF00",
        "0: mov [rax], edx",
        "add rax, 4",
        "loop 0b",
        "1:",
        "mov rdi, rbx",      // restaurar RDI limpio para kernel_main
        "call kernel_main",
        "2: hlt",
        "jmp 2b",
    );
}

#[unsafe(no_mangle)]
extern "C" fn kernel_main(boot_info_ptr: *const fastos_boot_protocol::BootInfo) -> ! {
    // ── Zero BSS section FIRST (before any static variable access) ─────
    unsafe {
        extern "C" {
            static __bss_start: u8;
            static __bss_end: u8;
        }
        let bss_start = &__bss_start as *const u8 as *mut u8;
        let bss_end = &__bss_end as *const u8 as *mut u8;
        let len = bss_end as usize - bss_start as usize;
        core::ptr::write_bytes(bss_start, 0, len);
    }

    // CHECKPOINT 1 - AZUL: Después de BSS zero (antes de serial init)
    // Necesitamos leer fb_addr pero sin serial init todavía
    if !boot_info_ptr.is_null() {
        let bi = unsafe { &*boot_info_ptr };
        if bi.fb_addr != 0 {
            unsafe {
                let fb = bi.fb_addr as *mut u32;
                // Franja azul (0x00FF0000) en la parte superior
                for i in 0..(bi.fb_width as usize * 10) {
                    *fb.add(i) = 0x00FF0000;
                }
            }
        }
    }

    // ── Initialize serial first for debug output ─────────────────────
    drivers::serial::init_serial();
    drivers::serial::serial_write("[FastOS] Kernel v0.5.0 starting\n");
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

    // CHECKPOINT 2 - AMARILLO: Después de BootInfo validation
    if bi.fb_addr != 0 {
        unsafe {
            let fb = bi.fb_addr as *mut u32;
            let offset = bi.fb_width as usize * 10;
            // Franja amarilla (0x00FFFF00) debajo del azul
            for i in 0..(bi.fb_width as usize * 10) {
                *fb.add(offset + i) = 0x00FFFF00;
            }
        }
    }

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

    // CHECKPOINT 3 - CYAN: Después de BootInfo validation
    if bi.fb_addr != 0 {
        unsafe {
            let fb = bi.fb_addr as *mut u32;
            let offset = bi.fb_width as usize * 20;
            // Franja cyan (0x0000FFFF) debajo del amarillo
            for i in 0..(bi.fb_width as usize * 10) {
                *fb.add(offset + i) = 0x0000FFFF;
            }
        }
    }

    // ── Store boot info globally ─────────────────────────────────────
    unsafe {
        boot_info::BOOT_INFO = boot_info_ptr;
        boot_info::GSP_FW_ADDR = bi.gsp_addr;
        boot_info::GSP_FW_SIZE = bi.gsp_size;
    }

    // ── Initialize arch subsystems ───────────────────────────────────
    arch::pic::init_pic();
    arch::pic::set_mask_keyboard_timer();
    drivers::serial::serial_write("[FastOS] PIC initialized\n");

    arch::idt::init_idt();
    drivers::serial::serial_write("[FastOS] IDT loaded\n");

    arch::pit::init_pit();
    arch::idt::register_irq(0, arch::pit::tick);
    drivers::serial::serial_write("[FastOS] PIT @ 100Hz\n");

    // ── Enable interrupts ────────────────────────────────────────────
    unsafe { core::arch::asm!("sti"); }
    drivers::serial::serial_write("[FastOS] Interrupts enabled\n");

    // CHECKPOINT 4 - MAGENTA: Después de arch subsystems init
    if bi.fb_addr != 0 {
        unsafe {
            let fb = bi.fb_addr as *mut u32;
            let offset = bi.fb_width as usize * 30;
            // Franja magenta (0x00FF00FF) debajo del cyan
            for i in 0..(bi.fb_width as usize * 10) {
                *fb.add(offset + i) = 0x00FF00FF;
            }
        }
    }

    // ── Initialize PS/2 keyboard ─────────────────────────────────────
    drivers::keyboard::init_keyboard();
    drivers::serial::serial_write("[FastOS] PS/2 keyboard ready\n");

    // ── PCI scan ─────────────────────────────────────────────────────
    drivers::serial::serial_write("[FastOS] Scanning PCI bus...\n");
    let _pci = drivers::pci::scan_pci_bus();
    drivers::serial::serial_write("[FastOS] PCI scan complete\n");

    // ── Initialize page frame allocator ───────────────────────────────────
    unsafe {
        arch::page_alloc::init(&bi.memory_map, bi.memory_map_count as usize);
    }
    drivers::serial::serial_write("[FastOS] Page allocator initialized (");
    serial_hex(unsafe { arch::page_alloc::free_count() } as u64);
    drivers::serial::serial_write(" free pages)\n");

    // CHECKPOINT 5 - ROJO: Antes de GPU init
    if bi.fb_addr != 0 {
        unsafe {
            let fb = bi.fb_addr as *mut u32;
            let offset = bi.fb_width as usize * 40;
            // Franja roja (0x00FF0000) debajo del magenta
            for i in 0..(bi.fb_width as usize * 10) {
                *fb.add(offset + i) = 0x00FF0000;
            }
        }
    }

    // ── GPU: Find NVIDIA GA106, map BAR0, load GSP firmware ───────────────
    drivers::serial::serial_write("[FastOS] Looking for NVIDIA GPU...\n");
    let gpu_pci = nv_hal::find_gpu(&platform::FastOsPlatform::new());
    if let Some(gpu_addr) = gpu_pci {
        drivers::serial::serial_write("[FastOS] GPU found on PCI bus\n");

        // Power on + enable bus mastering
        nv_hal::set_power_d0(&platform::FastOsPlatform::new(), gpu_addr);
        nv_hal::enable_bus_master(&platform::FastOsPlatform::new(), gpu_addr);

        // Read BAR0 physical address
        let bar0_phys = nv_hal::read_bar0(&platform::FastOsPlatform::new(), gpu_addr);
        drivers::serial::serial_write("[FastOS] GPU BAR0: ");
        serial_hex(bar0_phys);
        drivers::serial::serial_write("\n");

        // Map BAR0 (16 MB register space, identity-mapped)
        let bar0 = unsafe { nv_hal::MmioRegion::new(bar0_phys as *mut u8, 16 * 1024 * 1024) };

        // Load GSP firmware if bootloader provided it
        if bi.gsp_addr != 0 && bi.gsp_size != 0 {
            drivers::serial::serial_write("[FastOS] GSP firmware available: ");
            serial_hex(bi.gsp_size);
            drivers::serial::serial_write(" bytes at ");
            serial_hex(bi.gsp_addr);
            drivers::serial::serial_write("\n");

            // Create firmware slice from bootloader-loaded data
            let fw_blob = unsafe {
                core::slice::from_raw_parts(bi.gsp_addr as *const u8, bi.gsp_size as usize)
            };

            // Initialize console early for GSP diagnostics (if FB available)
            if bi.fb_addr != 0 {
                let mut con = console::Console::new(bi.fb_addr, bi.fb_pitch());
                con.clear();

                // Run GSP init sequence (PRIV Ring → DMA → Falcon boot → handshake)
                match drivers::gsp::gsp_init(&bar0, fw_blob, &mut con) {
                    Ok(()) => {
                        drivers::serial::serial_write("[FastOS] GSP firmware loaded OK!\n");
                    }
                    Err(_) => {
                        drivers::serial::serial_write("[FastOS] GSP load failed (non-fatal)\n");
                    }
                }

                // Continue to shell
                shell::run(&mut con);
            }
        } else {
            drivers::serial::serial_write("[FastOS] No GSP firmware (gsp_ga10x.bin not on ESP)\n");
        }
    } else {
        drivers::serial::serial_write("[FastOS] NVIDIA GPU not found on PCI bus\n");
    }

    // ── Fallback Console / Shell (no GPU or no FB) ───────────────────────
    if bi.fb_addr != 0 {
        drivers::serial::serial_write("[FastOS] Framebuffer detected, launching shell\n");
        let mut con = console::Console::new(bi.fb_addr, bi.fb_pitch());
        con.clear();
        shell::run(&mut con);
    }

    drivers::serial::serial_write("[FastOS] No framebuffer — serial-only mode\n");
    drivers::serial::serial_write("[FastOS] Halting.\n");
    loop { unsafe { core::arch::asm!("hlt"); } }
}
