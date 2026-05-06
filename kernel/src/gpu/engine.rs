//! GPU Engine — Command-Level Entry Point
//!
//! Provides the `cmd_gpucmd` shell handler that demonstrates:
//! 1. FIFO channel allocation + pushbuffer setup
//! 2. Command building (NOP, CE copy, 2D fill)
//! 3. CPU-side framebuffer drawing (immediate visible proof)
//! 4. Pushbuffer hex dump for inspection

use super::commands;
use super::dma;
use super::fifo;
use crate::console::Console;
use crate::fb::colors;

/// Shell command handler: `gpucmd`
/// Demonstrates the full Level 2 command pipeline.
pub fn cmd_gpucmd(con: &mut Console, fb_base: u64, fb_pitch: u32, fb_w: u32, fb_h: u32) {
    con.println("=== GPU Command-Level Engine ===");
    con.println("  Level 2: FIFO + Pushbuffer + Commands");
    con.println("");

    // ── Step 1: Allocate FIFO channel ────────────────────────────────────
    con.print("  [1] Allocating FIFO channel... ");
    let ch = fifo::create_channel(0);
    if ch.is_none() {
        con.print_colored("FAIL", colors::ACCENT_RED);
        con.println(" (out of DMA memory)");
        return;
    }
    let mut ch = ch.unwrap();
    con.print_colored("OK", colors::NV_GREEN);
    con.println("");

    // Print channel info
    con.print("      Pushbuf: 0x");
    print_hex32(con, (ch.pushbuf.phys >> 32) as u32);
    print_hex32(con, ch.pushbuf.phys as u32);
    con.print(" (");
    print_dec(con, ch.pushbuf.size as u32);
    con.println(" bytes)");

    con.print("      GPFIFO:  0x");
    print_hex32(con, (ch.gpfifo.phys >> 32) as u32);
    print_hex32(con, ch.gpfifo.phys as u32);
    con.print(" (");
    print_dec(con, ch.gpfifo.size as u32);
    con.println(" bytes)");

    con.print("      DMA remaining: ");
    print_dec(con, dma::gpu_dma_remaining() as u32);
    con.println(" bytes");
    con.println("");

    // ── Step 2: Build NOP commands ───────────────────────────────────────
    con.print("  [2] Building NOP + object binds... ");
    commands::cmd_nop(&mut ch);
    commands::cmd_nop(&mut ch);
    commands::cmd_nop(&mut ch);
    commands::cmd_bind_2d(&mut ch);
    commands::cmd_bind_ce(&mut ch);
    con.print_colored("OK", colors::NV_GREEN);
    con.print(" (");
    print_dec(con, ch.cmd_count);
    con.println(" cmds)");

    // ── Step 3: Build CE copy command ────────────────────────────────────
    con.print("  [3] Building CE DMA copy cmd... ");
    let src = 0x0060_0000u64; // 6MB (test source)
    let dst = 0x0060_1000u64; // 6MB+4K (test dest)
    commands::cmd_ce_copy(&mut ch, src, dst, 256);
    con.print_colored("OK", colors::NV_GREEN);
    con.print(" (");
    print_dec(con, ch.cmd_count);
    con.println(" cmds)");

    // ── Step 4: Build 2D fill command ────────────────────────────────────
    con.print("  [4] Building 2D fill cmd... ");
    commands::cmd_2d_fill(
        &mut ch, fb_base, fb_pitch, fb_w, fb_h, 100, 100, 200, 150, 0xFF76B900,
    );
    con.print_colored("OK", colors::NV_GREEN);
    con.print(" (");
    print_dec(con, ch.cmd_count);
    con.println(" cmds)");

    // ── Step 5: Submit to GPFIFO ─────────────────────────────────────────
    con.print("  [5] Submitting to GPFIFO... ");
    ch.submit_gpfifo(0, ch.put);
    con.print_colored("OK", colors::NV_GREEN);
    con.print(" (GP_PUT=");
    print_dec(con, ch.gp_put);
    con.println(")");
    con.println("");

    // ── Step 6: Pushbuffer hex dump ──────────────────────────────────────
    con.println("  [6] Pushbuffer dump (first 32 dwords):");
    let words = ch.dump_pushbuf(32);
    for (i, &w) in words.iter().enumerate() {
        if i % 8 == 0 {
            con.print("      ");
            print_hex16(con, (i * 4) as u16);
            con.print(": ");
        }
        print_hex32(con, w);
        con.print(" ");
        if i % 8 == 7 {
            con.println("");
        }
    }
    if words.len() % 8 != 0 {
        con.println("");
    }
    con.println("");

    // ── Step 7: CPU-side framebuffer drawing (VISIBLE PROOF!) ────────────
    if fb_base != 0 {
        con.print("  [7] CPU framebuffer draw... ");
        // Draw NVIDIA green border
        commands::cpu_fb_fill_rect_clipped(
            fb_base, fb_pitch, fb_w, fb_h, 0, 0, fb_w, 4, 0xFF76B900,
        );
        commands::cpu_fb_fill_rect_clipped(
            fb_base,
            fb_pitch,
            fb_w,
            fb_h,
            0,
            fb_h.saturating_sub(4),
            fb_w,
            4,
            0xFF76B900,
        );
        commands::cpu_fb_fill_rect_clipped(
            fb_base, fb_pitch, fb_w, fb_h, 0, 0, 4, fb_h, 0xFF76B900,
        );
        commands::cpu_fb_fill_rect_clipped(
            fb_base,
            fb_pitch,
            fb_w,
            fb_h,
            fb_w.saturating_sub(4),
            0,
            4,
            fb_h,
            0xFF76B900,
        );

        // Draw gradient box (proof of pixel-level FB access)
        commands::cpu_fb_gradient(fb_base, fb_pitch, fb_w, fb_h, 50, 50, 200, 150);

        // Draw FastOS "F" logo
        commands::cpu_fb_logo(fb_base, fb_pitch, fb_w / 2, fb_h / 2);

        con.print_colored("DRAWN!", colors::NV_GREEN);
        con.println("");
    } else {
        con.println("  [7] No framebuffer (VGA text mode) — skipping draw");
    }
    con.println("");

    // ── Summary ──────────────────────────────────────────────────────────
    con.print("  ");
    con.print_colored("LEVEL 2 READY", colors::NV_GREEN);
    con.println(" — Command pipeline operational");
    con.print("  Total: ");
    print_dec(con, ch.cmd_count);
    con.print(" GPU commands, ");
    print_dec(con, ch.put);
    con.print(" pushbuf words, ");
    print_dec(con, ch.gp_put);
    con.println(" GPFIFO entries");
    con.println("");
    con.println("  NOTE: GPU-side execution requires GSP-RM FIFO scheduling.");
    con.println("  CPU framebuffer writes work immediately (Step 7).");
    con.println("  Pushbuffer commands are ready for when FIFO is live.");
}

