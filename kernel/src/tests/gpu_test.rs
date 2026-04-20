//! GPU Hardware Communication Tests — Live Register Probes
//!
//! These tests read REAL registers from the RTX 3060 12G (GA106) via BAR0 MMIO.
//! They prove that FastOS has actual bidirectional communication with the GPU.
//!
//! Each test returns (pass: bool, detail: &str, raw_value: u32).
//! Run all via shell command `gputest`.

use crate::console::Console;
use crate::platform::FastOsPlatform;
use nv_hal::Platform; // Trait must be in scope for method calls

/// Individual test result.
pub struct TestResult {
    pub name: &'static str,
    pub passed: bool,
    pub detail: [u8; 64],
    pub detail_len: usize,
    pub raw: u32,
}

impl TestResult {
    fn pass(name: &'static str, raw: u32) -> Self {
        Self { name, passed: true, detail: [0; 64], detail_len: 0, raw }
    }
    fn fail(name: &'static str, raw: u32) -> Self {
        Self { name, passed: false, detail: [0; 64], detail_len: 0, raw }
    }
}

/// Run all GPU hardware tests. Returns (passed, total).
pub fn run_all_tests(con: &mut Console, boot_info: *const fastos_boot_protocol::BootInfo) {
    use nv_hal::{self, MmioRegion};
    use nv_regs::*;

    let platform = FastOsPlatform::new();

    con.println("=== GPU Hardware Communication Tests ===");
    con.println("  Probing live RTX 3060 12G registers...");
    con.println("");

    let mut passed = 0u32;
    let mut total = 0u32;

    // ── Test 0: PCI Discovery ────────────────────────────────────────────
    total += 1;
    let gpu_pci = nv_hal::find_gpu(&platform);
    if let Some(pci) = gpu_pci {
        print_result(con, "T00 PCI Discovery", true,
            "NVIDIA 10DE:2504 found", 0);
        passed += 1;
    } else {
        print_result(con, "T00 PCI Discovery", false,
            "GPU not found on PCI bus", 0);
        con.println("");
        print_summary(con, passed, total);
        return; // Can't continue without PCI
    }
    let pci = gpu_pci.unwrap();

    // ── Test 1: BAR0 Address Valid ───────────────────────────────────────
    total += 1;
    let bar0_phys = nv_hal::read_bar0(&platform, pci);
    let bar0_valid = bar0_phys != 0 && bar0_phys != 0xFFFF_FFFF_FFFF_FFF0
        && bar0_phys < 0x1_0000_0000; // Must be within 4GB (identity mapped)
    if bar0_valid {
        print_result_hex(con, "T01 BAR0 Address", true, bar0_phys as u32);
        passed += 1;
    } else {
        print_result_hex(con, "T01 BAR0 Address", false, bar0_phys as u32);
    }

    // Map BAR0 for all subsequent tests
    let bar0_ptr = platform.map_mmio(bar0_phys, BAR0_SIZE);
    if bar0_ptr.is_null() {
        con.print_colored("  FATAL: Cannot map BAR0\n", 0xFFFF0000);
        print_summary(con, passed, total);
        return;
    }
    let bar0 = unsafe { MmioRegion::new(bar0_ptr, BAR0_SIZE) };

    // ── Test 2: BOOT_0 Chip ID ───────────────────────────────────────────
    // Reading BOOT_0 (offset 0x0) returns the GPU chip identifier.
    // If this works, we have PROVEN register-level communication.
    total += 1;
    let boot0 = bar0.read32(pmc::BOOT_0);
    let boot0_ok = boot0 != 0 && boot0 != 0xFFFF_FFFF;
    if boot0_ok {
        print_result_hex(con, "T02 BOOT_0 Chip ID", true, boot0);
        passed += 1;
    } else {
        print_result_hex(con, "T02 BOOT_0 Chip ID", false, boot0);
    }

    // ── Test 3: GPU Timer Liveness ───────────────────────────────────────
    // PTIMER counts nanoseconds. Read twice — if the second read is larger,
    // the GPU clock is running = GPU is ALIVE.
    total += 1;
    let time1_lo = bar0.read32(ptimer::TIME_LO);
    // Small busy-wait
    for _ in 0..100_000u32 { core::hint::spin_loop(); }
    let time2_lo = bar0.read32(ptimer::TIME_LO);
    let timer_alive = time2_lo != time1_lo;
    if timer_alive {
        print_result_hex(con, "T03 PTIMER Liveness", true, time2_lo.wrapping_sub(time1_lo));
        passed += 1;
    } else {
        print_result_hex(con, "T03 PTIMER Liveness", false, 0);
    }

    // ── Test 4: VRAM Detection ───────────────────────────────────────────
    // FB_MEM_SIZE register should return a value that decodes to ~12GB
    total += 1;
    let fb_raw = bar0.read32(pmem::FB_MEM_SIZE);
    let fb_val = (fb_raw & 0xFFF) as u64;
    let vram_mb = fb_val * 16;
    let vram_ok = vram_mb >= 8192 && vram_mb <= 16384; // 8-16 GB range for 12GB card
    if vram_ok {
        con.print("  T04 VRAM Detect      ");
        con.print_colored("[PASS]", 0xFF00FF00);
        con.print(" = ");
        con.print_u64(vram_mb);
        con.println(" MB");
        passed += 1;
    } else {
        con.print("  T04 VRAM Detect      ");
        con.print_colored("[FAIL]", 0xFFFF0000);
        con.print(" raw=");
        con.print_hex32(fb_raw);
        con.println("");
    }

    // ── Test 5: Engine Enable Mask ───────────────────────────────────────
    // PMC.ENABLE shows which GPU engines are active.
    // VBIOS should have left engines running.
    total += 1;
    let engines = bar0.read32(pmc::ENABLE);
    let has_fifo = engines & pmc::ENABLE_PFIFO != 0;
    let has_graph = engines & pmc::ENABLE_PGRAPH != 0;
    let has_disp = engines & pmc::ENABLE_PDISPLAY != 0;
    let engines_ok = has_fifo || has_graph || has_disp;
    if engines_ok {
        con.print("  T05 Engine Enable    ");
        con.print_colored("[PASS]", 0xFF00FF00);
        con.print(" FIFO=");
        con.print(if has_fifo { "Y" } else { "N" });
        con.print(" GR=");
        con.print(if has_graph { "Y" } else { "N" });
        con.print(" DISP=");
        con.println(if has_disp { "Y" } else { "N" });
        passed += 1;
    } else {
        print_result_hex(con, "T05 Engine Enable", false, engines);
    }

    // ── Test 6: PFIFO Interrupt Register ─────────────────────────────────
    // Reading PFIFO interrupt register should return 0 or a valid bitmask.
    // If 0xFFFFFFFF, the engine is dead or BAR0 is broken.
    total += 1;
    let fifo_intr = bar0.read32(pfifo::INTR_0);
    let fifo_ok = fifo_intr != 0xFFFF_FFFF;
    if fifo_ok {
        print_result_hex(con, "T06 PFIFO Intr Reg", true, fifo_intr);
        passed += 1;
    } else {
        print_result_hex(con, "T06 PFIFO Intr Reg", false, fifo_intr);
    }

    // ── Test 7: PGRAPH Status ────────────────────────────────────────────
    // PGRAPH STATUS register shows if the graphics engine is idle/busy.
    total += 1;
    let gr_status = bar0.read32(pgraph::STATUS);
    let gr_ok = gr_status != 0xFFFF_FFFF;
    if gr_ok {
        con.print("  T07 PGRAPH Status    ");
        con.print_colored("[PASS]", 0xFF00FF00);
        con.print(" ");
        con.print(if gr_status == 0 { "IDLE" } else { "BUSY" });
        con.print(" (");
        con.print_hex32(gr_status);
        con.println(")");
        passed += 1;
    } else {
        print_result_hex(con, "T07 PGRAPH Status", false, gr_status);
    }

    // ── Test 8: PBDMA0 Status ────────────────────────────────────────────
    // Push Buffer DMA engine status — feeds commands to GPU.
    total += 1;
    let pbdma0 = bar0.read32(pbdma::STATUS(0));
    let pbdma0_ok = pbdma0 != 0xFFFF_FFFF;
    if pbdma0_ok {
        print_result_hex(con, "T08 PBDMA0 Status", true, pbdma0);
        passed += 1;
    } else {
        print_result_hex(con, "T08 PBDMA0 Status", false, pbdma0);
    }

    // ── Test 9: Display Head 0 ───────────────────────────────────────────
    // Reading display head register proves display engine communication.
    total += 1;
    let head0_ctrl = bar0.read32(pdisplay::HEAD_SET_CONTROL(0));
    let head0_ok = head0_ctrl != 0xFFFF_FFFF;
    if head0_ok {
        print_result_hex(con, "T09 Display Head 0", true, head0_ctrl);
        passed += 1;
    } else {
        print_result_hex(con, "T09 Display Head 0", false, head0_ctrl);
    }

    // ── Test 10: GSP FALCON Scratch Register — Firmware Loader ─────────────
    // The GSP FALCON has scratch registers at its base.
    // Try to load GSP firmware first, then verify scratch W/R.
    total += 1;
    use crate::drivers::gsp::loader::{GspLoader, GspLoadError};
    use crate::drivers::gsp::scratch::GspScratchTest;

    // Obtener firmware desde BootInfo
    let fw_result = unsafe {
        if boot_info.is_null() {
            None
        } else {
            let bi = &*boot_info;
            if bi.gsp_addr != 0 && bi.gsp_size != 0 {
                let ptr = bi.gsp_addr as *const u8;
                let len = bi.gsp_size as usize;
                Some(core::slice::from_raw_parts(ptr, len))
            } else {
                None
            }
        }
    };

    match fw_result {
        Some(fw_blob) => {
            let loader = GspLoader::new(&bar0);
            match loader.load(fw_blob, con) {
                Ok(()) => {
                    // Firmware cargado — ahora verificar scratch
                    let scratch = GspScratchTest::new(&bar0);
                    match scratch.verify_scratch(con) {
                        Ok(()) => {
                            let val = bar0.read32(0x0011_0800);
                            con.print("  T10 GSP Scratch W/R  ");
                            con.print_colored("[PASS]", 0xFF00FF00);
                            con.print(" write=readback=0x");
                            con.print_hex32(val);
                            con.println("");
                            passed += 1;
                        }
                        Err(_) => {
                            con.print("  T10 GSP Scratch W/R  ");
                            con.print_colored("[FAIL]", 0xFFFF0000);
                            con.println(" scratch failed post-load");
                        }
                    }
                }
                Err(GspLoadError::NullFirmware) => {
                    con.print("  T10 GSP Scratch W/R  ");
                    con.print_colored("[FAIL]", 0xFFFF0000);
                    con.println(" null firmware");
                }
                Err(GspLoadError::FirmwareTooLarge) => {
                    con.print("  T10 GSP Scratch W/R  ");
                    con.print_colored("[FAIL]", 0xFFFF0000);
                    con.println(" firmware too large");
                }
                Err(GspLoadError::DmaTimeout) => {
                    con.print("  T10 GSP Scratch W/R  ");
                    con.print_colored("[FAIL]", 0xFFFF0000);
                    con.println(" DMA timeout");
                }
                Err(GspLoadError::FalconBootTimeout) => {
                    con.print("  T10 GSP Scratch W/R  ");
                    con.print_colored("[FAIL]", 0xFFFF0000);
                    con.println(" Falcon boot timeout");
                }
                Err(GspLoadError::HandshakeTimeout) => {
                    con.print("  T10 GSP Scratch W/R  ");
                    con.print_colored("[FAIL]", 0xFFFF0000);
                    con.println(" GSP handshake timeout");
                }
            }
        }
        None => {
            // Sin firmware — diagnóstico directo de registros
            let pmc_en  = bar0.read32(0x0000_0200);
            let pmc_en2 = bar0.read32(0x0000_0204);
            con.print("  NO FW: PMC_ENABLE=0x");
            con.print_hex32(pmc_en);
            con.print(" PMC_ENABLE_2=0x");
            con.print_hex32(pmc_en2);
            con.println("");
            con.print("  T10 GSP Scratch W/R  ");
            con.print_colored("[FAIL]", 0xFFFF0000);
            con.println(" no firmware blob in BootInfo");
        }
    }

    // ── Test 11: PMC Interrupt Status ────────────────────────────────────
    // Top-level interrupt pending register — should be readable.
    total += 1;
    let pmc_intr = bar0.read32(pmc::INTR_0);
    let pmc_ok = pmc_intr != 0xFFFF_FFFF;
    if pmc_ok {
        print_result_hex(con, "T11 PMC Intr Status", true, pmc_intr);
        passed += 1;
    } else {
        print_result_hex(con, "T11 PMC Intr Status", false, pmc_intr);
    }

    // ── Test 12: GPU Timer Precision ─────────────────────────────────────
    // Read PTIMER high+low for full 64-bit nanosecond timestamp.
    // A valid GPU should report a non-zero time that changes.
    total += 1;
    let t_hi = bar0.read32(ptimer::TIME_HI);
    let t_lo = bar0.read32(ptimer::TIME_LO);
    let gpu_ns = ((t_hi as u64) << 32) | (t_lo as u64);
    let time_ok = gpu_ns > 0;
    if time_ok {
        con.print("  T12 GPU Timestamp    ");
        con.print_colored("[PASS]", 0xFF00FF00);
        con.print(" = ");
        con.print_u64(gpu_ns / 1_000_000); // milliseconds
        con.println(" ms");
        passed += 1;
    } else {
        print_result_hex(con, "T12 GPU Timestamp", false, t_lo);
    }

    // ── Test 13: BAR1 VRAM Aperture ──────────────────────────────────────
    // BAR1 is the VRAM window. Should have a valid address.
    total += 1;
    let bar1_phys = nv_hal::read_bar1(&platform, pci);
    let bar1_ok = bar1_phys != 0 && bar1_phys != 0xFFFF_FFFF_FFFF_FFF0;
    if bar1_ok {
        print_result_hex(con, "T13 BAR1 VRAM Addr", true, bar1_phys as u32);
        passed += 1;
    } else {
        print_result_hex(con, "T13 BAR1 VRAM Addr", false, bar1_phys as u32);
    }

    // ── Test 14: Copy Engine 0 Interrupt Reg ─────────────────────────────
    // CE0 handles DMA copies. Reading its interrupt register proves access.
    total += 1;
    let ce0_intr = bar0.read32(pcopy::CE_INTR(0));
    let ce0_ok = ce0_intr != 0xFFFF_FFFF;
    if ce0_ok {
        print_result_hex(con, "T14 CE0 Intr Reg", true, ce0_intr);
        passed += 1;
    } else {
        print_result_hex(con, "T14 CE0 Intr Reg", false, ce0_intr);
    }

    con.println("");
    print_summary(con, passed, total);

    // Verdict
    con.println("");
    if passed == total {
        con.print_colored("  VERDICT: FULL GPU COMMUNICATION CONFIRMED", 0xFF00FF00);
        con.println("");
        con.println("  Your OS talks to every GPU subsystem:");
        con.println("    - PMC (power/engine control)");
        con.println("    - PTIMER (GPU clock)");
        con.println("    - PFIFO (command submission)");
        con.println("    - PGRAPH (3D/compute engine)");
        con.println("    - PBDMA (push buffer DMA)");
        con.println("    - PDISPLAY (display heads)");
        con.println("    - PCOPY/CE (copy engines)");
        con.println("    - FALCON/GSP (microcontroller)");
        con.println("    - FB/VRAM (memory controller)");
    } else if passed >= total - 2 {
        con.print_colored("  VERDICT: GPU COMMUNICATION OK (minor issues)", 0xFFFFFF00);
        con.println("");
    } else {
        con.print_colored("  VERDICT: GPU COMMUNICATION PARTIAL", 0xFFFF8800);
        con.println("");
        con.println("  Some subsystems not responding.");
    }
}

