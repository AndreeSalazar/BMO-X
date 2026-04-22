//! FastOS Kernel v0.5.0 — Entry Point (checkpoint debug)
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
        "mov rcx, [rbx + 24]",  // fb_width
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

    // PIC/PIT/STI disabled for now — not needed until we want interrupts
    /*
    arch::pic::init_pic();
    arch::pic::set_mask_keyboard_timer();
    drivers::serial::serial_write("[FastOS] PIC initialized\n");

    arch::pit::init_pit();
    arch::idt::register_irq(0, arch::pit::tick);
    drivers::serial::serial_write("[FastOS] PIT @ 100Hz\n");

    unsafe { core::arch::asm!("sti"); }
    drivers::serial::serial_write("[FastOS] Interrupts enabled\n");
    */

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
    /*
    drivers::keyboard::init_keyboard();
    drivers::serial::serial_write("[FastOS] PS/2 keyboard ready\n");
    */

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
    /*
    unsafe {
        arch::page_alloc::init(&bi.memory_map, bi.memory_map_count as usize);
    }
    drivers::serial::serial_write("[FastOS] Page allocator initialized (");
    serial_hex(unsafe { arch::page_alloc::free_count() } as u64);
    drivers::serial::serial_write(" free pages)\n");
    */

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

            let mut con = console::Console::new(fb_addr, bi.fb_pitch());
            shell::run(&mut con);
            loop { unsafe { core::arch::asm!("hlt"); } }
        }
    }
}
