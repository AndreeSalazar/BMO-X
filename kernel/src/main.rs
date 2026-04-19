//! FastOS Kernel v0.2.0 - Entry Point
//!
//! Receives control from stage2 in 64-bit long mode, Ring 0.
//! SSE/AVX2 initialized. RDI = pointer to BootInfo.

#![no_std]
#![no_main]

mod arch;
mod console;
mod drivers;
mod fb;
mod fs;
mod vga;
mod panic;
mod platform;
mod shell;

use fb::{Framebuffer, colors};
use vga::VgaWriter;
use console::Console;

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
    pub fb_pitch: u64,
    pub vbe_mode: u64,
}

#[unsafe(no_mangle)]
pub extern "C" fn _start(boot_info: *const BootInfo) -> ! {
    let info = unsafe { &*boot_info };

    // Validate boot info early
    if info.magic != 0xFA5705 {
        let mut vga = VgaWriter::new();
        vga.clear();
        vga.write_str_color("ERROR: Invalid boot magic!", vga::Color::Red);
        halt_loop();
    }

    let is_graphics = info.vbe_mode == 1;

    // ── Phase 1: Quick text boot log ────────────────────────────────────
    let mut vga = VgaWriter::from_boot_info(
        info.framebuffer_addr, info.fb_pitch, info.vbe_mode,
    );
    vga.clear();
    vga.write_str_color("FastOS v0.2.0 - Booting...", vga::Color::LightCyan);
    vga.newline();

    // Serial
    drivers::serial::init_serial();
    drivers::serial::serial_write("[FastOS] Kernel starting\n");

    // CPU
    let _cpu = arch::cpu::detect_cpu();
    vga.write_str("[CPU] Zen 3 OK  ");

    // Interrupts: IDT + PIC + PIT
    arch::pic::init_pic();
    arch::idt::init_idt();
    arch::pit::init_pit();
    arch::idt::register_irq(0, arch::pit::tick);
    arch::pic::set_mask_keyboard_timer();
    arch::cpu::sti();
    vga.write_str_color("[IRQ] OK  ", vga::Color::Green);
    drivers::serial::serial_write("[FastOS] Interrupts enabled (PIC+PIT+KB)\n");

    // PCI + GPU
    let devices = drivers::pci::scan_pci_bus();
    vga.write_str("[PCI] ");
    vga.write_u64(devices.count as u64);
    vga.write_str(" devs  ");

    let mut gpu_ok = false;
    if let Some(gpu_pci) = devices.find_nvidia_gpu() {
        if gpu_pci.device_id == 0x2504 {
            match drivers::gpu::rtx3060::init_gpu_driver() {
                Ok(_) => {
                    gpu_ok = true;
                    vga.write_str_color("[GPU] OK  ", vga::Color::Green);
                }
                Err(_e) => {
                    vga.write_str_color("[GPU] FAIL  ", vga::Color::Red);
                }
            }
        }
    }

    // Keyboard init
    drivers::keyboard::init_keyboard();
    vga.write_str_color("[KB] OK", vga::Color::Green);
    vga.newline();

    drivers::serial::serial_write("[FastOS] Keyboard ready\n");

    // ── Phase 2: Graphics boot screen ───────────────────────────────────
    if is_graphics {
        vga.write_str("[GFX] Boot screen...");
        vga.newline();

        let fb = Framebuffer::new(info.framebuffer_addr, info.fb_pitch);
        draw_boot_screen(&fb, gpu_ok);

        // Wait ~3 seconds (spin loop, no timer needed)
        for _ in 0..150_000_000u32 { core::hint::spin_loop(); }

        // ── Phase 3: Shell ──────────────────────────────────────────────
        let mut con = Console::new(info.framebuffer_addr, info.fb_pitch);
        con.clear();
        shell::run(&mut con);
    } else {
        vga.newline();
        vga.write_str_color("FastOS initialized! (VGA text, no shell)", vga::Color::LightGreen);
    }

    halt_loop();
}

// ── Boot Screen Graphics ────────────────────────────────────────────────────

