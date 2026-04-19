//! High-Level GPU Command Builders
//!
//! Builds pushbuffer command sequences for common operations:
//! - Framebuffer fill (solid color rectangle via CPU MMIO)
//! - DMA memory copy command (CE pushbuffer)
//! - NOP/fence commands

use super::fifo::GpuChannel;
use super::methods::*;

/// Build a NOP command (useful for testing pushbuffer construction).
pub fn cmd_nop(ch: &mut GpuChannel) {
    ch.push_method(0, NV_NOP, 0);
}

/// Build a CE DMA copy command in the pushbuffer.
/// Copies `byte_count` bytes from src_phys to dst_phys.
pub fn cmd_ce_copy(ch: &mut GpuChannel, src_phys: u64, dst_phys: u64, byte_count: u32) {
    let sc = SUBCHAN_CE;
    ch.push_method(sc, CE_SRC_ADDR_HI, (src_phys >> 32) as u32);
    ch.push_method(sc, CE_SRC_ADDR_LO, src_phys as u32);
    ch.push_method(sc, CE_DST_ADDR_HI, (dst_phys >> 32) as u32);
    ch.push_method(sc, CE_DST_ADDR_LO, dst_phys as u32);
    ch.push_method(sc, CE_SRC_PITCH, byte_count);
    ch.push_method(sc, CE_DST_PITCH, byte_count);
    ch.push_method(sc, CE_X_COUNT, byte_count);
    ch.push_method(sc, CE_Y_COUNT, 1);
    ch.push_method(sc, CE_LAUNCH_DMA, CE_LAUNCH_NON_PIPELINED | CE_SRC_TYPE_PHYS | CE_DST_TYPE_PHYS);
}

/// Build a 2D solid fill command in the pushbuffer.
/// Fills a rectangle on the framebuffer with a solid color.
pub fn cmd_2d_fill(ch: &mut GpuChannel, fb_phys: u64, pitch: u32,
                   width: u32, height: u32,
                   x: u32, y: u32, w: u32, h: u32, color: u32) {
    let sc = SUBCHAN_2D;
    ch.push_method(sc, M2D_DST_FORMAT, M2D_FORMAT_A8R8G8B8);
    ch.push_method(sc, M2D_DST_PITCH, pitch);
    ch.push_method(sc, M2D_DST_WIDTH, width);
    ch.push_method(sc, M2D_DST_HEIGHT, height);
    ch.push_method(sc, M2D_DST_ADDR_HI, (fb_phys >> 32) as u32);
    ch.push_method(sc, M2D_DST_ADDR_LO, fb_phys as u32);
    ch.push_method(sc, M2D_OPERATION, M2D_OP_SOLID_FILL);
    ch.push_method(sc, M2D_SOLID_COLOR, color);
    // Packed rectangle: start in high 16, end in low 16
    ch.push_method(sc, M2D_RENDER_SOLID_PRIM_X, (x << 16) | (x + w));
    ch.push_method(sc, M2D_RENDER_SOLID_PRIM_Y, (y << 16) | (y + h));
}

/// Build a semaphore release command (for synchronization).
pub fn cmd_semaphore_release(ch: &mut GpuChannel, sem_phys: u64, payload: u32) {
    ch.push_method(0, NV_SEMAPHORE_ADDR_HI, (sem_phys >> 32) as u32);
    ch.push_method(0, NV_SEMAPHORE_ADDR_LO, sem_phys as u32);
    ch.push_method(0, NV_SEMAPHORE_PAYLOAD, payload);
    ch.push_method(0, NV_SEMAPHORE_OP, 1); // release
}

/// CPU-side direct framebuffer fill via MMIO writes.
/// This doesn't use GPU commands — it writes directly to the framebuffer memory.
/// Works immediately because the VBE framebuffer is memory-mapped.
///
/// `fb_base`: Physical/virtual base of framebuffer (e.g. 0xD0000000).
/// `pitch`: Bytes per scanline (e.g. 1920*4 = 7680).
pub fn cpu_fb_fill_rect(fb_base: u64, pitch: u32,
                        x: u32, y: u32, w: u32, h: u32, color: u32) {
    let fb = fb_base as *mut u32;
    let stride = pitch / 4; // pixels per row
    for row in y..(y + h) {
        for col in x..(x + w) {
            let offset = (row * stride + col) as isize;
            unsafe {
                core::ptr::write_volatile(fb.offset(offset), color);
            }
        }
    }
}

/// CPU-side gradient fill — draws a visible gradient to prove GPU framebuffer access.
pub fn cpu_fb_gradient(fb_base: u64, pitch: u32, width: u32, height: u32,
                       x: u32, y: u32, w: u32, h: u32) {
    let fb = fb_base as *mut u32;
    let stride = pitch / 4;
    for row in y..(y + h) {
        for col in x..(x + w) {
            let r = ((col - x) * 255 / w) as u32;
            let g = ((row - y) * 255 / h) as u32;
            let b = 128u32;
            let color = 0xFF000000 | (r << 16) | (g << 8) | b;
            let offset = (row * stride + col) as isize;
            unsafe {
                core::ptr::write_volatile(fb.offset(offset), color);
            }
        }
    }
}

/// CPU-side draw the NVIDIA-green FastOS logo pattern.
pub fn cpu_fb_logo(fb_base: u64, pitch: u32,
                   center_x: u32, center_y: u32) {
    // Green rectangles forming "F" letter
    let green = 0xFF76B900u32; // NVIDIA green
    let dark = 0xFF1A1A2Eu32;

    // Background box
    cpu_fb_fill_rect(fb_base, pitch, center_x - 60, center_y - 50, 120, 100, dark);
    // F vertical bar
    cpu_fb_fill_rect(fb_base, pitch, center_x - 40, center_y - 35, 15, 70, green);
    // F top bar
    cpu_fb_fill_rect(fb_base, pitch, center_x - 40, center_y - 35, 60, 12, green);
    // F middle bar
    cpu_fb_fill_rect(fb_base, pitch, center_x - 40, center_y - 8, 45, 12, green);
}
