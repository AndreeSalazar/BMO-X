//! FastOS / BMO Kernel — Entry Point.
//!
//! Recibe control desde el bootloader UEFI en Long Mode 64-bit, Ring 0.
//! RDI = *const fastos_boot_protocol::BootInfo.
//!
//! Boot path (delgado, lo que falta lo invoca el shell bajo demanda):
//!
//!   1. serial init
//!   2. validate BootInfo magic
//!   3. globals (FB + optional reserved payload)
//!   4. arch: GDT+TSS → IDT → syscall MSRs
//!   5. ACPI MCFG → PCI ECAM
//!   6. page allocator
//!   7. console + shell
//!
//! El backend gráfico funcional es UEFI GOP/framebuffer. Los prototipos de GPU
//! acelerada quedan fuera del build activo hasta que exista un driver real.

#![no_std]
#![no_main]

extern crate alloc;

mod allocator;
mod arch;
mod boot_info;
mod console;
mod diag;
mod desktop;
mod drivers;
mod fb;
mod font;
mod fs;          // sólo traits DiskReader/DiskWriter para los drivers
mod panic;
mod shell;

// BareX + BEF + scheduler + syscall + sandbox (Ring 0/Ring 3).
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

#[unsafe(no_mangle)]
#[link_section = ".text._start"]
#[unsafe(naked)]
unsafe extern "C" fn _start() -> ! {
    naked_asm!(
        "test rdi, rdi",
        "jz 2f",
        "mov rbx, rdi",
        "and rsp, -16",
        "mov rdi, rbx",
        "call kernel_main",
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
        "call kernel_main_real",
        "3: hlt",
        "jmp 3b"
    );
}