// ── Helper print functions ───────────────────────────────────────────────────

/// Shell command handler: `gpu2d`
/// Builds a larger 2D command stream and draws a matching CPU fallback.
pub fn cmd_gpu2d(con: &mut Console, fb_base: u64, fb_pitch: u32, fb_w: u32, fb_h: u32) {
    con.println("=== GPU 2D Workload ===");
    con.println("  Target: Ampere 2D + CE pushbuffer");
    con.println("");

    if fb_base == 0 || fb_pitch == 0 || fb_w == 0 || fb_h == 0 {
        con.println("  No linear framebuffer available.");
        return;
    }

    con.print("  [1] Allocating FIFO channel... ");
    let ch = fifo::create_channel(2);
    if ch.is_none() {
        con.print_colored("FAIL", colors::ACCENT_RED);
        con.println(" (out of DMA memory)");
        return;
    }
    let mut ch = ch.unwrap();
    con.print_colored("OK", colors::NV_GREEN);
    con.println("");

    con.print("  [2] Binding 2D/CE objects... ");
    commands::cmd_nop(&mut ch);
    commands::cmd_bind_2d(&mut ch);
    commands::cmd_bind_ce(&mut ch);
    con.print_colored("OK", colors::NV_GREEN);
    con.println("");

    con.print("  [3] Queuing 2D rectangles... ");
    let top_h = core::cmp::min(72, core::cmp::max(24, fb_h / 10));
    let rail_w = core::cmp::min(96, core::cmp::max(24, fb_w / 18));
    let colors = [
        0xFF76B900, 0xFF00D1FF, 0xFFFFD166, 0xFFFF4D6D, 0xFFB8F7D4, 0xFF8E7DFF, 0xFFFF9F1C,
        0xFF2EC4B6,
    ];

    queue_2d_fill(
        &mut ch, fb_base, fb_pitch, fb_w, fb_h, 0, 0, fb_w, fb_h, 0xFF101820,
    );
    queue_2d_fill(
        &mut ch, fb_base, fb_pitch, fb_w, fb_h, 0, 0, fb_w, top_h, 0xFF16213E,
    );
    queue_2d_fill(
        &mut ch, fb_base, fb_pitch, fb_w, fb_h, 0, 0, rail_w, fb_h, 0xFF0B132B,
    );
    queue_2d_fill(
        &mut ch,
        fb_base,
        fb_pitch,
        fb_w,
        fb_h,
        fb_w.saturating_sub(rail_w),
        0,
        rail_w,
        fb_h,
        0xFF0B132B,
    );

    let work_x = rail_w.saturating_add(16);
    let work_y = top_h.saturating_add(24);
    let work_w = fb_w.saturating_sub((rail_w.saturating_add(16)).saturating_mul(2));
    let work_h = fb_h.saturating_sub(work_y.saturating_add(32));
    let bar_w = core::cmp::max(4, work_w / 16);

    let mut i = 0u32;
    while i < 16 {
        let x = work_x.saturating_add(i.saturating_mul(bar_w));
        let h = core::cmp::max(12, work_h / 4 + (i % 5) * core::cmp::max(6, work_h / 18));
        let y = work_y.saturating_add(work_h.saturating_sub(h));
        queue_2d_fill(
            &mut ch,
            fb_base,
            fb_pitch,
            fb_w,
            fb_h,
            x,
            y,
            bar_w.saturating_sub(2),
            h,
            colors[(i as usize) & 7],
        );
        i += 1;
    }

    queue_2d_fill(
        &mut ch, fb_base, fb_pitch, fb_w, fb_h, work_x, work_y, work_w, 3, 0xFF76B900,
    );
    queue_2d_fill(
        &mut ch, fb_base, fb_pitch, fb_w, fb_h, work_x, work_y, 3, work_h, 0xFF00D1FF,
    );
    queue_2d_fill(
        &mut ch,
        fb_base,
        fb_pitch,
        fb_w,
        fb_h,
        work_x,
        work_y.saturating_add(work_h.saturating_sub(3)),
        work_w,
        3,
        0xFFFFD166,
    );
    queue_2d_fill(
        &mut ch,
        fb_base,
        fb_pitch,
        fb_w,
        fb_h,
        work_x.saturating_add(work_w.saturating_sub(3)),
        work_y,
        3,
        work_h,
        0xFFFF4D6D,
    );

    con.print_colored("OK", colors::NV_GREEN);
    con.print(" (");
    print_dec(con, ch.cmd_count);
    con.println(" cmds)");

    con.print("  [4] Queue CE copy + semaphore... ");
    commands::cmd_ce_copy(&mut ch, 0x0060_2000, 0x0060_3000, 512);
    let sem_phys = ch.gpfifo.phys;
    commands::cmd_semaphore_release(&mut ch, sem_phys, 0x2D2D_0001);
    con.print_colored("OK", colors::NV_GREEN);
    con.println("");

    con.print("  [5] Submit software GPFIFO entry... ");
    ch.submit_gpfifo(0, ch.put);
    con.print_colored("OK", colors::NV_GREEN);
    con.print(" (words=");
    print_dec(con, ch.put);
    con.print(", gp_entries=");
    print_dec(con, ch.gp_put);
    con.println(")");

    con.println("  [6] Pushbuffer dump (first 24 dwords):");
    let words = ch.dump_pushbuf(24);
    for (idx, &word) in words.iter().enumerate() {
        if idx % 6 == 0 {
            con.print("      ");
            print_hex16(con, (idx * 4) as u16);
            con.print(": ");
        }
        print_hex32(con, word);
        con.print(" ");
        if idx % 6 == 5 {
            con.println("");
        }
    }
    if words.len() % 6 != 0 {
        con.println("");
    }

    con.print("  [7] CPU fallback mirror draw... ");
    draw_2d_fallback(fb_base, fb_pitch, fb_w, fb_h);
    con.print_colored("DRAWN", colors::NV_GREEN);
    con.println("");
    con.println("");
    con.println("  GPU stream prepared: 2D fills + CE copy + semaphore.");
    con.println("  Hardware execution still needs GSP-RM channel scheduling and a fence.");
    con.println("  The visible picture is CPU fallback until that fence changes.");
}