// ── Helpers ──────────────────────────────────────────────────────────────────

fn print_result(con: &mut Console, name: &str, pass: bool, detail: &str, _raw: u32) {
    con.print("  ");
    con.print(name);
    // Pad to column 23
    let pad = if name.len() < 21 { 21 - name.len() } else { 1 };
    for _ in 0..pad { con.print(" "); }
    if pass {
        con.print_colored("[PASS]", 0xFF00FF00);
    } else {
        con.print_colored("[FAIL]", 0xFFFF0000);
    }
    con.print(" ");
    con.println(detail);
}

fn print_result_hex(con: &mut Console, name: &str, pass: bool, raw: u32) {
    con.print("  ");
    con.print(name);
    let pad = if name.len() < 21 { 21 - name.len() } else { 1 };
    for _ in 0..pad { con.print(" "); }
    if pass {
        con.print_colored("[PASS]", 0xFF00FF00);
    } else {
        con.print_colored("[FAIL]", 0xFFFF0000);
    }
    con.print(" = ");
    con.print_hex32(raw);
    con.println("");
}

fn print_summary(con: &mut Console, passed: u32, total: u32) {
    con.print("  Results: ");
    con.print_u64(passed as u64);
    con.print("/");
    con.print_u64(total as u64);
    con.print(" passed");
    if passed == total {
        con.print_colored(" (ALL PASS)", 0xFF00FF00);
    }
    con.println("");
}
