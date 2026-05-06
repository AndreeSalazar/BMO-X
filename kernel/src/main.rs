//! FastOS Kernel v0.6.0 — Entry Point
//!
//! Receives control from UEFI bootloader in 64-bit long mode, Ring 0.
//! RDI = *const fastos_boot_protocol::BootInfo

#![no_std]
#![no_main]

mod arch;
mod boot_info;
mod console;
mod drivers;
mod fb;
mod gpu;
mod render3d;
mod font;
mod panic;
mod platform;
mod shell;
mod tests;

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
        // CHECKPOINT 1 - VERDE: Pantalla completa
        "mov rcx, 2073600",
        "mov edx, 0x0000FF00",
        "0: mov [rax], edx",
        "add rax, 4",
        "dec rcx",
        "jnz 0b",
        // Recargar fb_addr para cyan
        "mov rax, [rbx + 8]",
        "test rax, rax",
        "jz 1f",
        // CHECKPOINT 2 - CYAN: Pantalla completa antes del call
        "mov rcx, 2073600",
        "mov edx, 0x0000FFFF",
        "4: mov [rax], edx",
        "add rax, 4",
        "dec rcx",
        "jnz 4b",
        "1:",
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
        // CHECKPOINT 1 - AZUL: Primera línea de kernel_main (sin prólogo)
        "test rdi, rdi",
        "jz 1f",
        "mov rbx, rdi",
        "mov rax, [rbx + 8]",  // fb_addr
        "test rax, rax",
        "jz 1f",
        "mov ecx, dword ptr [rbx + 24]",  // fb_width (4 bytes)
        "imul rcx, rcx, 10",
        "mov edx, 0x00FF0000",
        "2: mov [rax], edx",
        "add rax, 4",
        "dec rcx",
        "jnz 2b",
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

    // ── Run shell ─────────────────────────────────────────────────────────
    match bi.fb_addr {
        0 => loop { unsafe { core::arch::asm!("hlt"); } },
        fb_addr => {
            // Debug: Paint color based on fb_stride value
            if bi.fb_addr != 0 {
                unsafe {
                    let fb = bi.fb_addr as *mut u32;
                    let offset = bi.fb_width as usize * 50;
                    let color = if bi.fb_stride > 2000 { 0x00FF00FF } else { 0x00FFFF00 }; // PURPLE if >2000, YELLOW if <=2000
                    for i in 0..(bi.fb_width as usize * 10) {
                        *fb.add(offset + i) = color;
                    }
                }
            }

            let mut con = console::Console::new(fb_addr, bi.fb_pitch(), bi.fb_width, bi.fb_height);
            con.clear(); // Limpiar la pantalla de las franjas de debug antes de iniciar el shell
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
            
            // ── Iniciar GSP (TEMPORALMENTE DESACTIVADO) ─────────────────────
            con.println("[FastOS] Buscando GPU para cargar GSP...");
            let platform = platform::FastOsPlatform::new();
            if let Some(pci) = nv_hal::find_gpu(&platform) {
                use nv_hal::Platform;
                let bar0_phys = nv_hal::read_bar0(&platform, pci);
                if bar0_phys != 0 && bar0_phys != 0xFFFF_FFFF_FFFF_FFF0 {
                    // BAR0 es típicamente de 16MB
                    let bar0_ptr = platform.map_mmio(bar0_phys, 16 * 1024 * 1024);
                    if !bar0_ptr.is_null() {
                        let bar0 = unsafe { nv_hal::MmioRegion::new(bar0_ptr, 16 * 1024 * 1024) };
                        if bi.gsp_addr != 0 && bi.gsp_size > 0 {
                            let fw_blob = unsafe { core::slice::from_raw_parts(bi.gsp_addr as *const u8, bi.gsp_size as usize) };
                            let gsp_result =
                                if bi.gsp_bootloader_addr != 0 && bi.gsp_bootloader_size > 0 &&
                                   bi.gsp_booter_load_addr != 0 && bi.gsp_booter_load_size > 0 {
                                    let bootloader = unsafe {
                                        core::slice::from_raw_parts(
                                            bi.gsp_bootloader_addr as *const u8,
                                            bi.gsp_bootloader_size as usize,
                                        )
                                    };
                                    let booter_load = unsafe {
                                        core::slice::from_raw_parts(
                                            bi.gsp_booter_load_addr as *const u8,
                                            bi.gsp_booter_load_size as usize,
                                        )
                                    };
                                    let vbios_rom = if bi.vbios_addr != 0 && bi.vbios_size > 0 {
                                        Some(unsafe {
                                            core::slice::from_raw_parts(
                                                bi.vbios_addr as *const u8,
                                                bi.vbios_size as usize,
                                            )
                                        })
                                    } else {
                                        None
                                    };
                                    let blobs = crate::drivers::gsp::GspFirmwareBlobs {
                                        gsp_rm: fw_blob,
                                        bootloader,
                                        booter_load,
                                        vbios_rom,
                                    };
                                    crate::drivers::gsp::gsp_init_full(&bar0, &blobs, &mut con)
                                } else {
                                    con.print_colored("[FastOS] AVISO: blobs GSP separados incompletos; usando modo ELF unico.\n", 0xFFFFFF00);
                                    crate::drivers::gsp::gsp_init(&bar0, fw_blob, &mut con)
                                };

                            if let Err(_e) = gsp_result {
                                con.print_colored("[FastOS] ERROR: No se pudo arrancar el GSP.\n", 0xFFFF0000);
                            } else {
                                con.print_colored("[FastOS] EXITO: GSP Bootloader handshake OK.\n", 0xFF00FF00);
                            }
                        } else {
                            con.print_colored("[FastOS] ERROR: Firmware gsp_ga10x.bin no cargado por bootloader.\n", 0xFFFF0000);
                        }
                    } else {
                        con.println("[FastOS] ERROR: Mapeo MMIO BAR0 fallido.");
                    }
                } else {
                    con.println("[FastOS] ERROR: BAR0 Invalido.");
                }
            } else {
                con.println("[FastOS] ERROR: GPU no encontrada.");
            }
            con.println("");

            shell::run(&mut con);
            loop { unsafe { core::arch::asm!("hlt"); } }
        }
    }
}
