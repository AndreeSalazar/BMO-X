//! FastOS Kernel - Entry Point
//!
//! Receives control from stage2 in 64-bit long mode, Ring 0.
//! SSE/AVX2 initialized. RDI = pointer to BootInfo.

#![no_std]
#![no_main]

mod arch;
mod drivers;
mod fb;
mod fs;
mod vga;
mod panic;
mod platform;

use fb::{Framebuffer, colors};
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
    pub fb_pitch: u64,
    pub vbe_mode: u64,
}

#[unsafe(no_mangle)]
pub extern "C" fn _start(boot_info: *const BootInfo) -> ! {
    let info = unsafe { &*boot_info };

    // Create text writer
    let mut vga = VgaWriter::from_boot_info(
        info.framebuffer_addr,
        info.fb_pitch,
        info.vbe_mode,
    );
    vga.clear();

    // Validate boot info
    if info.magic != 0xFA5705 {
        vga.write_str_color("ERROR: Invalid boot magic!", vga::Color::Red);
        halt_loop();
    }

    // ── Phase 1: Text boot log ──────────────────────────────────────────
    vga.write_str_color("FastOS v0.1.0 - Ryzen 5 5600X + RTX 3060 12G", vga::Color::LightCyan);
    vga.newline();
    vga.write_separator();

    if vga.is_graphics_mode() {
        vga.write_str_color("[OK] ", vga::Color::Green);
        vga.write_str("VBE 1920x1080x32bpp");
    } else {
        vga.write_str_color("[OK] ", vga::Color::Yellow);
        vga.write_str("VGA 80x25 text");
    }
    vga.newline();

    // CPU
    let cpu = arch::cpu::detect_cpu();
    vga.write_str_color("[CPU] ", vga::Color::LightCyan);
    vga.write_str("Zen 3: SSE4.2 AVX2 FMA3 AES SHA BMI2");
    vga.newline();

    // Serial
    drivers::serial::init_serial();

    // PCI + GPU
    let devices = drivers::pci::scan_pci_bus();
    vga.write_str("[PCI] ");
    vga.write_u64(devices.count as u64);
    vga.write_str(" devices");
    vga.newline();

    let mut gpu_ok = false;
    let mut gpu_vram_mb: u64 = 0;
    let mut gpu_chip: u32 = 0;

    if let Some(gpu_pci) = devices.find_nvidia_gpu() {
        if gpu_pci.device_id == 0x2504 {
            vga.write_str_color("[GPU] ", vga::Color::Green);
            vga.write_str("GA106 RTX 3060 12G - ");

            match drivers::gpu::rtx3060::init_gpu_driver() {
                Ok(driver_state) => {
                    let gi = drivers::gpu::rtx3060::gpu_info(&driver_state);
                    gpu_vram_mb = gi.vram_size_mb;
                    gpu_chip = gi.chip_id;
                    gpu_ok = true;
                    vga.write_str_color("Ring 0 OK", vga::Color::Green);
                }
                Err(e) => {
                    vga.write_str_color("FAILED: ", vga::Color::Red);
                    vga.write_str(e.description());
                }
            }
            vga.newline();
        }
    }

    // ── Phase 2: Graphical boot screen ──────────────────────────────────
    if vga.is_graphics_mode() {
        vga.newline();
        vga.write_str_color("[GFX] ", vga::Color::LightCyan);
        vga.write_str("Drawing framebuffer demo...");
        vga.newline();

        // Small delay so user sees text
        for _ in 0..50_000_000u32 { core::hint::spin_loop(); }

        let fb = Framebuffer::new(info.framebuffer_addr, info.fb_pitch);
        draw_boot_screen(&fb, &cpu, gpu_ok, gpu_vram_mb, gpu_chip);
    } else {
        vga.newline();
        vga.write_str_color("FastOS kernel initialized!", vga::Color::LightGreen);
        vga.newline();
        vga.write_str("System ready. Halted.");
    }

    halt_loop();
}

// ── Boot Screen Graphics ────────────────────────────────────────────────────