fn queue_2d_fill(
    ch: &mut fifo::GpuChannel,
    fb_base: u64,
    fb_pitch: u32,
    fb_w: u32,
    fb_h: u32,
    x: u32,
    y: u32,
    w: u32,
    h: u32,
    color: u32,
) {
    if w == 0 || h == 0 || x >= fb_w || y >= fb_h {
        return;
    }
    let x1 = core::cmp::min(x.saturating_add(w), fb_w);
    let y1 = core::cmp::min(y.saturating_add(h), fb_h);
    if x1 > x && y1 > y {
        commands::cmd_2d_fill(
            ch,
            fb_base,
            fb_pitch,
            fb_w,
            fb_h,
            x,
            y,
            x1 - x,
            y1 - y,
            color,
        );
    }
}

fn draw_2d_fallback(fb_base: u64, fb_pitch: u32, fb_w: u32, fb_h: u32) {
    let top_h = core::cmp::min(72, core::cmp::max(24, fb_h / 10));
    let rail_w = core::cmp::min(96, core::cmp::max(24, fb_w / 18));
    let colors = [
        0xFF76B900, 0xFF00D1FF, 0xFFFFD166, 0xFFFF4D6D, 0xFFB8F7D4, 0xFF8E7DFF, 0xFFFF9F1C,
        0xFF2EC4B6,
    ];

    commands::cpu_fb_fill_rect_clipped(fb_base, fb_pitch, fb_w, fb_h, 0, 0, fb_w, fb_h, 0xFF101820);
    commands::cpu_fb_fill_rect_clipped(
        fb_base, fb_pitch, fb_w, fb_h, 0, 0, fb_w, top_h, 0xFF16213E,
    );
    commands::cpu_fb_fill_rect_clipped(
        fb_base, fb_pitch, fb_w, fb_h, 0, 0, rail_w, fb_h, 0xFF0B132B,
    );
    commands::cpu_fb_fill_rect_clipped(
        fb_base,
        fb_pitch,
        fb_w,
        fb_h,
        fb_w.saturating_sub(rail_w),
        0,
        rail_w,
        fb_h,
        0xFF0B132B,
    );

    let work_x = rail_w.saturating_add(16);
    let work_y = top_h.saturating_add(24);
    let work_w = fb_w.saturating_sub((rail_w.saturating_add(16)).saturating_mul(2));
    let work_h = fb_h.saturating_sub(work_y.saturating_add(32));
    let bar_w = core::cmp::max(4, work_w / 16);

    let mut i = 0u32;
    while i < 16 {
        let x = work_x.saturating_add(i.saturating_mul(bar_w));
        let h = core::cmp::max(12, work_h / 4 + (i % 5) * core::cmp::max(6, work_h / 18));
        let y = work_y.saturating_add(work_h.saturating_sub(h));
        commands::cpu_fb_fill_rect_clipped(
            fb_base,
            fb_pitch,
            fb_w,
            fb_h,
            x,
            y,
            bar_w.saturating_sub(2),
            h,
            colors[(i as usize) & 7],
        );
        i += 1;
    }

    commands::cpu_fb_fill_rect_clipped(
        fb_base, fb_pitch, fb_w, fb_h, work_x, work_y, work_w, 3, 0xFF76B900,
    );
    commands::cpu_fb_fill_rect_clipped(
        fb_base, fb_pitch, fb_w, fb_h, work_x, work_y, 3, work_h, 0xFF00D1FF,
    );
    commands::cpu_fb_fill_rect_clipped(
        fb_base,
        fb_pitch,
        fb_w,
        fb_h,
        work_x,
        work_y.saturating_add(work_h.saturating_sub(3)),
        work_w,
        3,
        0xFFFFD166,
    );
    commands::cpu_fb_fill_rect_clipped(
        fb_base,
        fb_pitch,
        fb_w,
        fb_h,
        work_x.saturating_add(work_w.saturating_sub(3)),
        work_y,
        3,
        work_h,
        0xFFFF4D6D,
    );
}

fn print_hex32(con: &mut Console, val: u32) {
    let digits = b"0123456789ABCDEF";
    for i in (0..8).rev() {
        let nibble = ((val >> (i * 4)) & 0xF) as usize;
        con.put_char(digits[nibble]);
    }
}

fn print_hex16(con: &mut Console, val: u16) {
    let digits = b"0123456789ABCDEF";
    for i in (0..4).rev() {
        let nibble = ((val >> (i * 4)) & 0xF) as usize;
        con.put_char(digits[nibble]);
    }
}

fn print_dec(con: &mut Console, mut val: u32) {
    if val == 0 {
        con.put_char(b'0');
        return;
    }
    let mut buf = [0u8; 10];
    let mut i = 0;
    while val > 0 {
        buf[i] = b'0' + (val % 10) as u8;
        val /= 10;
        i += 1;
    }
    for j in (0..i).rev() {
        con.put_char(buf[j]);
    }
}