#[unsafe(no_mangle)]
#[inline(never)]
extern "C" fn kernel_main_real(boot_info_ptr: *const fastos_boot_protocol::BootInfo) -> ! {
    drivers::serial::init_serial();
    drivers::serial::serial_write("[FastOS] BMO Kernel v0.9.0 starting (slim boot)\n");

    if boot_info_ptr.is_null() {
        drivers::serial::serial_write("[FastOS] FATAL: boot_info_ptr is NULL!\n");
        loop { unsafe { core::arch::asm!("hlt"); } }
    }
    let bi = unsafe { &*boot_info_ptr };
    if bi.magic != fastos_boot_protocol::BOOT_MAGIC {
        drivers::serial::serial_write("[FastOS] FATAL: BootInfo magic ");
        serial_hex(bi.magic);
        drivers::serial::serial_write("\n");
        loop { unsafe { core::arch::asm!("hlt"); } }
    }

    // ── Globals que consumen módulos cliente (desktop/syscalls) ──────
    unsafe {
        boot_info::BOOT_INFO = boot_info_ptr;
        boot_info::RESERVED_PAYLOAD_ADDR = bi.gsp_addr;
        boot_info::RESERVED_PAYLOAD_SIZE = bi.gsp_size;
        boot_info::FB_ADDR = bi.fb_addr;
        boot_info::FB_WIDTH = bi.fb_width;
        boot_info::FB_HEIGHT = bi.fb_height;
        boot_info::FB_STRIDE = bi.fb_stride;
    }
    diag::init();
    diag::info_u64("boot", "framebuffer base", bi.fb_addr);
    drivers::serial::serial_write("[FastOS] FB "); serial_hex(bi.fb_addr);
    drivers::serial::serial_write(" "); serial_hex(bi.fb_width as u64);
    drivers::serial::serial_write("x"); serial_hex(bi.fb_height as u64);
    drivers::serial::serial_write("\n");

    // ── Ring 0 protected mode ───────────────────────────────────────
    arch::gdt::init_gdt();
    diag::info("arch", "GDT+TSS loaded; Ring0/Ring3 descriptors ready");
    drivers::serial::serial_write("[FastOS] GDT+TSS loaded (Ring0/Ring3 active)\n");

    arch::idt::init_idt();
    diag::info("arch", "IDT loaded");
    drivers::serial::serial_write("[FastOS] IDT loaded\n");

    arch::syscall_entry::init_syscall();
    diag::info("syscall", "BMO ABI syscall MSRs programmed");
    drivers::serial::serial_write("[FastOS] syscall MSRs programmed (BMO ABI)\n");

    // AMD Ryzen 5 5600X CPU optimizations (SSE, AVX, AVX2, XSAVE)
    arch::cpu_amd::init();

    // ── ACPI / PCI (sólo enumeración; sin driver GPU dedicado) ──────
    if let Some(ecam) = arch::acpi::parse_mcfg(bi.rsdp_addr) {
        drivers::pci::init_ecam(ecam.base_addr, ecam.end_bus);
        let pci = drivers::pci::scan_pci_bus();
        unsafe { drivers::pci::SCAN_RESULT = Some(pci); }
        let pci = unsafe { drivers::pci::SCAN_RESULT.as_ref().unwrap() };
        diag::info_u64("pci", "devices discovered", pci.count as u64);
        drivers::serial::serial_write("[FastOS] PCI devices: ");
        serial_hex(pci.count as u64);
        drivers::serial::serial_write("\n");
    } else {
        diag::warn("acpi", "MCFG not found; PCI ECAM unavailable");
        drivers::serial::serial_write("[FastOS] WARN: MCFG not found\n");
    }

    // ── Page allocator ──────────────────────────────────────────────
    unsafe {
        arch::page_alloc::init(
            &bi.memory_map,
            bi.memory_map_count as usize,
            bi.gsp_addr,
            bi.gsp_size,
            bi.kernel_base,
            bi.kernel_size,
        );
    }
    diag::info_u64("memory", "free pages", unsafe { arch::page_alloc::free_count() } as u64);
    drivers::serial::serial_write("[FastOS] Page allocator ready (");
    serial_hex(unsafe { arch::page_alloc::free_count() } as u64);
    drivers::serial::serial_write(" free pages)\n");

    // ── Storage deferred ────────────────────────────────────────────
    // El último panic reportado por diag/ cae justo después de `memory`,
    // antes de GOP/APIC/welcome; por lo tanto el sospechoso inmediato es
    // USB/BMO-FS. No debe bloquear `Run -> Desktop`, así que lo sacamos
    // del boot crítico hasta tener persistencia segura en diag/.
    diag::warn("storage", "USB/BMO-FS init deferred; desktop boot has priority");
    drivers::serial::serial_write("[FastOS] Storage deferred: USB/BMO-FS not initialized in boot path\n");

    // ── GOP Display ─────────────────────────────────────────────────
    if bi.fb_addr != 0 {
        drivers::gop::init_gop(bi.fb_addr, bi.fb_width, bi.fb_height, bi.fb_stride);
        diag::info("gop", "GOP display initialized");
        drivers::serial::serial_write("[FastOS] GOP display initialized\n");
    }

    // ── APIC Timer (100 Hz = 10ms ticks for scheduling) ────────────
    arch::apic::init_apic(100);
    diag::info("apic", "APIC timer started at 100 Hz");
    drivers::serial::serial_write("[FastOS] APIC timer started (100 Hz)\n");

    // ── Console + shell ─────────────────────────────────────────────
    if bi.fb_addr == 0 {
        diag::fault("boot", "no framebuffer; cannot start visual desktop");
        drivers::serial::serial_write("[FastOS] no framebuffer — halt\n");
        loop { unsafe { core::arch::asm!("hlt"); } }
    }
    let mut con = console::Console::new(
        bi.fb_addr, bi.fb_pitch(), bi.fb_stride, bi.fb_width, bi.fb_height,
    );
    con.clear();

    // ── Startup banner ──────────────────────────────────────────────
    con.println("================================================================");
    con.println("  FastOS v0.9.0 — Bare Metal Orchestrator");
    con.println("  Ring 0/3 | GDT+TSS | Syscall/Sysret | APIC Timer | GOP");
    con.println("================================================================");
    con.print("  CPU: Ryzen 5 5600X (Zen 3) | ");
    con.print("FB: "); con.print_u64(bi.fb_width as u64);
    con.print("x"); con.print_u64(bi.fb_height as u64); con.println("");
    con.print("  Free pages: "); con.print_u64(unsafe { arch::page_alloc::free_count() } as u64);
    con.print(" | Processes: "); con.print_u64(sched::process::process_count() as u64);
    con.print(" | Threads: "); con.print_u64(sched::thread::ready_count() as u64);
    con.println("");
    con.println("================================================================");
    con.println("");

    // ── Enable interrupts ───────────────────────────────────────────
    arch::cpu::sti();
    diag::info("boot", "interrupts enabled; launching welcome");
    drivers::serial::serial_write("[FastOS] Interrupts enabled (STI)\n");

    // ── Welcome screen Ring 0 → escribe (Run) → escritorio ──────────
    // (El banner de arriba queda 1 frame antes de que welcome pinte
    // su tarjeta encima; eso da feedback de progreso durante el boot.)
    drivers::serial::serial_write("[FastOS] launching welcome screen — type 'Run' on the keyboard\n");
    diag::info("welcome", "type Run + Enter to launch desktop");
    desktop::welcome::run();
}