fn draw_boot_screen(
    fb: &Framebuffer,
    _cpu: &arch::cpu::CpuFeatures,
    gpu_ok: bool,
    vram_mb: u64,
    chip_id: u32,
) {
    // 1. Dark gradient background
    fb.gradient_v(0, 0, 1920, 1080, 0xFF080C12, 0xFF0D1117);

    // 2. Top accent line (NVIDIA green gradient)
    fb.gradient_h(0, 0, 1920, 3, colors::NV_GREEN, colors::ACCENT_CYAN);

    // 3. Header area
    fb.fill_rounded_rect(60, 40, 1800, 100, 12, colors::BG_PANEL);
    fb.draw_rect(60, 40, 1800, 100, colors::BORDER, 1);

    // FastOS logo text area — draw "F" logo circle
    fb.fill_circle(120, 90, 28, colors::NV_GREEN);
    fb.fill_circle(120, 90, 22, colors::BG_PANEL);
    fb.fill_rect(112, 72, 4, 36, colors::NV_GREEN);   // F vertical
    fb.fill_rect(112, 72, 18, 4, colors::NV_GREEN);    // F top
    fb.fill_rect(112, 86, 14, 4, colors::NV_GREEN);    // F middle

    // "FastOS" text rendered as pixel blocks (8px wide chars)
    draw_title(fb, 170, 68);

    // Version + subtitle
    draw_small_text(fb, 170, 108, "Bare Metal OS - Ring 0 Kernel", colors::TEXT_SECONDARY);

    // 4. System info panel
    fb.fill_rounded_rect(60, 170, 880, 500, 12, colors::BG_PANEL);
    fb.draw_rect(60, 170, 880, 500, colors::BORDER, 1);

    // Panel title
    draw_small_text(fb, 90, 190, "SYSTEM INFORMATION", colors::ACCENT_BLUE);
    fb.hline(90, 210, 820, colors::BORDER);

    // CPU info
    draw_small_text(fb, 90, 230, "CPU", colors::ACCENT_PURPLE);
    draw_small_text(fb, 200, 230, "AMD Ryzen 5 5600X (Zen 3)", colors::TEXT_PRIMARY);

    // CPU features as colored badges
    let features = ["SSE4.2", "AVX2", "FMA3", "AES-NI", "SHA", "BMI2"];
    let badge_colors = [
        colors::ACCENT_BLUE, colors::NV_GREEN, colors::ACCENT_PURPLE,
        colors::ACCENT_CYAN, colors::ACCENT_ORANGE, colors::ACCENT_PINK,
    ];
    let mut bx = 90;
    for (i, feat) in features.iter().enumerate() {
        let w = feat.len() * 8 + 16;
        fb.fill_rounded_rect(bx, 260, w, 26, 6, badge_colors[i % badge_colors.len()]);
        draw_small_text(fb, bx + 8, 265, feat, colors::BLACK);
        bx += w + 10;
    }

    // GPU info
    fb.hline(90, 305, 820, colors::BORDER);
    draw_small_text(fb, 90, 320, "GPU", colors::ACCENT_PURPLE);
    if gpu_ok {
        draw_small_text(fb, 200, 320, "NVIDIA RTX 3060 12G (GA106)", colors::TEXT_PRIMARY);
        draw_small_text(fb, 90, 350, "VRAM", colors::TEXT_SECONDARY);
        draw_vram_bar(fb, 200, 348, 600, 20, vram_mb, 12288);
        draw_small_text(fb, 90, 382, "Chip", colors::TEXT_SECONDARY);
        draw_small_text(fb, 200, 382, "0xB76000A1 (Ampere A1)", colors::TEXT_PRIMARY);
        draw_small_text(fb, 90, 412, "Status", colors::TEXT_SECONDARY);
        draw_small_text(fb, 200, 412, "Ring 0 Driver Loaded", colors::TEXT_SUCCESS);

        // NVIDIA green indicator dot
        fb.fill_circle(185, 420, 5, colors::NV_GREEN);
    } else {
        draw_small_text(fb, 200, 320, "Driver not loaded", colors::ACCENT_RED);
    }

    // Memory info
    fb.hline(90, 445, 820, colors::BORDER);
    draw_small_text(fb, 90, 460, "MEM", colors::ACCENT_PURPLE);
    draw_small_text(fb, 200, 460, "Framebuffer: 0xD0000000 (VBE LFB)", colors::TEXT_PRIMARY);
    draw_small_text(fb, 90, 490, "Mode", colors::TEXT_SECONDARY);
    draw_small_text(fb, 200, 490, "1920 x 1080 x 32bpp @ 60Hz", colors::TEXT_PRIMARY);

    // Boot info
    fb.hline(90, 525, 820, colors::BORDER);
    draw_small_text(fb, 90, 540, "BOOT", colors::ACCENT_PURPLE);
    draw_small_text(fb, 200, 540, "MBR -> Stage2 -> Long Mode -> Rust Kernel", colors::TEXT_PRIMARY);
    draw_small_text(fb, 90, 570, "Board", colors::TEXT_SECONDARY);
    draw_small_text(fb, 200, 570, "MSI MAG B550 TOMAHAWK (MS-7C52)", colors::TEXT_PRIMARY);
    draw_small_text(fb, 90, 600, "Stack", colors::TEXT_SECONDARY);
    draw_small_text(fb, 200, 600, "0x800000 (8MB) Ring 0, no_std", colors::TEXT_PRIMARY);
    draw_small_text(fb, 90, 630, "Driver", colors::TEXT_SECONDARY);
    draw_small_text(fb, 200, 630, "SigDead-BIB GA106 nv_kernel", colors::TEXT_PRIMARY);

    // 5. Color palette demo panel
    fb.fill_rounded_rect(970, 170, 890, 240, 12, colors::BG_PANEL);
    fb.draw_rect(970, 170, 890, 240, colors::BORDER, 1);
    draw_small_text(fb, 1000, 190, "FRAMEBUFFER DEMO", colors::ACCENT_BLUE);
    fb.hline(1000, 210, 830, colors::BORDER);

    // Gradient bars
    fb.gradient_h(1000, 225, 830, 30, 0xFFFF0000, 0xFF0000FF);
    fb.gradient_h(1000, 265, 830, 30, colors::NV_GREEN, colors::ACCENT_CYAN);
    fb.gradient_h(1000, 305, 830, 30, colors::ACCENT_PURPLE, colors::ACCENT_PINK);

    // Color circles
    let circle_colors = [
        colors::ACCENT_RED, colors::ACCENT_ORANGE, colors::NV_GREEN,
        colors::ACCENT_CYAN, colors::ACCENT_BLUE, colors::ACCENT_PURPLE,
        colors::ACCENT_PINK, colors::TEXT_SUCCESS,
    ];
    for (i, &c) in circle_colors.iter().enumerate() {
        fb.fill_circle(1050 + i * 105, 375, 30, c);
    }

    // 6. Architecture panel
    fb.fill_rounded_rect(970, 440, 890, 230, 12, colors::BG_PANEL);
    fb.draw_rect(970, 440, 890, 230, colors::BORDER, 1);
    draw_small_text(fb, 1000, 460, "ARCHITECTURE", colors::ACCENT_BLUE);
    fb.hline(1000, 480, 830, colors::BORDER);

    draw_small_text(fb, 1000, 500, "Ring 0   Bare metal, no OS abstraction", colors::TEXT_PRIMARY);
    draw_small_text(fb, 1000, 530, "AVX2     256-bit SIMD enabled", colors::TEXT_PRIMARY);
    draw_small_text(fb, 1000, 560, "MMIO     GPU registers mapped to BAR0", colors::TEXT_PRIMARY);
    draw_small_text(fb, 1000, 590, "PCI      40 devices enumerated", colors::TEXT_PRIMARY);
    draw_small_text(fb, 1000, 620, "VBE      VBIOS framebuffer active", colors::TEXT_PRIMARY);

    // 7. Bottom status bar
    fb.fill_rect(0, 1050, 1920, 30, colors::BAR_BG);
    fb.hline(0, 1050, 1920, colors::BORDER);
    draw_small_text(fb, 20, 1057, "FastOS v0.1.0", colors::NV_GREEN);
    draw_small_text(fb, 300, 1057, "Ryzen 5 5600X", colors::TEXT_SECONDARY);
    draw_small_text(fb, 550, 1057, "RTX 3060 12G", colors::TEXT_SECONDARY);
    draw_small_text(fb, 780, 1057, "1920x1080", colors::TEXT_SECONDARY);
    draw_small_text(fb, 1600, 1057, "System Ready", colors::TEXT_SUCCESS);

    // 8. Bottom accent line
    fb.gradient_h(0, 1077, 1920, 3, colors::ACCENT_CYAN, colors::NV_GREEN);
}

