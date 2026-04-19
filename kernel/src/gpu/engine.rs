//! GPU Engine — Command-Level Entry Point
//!
//! Provides the `cmd_gpucmd` shell handler that demonstrates:
//! 1. FIFO channel allocation + pushbuffer setup
//! 2. Command building (NOP, CE copy, 2D fill)
//! 3. CPU-side framebuffer drawing (immediate visible proof)
//! 4. Pushbuffer hex dump for inspection

use crate::console::Console;
use crate::fb::colors;
use super::dma;
use super::fifo;
use super::commands;

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
    con.print("  [2] Building NOP commands... ");
    commands::cmd_nop(&mut ch);
    commands::cmd_nop(&mut ch);
    commands::cmd_nop(&mut ch);
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
    commands::cmd_2d_fill(&mut ch, fb_base, fb_pitch,
                          fb_w, fb_h, 100, 100, 200, 150, 0xFF76B900);
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
        commands::cpu_fb_fill_rect(fb_base, fb_pitch, 0, 0, fb_w, 4, 0xFF76B900);
        commands::cpu_fb_fill_rect(fb_base, fb_pitch, 0, fb_h - 4, fb_w, 4, 0xFF76B900);
        commands::cpu_fb_fill_rect(fb_base, fb_pitch, 0, 0, 4, fb_h, 0xFF76B900);
        commands::cpu_fb_fill_rect(fb_base, fb_pitch, fb_w - 4, 0, 4, fb_h, 0xFF76B900);

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