fn draw_boot_screen(fb: &Framebuffer, gpu_ok: bool) {
    fb.gradient_v(0, 0, 1920, 1080, 0xFF080C12, 0xFF0D1117);

    // Top accent
    fb.gradient_h(0, 0, 1920, 3, colors::NV_GREEN, colors::ACCENT_CYAN);

    // Header
    fb.fill_rounded_rect(60, 40, 1800, 100, 12, colors::BG_PANEL);
    fb.draw_rect(60, 40, 1800, 100, colors::BORDER, 1);

    // Logo
    fb.fill_circle(120, 90, 28, colors::NV_GREEN);
    fb.fill_circle(120, 90, 22, colors::BG_PANEL);
    fb.fill_rect(112, 72, 4, 36, colors::NV_GREEN);
    fb.fill_rect(112, 72, 18, 4, colors::NV_GREEN);
    fb.fill_rect(112, 86, 14, 4, colors::NV_GREEN);

    draw_title(fb, 170, 68);
    draw_text(fb, 170, 108, "Bare Metal OS - Ring 0 Kernel", colors::TEXT_SECONDARY);

    // System panel
    fb.fill_rounded_rect(60, 170, 880, 500, 12, colors::BG_PANEL);
    fb.draw_rect(60, 170, 880, 500, colors::BORDER, 1);
    draw_text(fb, 90, 190, "SYSTEM INFORMATION", colors::ACCENT_BLUE);
    fb.hline(90, 210, 820, colors::BORDER);

    draw_text(fb, 90, 230, "CPU    AMD Ryzen 5 5600X (Zen 3)", colors::TEXT_PRIMARY);
    draw_text(fb, 90, 260, "GPU    NVIDIA RTX 3060 12G (GA106)", colors::TEXT_PRIMARY);
    draw_text(fb, 90, 290, "VRAM   12288 MB GDDR6", colors::TEXT_PRIMARY);

    if gpu_ok {
        draw_text(fb, 90, 320, "Driver Ring 0 loaded OK", colors::TEXT_SUCCESS);
        fb.fill_circle(80, 328, 4, colors::NV_GREEN);
    }

    draw_text(fb, 90, 360, "Board  MSI MAG B550 TOMAHAWK", colors::TEXT_PRIMARY);
    draw_text(fb, 90, 390, "Mode   1920x1080x32bpp VBE LFB", colors::TEXT_PRIMARY);
    draw_text(fb, 90, 420, "Boot   MBR > Stage2 > Long Mode > Rust", colors::TEXT_PRIMARY);
    draw_text(fb, 90, 460, "Keyb   PS/2 polling mode", colors::TEXT_SECONDARY);
    draw_text(fb, 90, 490, "Stack  0x800000 (8MB) Ring 0, no_std", colors::TEXT_SECONDARY);

    // Demo panel
    fb.fill_rounded_rect(970, 170, 890, 240, 12, colors::BG_PANEL);
    fb.draw_rect(970, 170, 890, 240, colors::BORDER, 1);
    draw_text(fb, 1000, 190, "FRAMEBUFFER DEMO", colors::ACCENT_BLUE);
    fb.hline(1000, 210, 830, colors::BORDER);
    fb.gradient_h(1000, 225, 830, 30, 0xFFFF0000, 0xFF0000FF);
    fb.gradient_h(1000, 265, 830, 30, colors::NV_GREEN, colors::ACCENT_CYAN);
    fb.gradient_h(1000, 305, 830, 30, colors::ACCENT_PURPLE, colors::ACCENT_PINK);

    let cc = [colors::ACCENT_RED, colors::ACCENT_ORANGE, colors::NV_GREEN,
              colors::ACCENT_CYAN, colors::ACCENT_BLUE, colors::ACCENT_PURPLE,
              colors::ACCENT_PINK, colors::TEXT_SUCCESS];
    for (i, &c) in cc.iter().enumerate() {
        fb.fill_circle(1050 + i * 105, 375, 30, c);
    }

    // Loading message
    fb.fill_rounded_rect(970, 440, 890, 60, 12, colors::BG_PANEL);
    draw_text(fb, 1000, 460, "Starting shell in 3 seconds...", colors::ACCENT_CYAN);

    // Bottom bar
    fb.fill_rect(0, 1050, 1920, 30, colors::BAR_BG);
    fb.hline(0, 1050, 1920, colors::BORDER);
    draw_text(fb, 20, 1057, "FastOS v0.2.0", colors::NV_GREEN);
    draw_text(fb, 300, 1057, "Ryzen 5 5600X + RTX 3060 12G", colors::TEXT_SECONDARY);
    draw_text(fb, 1600, 1057, "System Ready", colors::TEXT_SUCCESS);

    fb.gradient_h(0, 1077, 1920, 3, colors::ACCENT_CYAN, colors::NV_GREEN);
}

fn draw_text(fb: &Framebuffer, x: usize, y: usize, text: &str, color: u32) {
    let mut cx = x;
    for byte in text.bytes() {
        if byte >= 32 && byte <= 126 {
            let glyph = vga::get_glyph(byte);
            for gy in 0..16usize {
                let row = glyph[gy];
                for gx in 0..8usize {
                    if row & (0x80 >> gx) != 0 {
                        fb.put_pixel(cx + gx, y + gy, color);
                    }
                }
            }
        }
        cx += 8;
    }
}

fn draw_title(fb: &Framebuffer, x: usize, y: usize) {
    let mut cx = x;
    for byte in "FastOS".bytes() {
        let glyph = vga::get_glyph(byte);
        let scale = 3;
        for gy in 0..16usize {
            let row = glyph[gy];
            for gx in 0..8usize {
                if row & (0x80 >> gx) != 0 {
                    fb.fill_rect(cx + gx * scale, y + gy * scale, scale, scale, colors::TEXT_PRIMARY);
                }
            }
        }
        cx += 24;
    }
}

pub fn halt_loop() -> ! {
    loop { unsafe { core::arch::asm!("hlt"); } }
}