// ── VRAM progress bar ──────────────────────────────────────────────────────

fn draw_vram_bar(fb: &Framebuffer, x: usize, y: usize, w: usize, h: usize, used: u64, total: u64) {
    fb.fill_rounded_rect(x, y, w, h, 4, colors::BG_CARD);
    let fill_w = if total > 0 { ((used as usize) * (w - 4)) / (total as usize) } else { 0 };
    fb.fill_rounded_rect(x + 2, y + 2, fill_w, h - 4, 3, colors::NV_GREEN);

    // Label
    let label_x = x + w + 10;
    draw_small_text(fb, label_x, y + 2, "12288 MB", colors::TEXT_PRIMARY);
}

// ── Text rendering using built-in font ─────────────────────────────────────

fn draw_small_text(fb: &Framebuffer, x: usize, y: usize, text: &str, color: u32) {
    let mut cx = x;
    for byte in text.bytes() {
        if byte >= 32 && byte <= 126 {
            draw_glyph(fb, cx, y, byte, color);
        }
        cx += 8;
    }
}

fn draw_glyph(fb: &Framebuffer, x: usize, y: usize, ch: u8, color: u32) {
    let glyph = vga::get_glyph(ch);
    for gy in 0..16usize {
        let row = glyph[gy];
        for gx in 0..8usize {
            if row & (0x80 >> gx) != 0 {
                fb.put_pixel(x + gx, y + gy, color);
            }
        }
    }
}

// ── Big title text "FastOS" ────────────────────────────────────────────────

fn draw_title(fb: &Framebuffer, x: usize, y: usize) {
    let title = "FastOS";
    let mut cx = x;
    for byte in title.bytes() {
        draw_big_char(fb, cx, y, byte, colors::TEXT_PRIMARY);
        cx += 24; // 3x scale = 8*3 = 24px wide
    }
}

fn draw_big_char(fb: &Framebuffer, x: usize, y: usize, ch: u8, color: u32) {
    let glyph = vga::get_glyph(ch);
    let scale = 3;
    for gy in 0..16usize {
        let row = glyph[gy];
        for gx in 0..8usize {
            if row & (0x80 >> gx) != 0 {
                fb.fill_rect(x + gx * scale, y + gy * scale, scale, scale, color);
            }
        }
    }
}

pub fn halt_loop() -> ! {
    loop {
        unsafe { core::arch::asm!("hlt"); }
    }
}
