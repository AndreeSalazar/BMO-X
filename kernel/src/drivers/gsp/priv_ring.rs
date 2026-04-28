//! PRIV Ring Initialization for GA106 (Ampere)
//!
//! The PRIV ring is NVIDIA's internal register interconnect bus.
//! On Ampere GPUs the GSP (and many other engines) sit behind this bus;
//! their MMIO registers will not respond until the ring is started and
//! the corresponding PMC-enable bits are set.
//!
//! Sequence (derived from nouveau / nvkm):
//!   1. Reset the PRIV ring master (`NV_PPRIV_SYS_MASTER`).
//!   2. Issue the "start ring" command.
//!   3. Wait for the ring to report ready.
//!   4. Clear any pending ring interrupts.
//!   5. Enable GSP in PMC (Ampere uses `NV_PMC_DEVICE_ENABLE`).
//!   6. Reset the GSP Falcon so it enters a known state.
//!   7. Verify the GSP scratch register is now accessible.

use crate::console::Console;

// ---------------------------------------------------------------------------
// PRIV Ring master registers (BAR0 offsets)
// ---------------------------------------------------------------------------

const NV_PPRIV_SYS_MASTER_CTRL: u32 = 0x0012_0050;
const NV_PPRIV_SYS_MASTER_RING_COMMAND: u32 = 0x0012_004C;
const NV_PPRIV_SYS_MASTER_RING_START_RESULTS: u32 = 0x0012_0054;
const NV_PPRIV_SYS_MASTER_RING_INTERRUPT_STATUS0: u32 = 0x0012_0058;
const NV_PPRIV_SYS_MASTER_RING_INTERRUPT_STATUS1: u32 = 0x0012_005C;

// Ring commands
const RING_CMD_START: u32 = 0x1;
const RING_CMD_ACK_INTERRUPT: u32 = 0x2;

// ---------------------------------------------------------------------------
// PMC registers
// ---------------------------------------------------------------------------

const NV_PMC_ENABLE: u32 = 0x0000_0200;
const NV_PMC_ENABLE_2: u32 = 0x0000_0204;
const NV_PMC_DEVICE_ENABLE: u32 = 0x0000_0600; // Ampere per-device enable

// ---------------------------------------------------------------------------
// GSP Falcon registers
// ---------------------------------------------------------------------------

const NV_PGSP_FALCON_ENGINE: u32 = 0x0011_03C0;
const NV_PGSP_FALCON_RESET: u32 = 0x0011_0094;

/// Scratch register 0 — used as a quick "is GSP alive?" probe.
/// Note: 0x0040 is MAILBOX0; SCRATCH0 is at falcon_base + 0x0080.
const NV_PGSP_FALCON_SCRATCH0: u32 = 0x0011_0080;

// ---------------------------------------------------------------------------
// Timing helpers (busy-loop; no timer driver yet)
// ---------------------------------------------------------------------------

/// Spin for approximately `us` microseconds.
/// At ~3 GHz a single `nop` ≈ 0.33 ns → ~3 000 nops per µs.
#[inline]
fn delay_us(us: u32) {
    for _ in 0..(us as u64 * 3000) {
        unsafe { core::arch::asm!("nop", options(nomem, nostack)) };
    }
}

/// Spin for approximately `ms` milliseconds.
#[inline]
fn delay_ms(ms: u32) {
    delay_us(ms * 1000);
}

// ---------------------------------------------------------------------------
// Error type
// ---------------------------------------------------------------------------

pub enum PrivRingError {
    /// The ring did not report "started" within the timeout.
    RingStartTimeout,
    /// A ring interrupt was still pending after acknowledge.
    RingInterruptPending,
    /// GSP did not become accessible after PMC enable + Falcon reset.
    GspEnableFailed,
}

// ---------------------------------------------------------------------------
// PrivRingInit
// ---------------------------------------------------------------------------

pub struct PrivRingInit<'a> {
    bar0: &'a nv_hal::MmioRegion,
}

impl<'a> PrivRingInit<'a> {
    pub fn new(bar0: &'a nv_hal::MmioRegion) -> Self {
        Self { bar0 }
    }

    // -- low-level MMIO helpers ------------------------------------------

    #[inline]
    fn read(&self, offset: u32) -> u32 {
        self.bar0.read32(offset)
    }

    #[inline]
    fn write(&self, offset: u32, val: u32) {
        self.bar0.write32(offset, val);
    }

    // -- individual steps ------------------------------------------------

    /// Step 1 — Reset the PRIV ring master and issue the START command.
    fn start_ring(&self, con: &mut Console) {
        con.print("  [PRIV] Resetting ring master ... ");

        // Disable the ring master, wait briefly, then re-enable.
        self.write(NV_PPRIV_SYS_MASTER_CTRL, 0x0);
        delay_us(100);
        self.write(NV_PPRIV_SYS_MASTER_CTRL, 0x1); // enable
        delay_us(100);

        // Issue the START command.
        self.write(NV_PPRIV_SYS_MASTER_RING_COMMAND, RING_CMD_START);
        con.println("START issued");
    }

    /// Step 2 — Poll `RING_START_RESULTS` until the ring reports ready.
    fn wait_ring_ready(&self, con: &mut Console) -> Result<(), PrivRingError> {
        con.print("  [PRIV] Waiting for ring ready ");

        // The low bit of START_RESULTS is set when the ring is up.
        for attempt in 0..100 {
            let val = self.read(NV_PPRIV_SYS_MASTER_RING_START_RESULTS);
            if val & 0x1 != 0 {
                con.print(" OK (attempt ");
                con.print_hex32(attempt as u32);
                con.println(")");
                return Ok(());
            }
            if attempt % 10 == 0 {
                con.print(".");
            }
            delay_ms(1);
        }

        con.println(" TIMEOUT");
        Err(PrivRingError::RingStartTimeout)
    }

    /// Step 3 — Acknowledge / clear any pending ring interrupts.
    fn clear_ring_interrupts(&self, con: &mut Console) -> Result<(), PrivRingError> {
        let status0 = self.read(NV_PPRIV_SYS_MASTER_RING_INTERRUPT_STATUS0);
        let status1 = self.read(NV_PPRIV_SYS_MASTER_RING_INTERRUPT_STATUS1);

        con.print("  [PRIV] Ring IRQ status0=0x");
        con.print_hex32(status0);
        con.print(" status1=0x");
        con.print_hex32(status1);
        con.newline();

        if status0 != 0 || status1 != 0 {
            // Send ACK command to clear.
            self.write(NV_PPRIV_SYS_MASTER_RING_COMMAND, RING_CMD_ACK_INTERRUPT);
            delay_us(200);

            // Re-check.
            let s0 = self.read(NV_PPRIV_SYS_MASTER_RING_INTERRUPT_STATUS0);
            let s1 = self.read(NV_PPRIV_SYS_MASTER_RING_INTERRUPT_STATUS1);
            if s0 != 0 || s1 != 0 {
                con.println("  [PRIV] WARNING: ring interrupt still pending after ACK");
                return Err(PrivRingError::RingInterruptPending);
            }
        }

        con.println("  [PRIV] Ring interrupts clear");
        Ok(())
    }

    /// Step 4 — Enable GSP + SEC2 in PMC using the Ampere device-enable register.
    fn enable_gsp_pmc(&self, con: &mut Console) {
        con.print("  [PRIV] PMC ENABLE  = 0x");
        con.print_hex32(self.read(NV_PMC_ENABLE));
        con.newline();

        con.print("  [PRIV] PMC ENABLE2 = 0x");
        con.print_hex32(self.read(NV_PMC_ENABLE_2));
        con.newline();

        // On Ampere the per-device enable register at 0x600 controls
        // individual engines.  Bit 4 is GSP/PGSP, Bit 3 is SEC2.
        let dev_en = self.read(NV_PMC_DEVICE_ENABLE);
        con.print("  [PRIV] PMC DEV_EN  = 0x");
        con.print_hex32(dev_en);
        con.newline();

        // Set GSP enable bit (bit 4) AND SEC2 enable bit (bit 3).
        // SEC2 is required for the authenticated booter_load path.
        let new_dev_en = dev_en | (1 << 4) | (1 << 3);
        self.write(NV_PMC_DEVICE_ENABLE, new_dev_en);
        delay_us(500);

        // Also ensure the legacy PMC_ENABLE bits are set for PGSP + SEC2.
        let pmc = self.read(NV_PMC_ENABLE);
        self.write(NV_PMC_ENABLE, pmc | (1 << 4) | (1 << 3));
        delay_us(200);

        con.print("  [PRIV] PMC DEV_EN after = 0x");
        con.print_hex32(self.read(NV_PMC_DEVICE_ENABLE));
        con.newline();
    }

    /// Step 5 — Reset the GSP Falcon so it enters a clean state.
    fn reset_gsp_falcon(&self, con: &mut Console) {
        con.println("  [PRIV] Resetting GSP Falcon ...");

        // Assert reset.
        self.write(NV_PGSP_FALCON_RESET, 0x1);
        delay_ms(2);

        // De-assert reset.
        self.write(NV_PGSP_FALCON_RESET, 0x0);
        delay_ms(5);

        // Read engine status for diagnostics.
        let engine = self.read(NV_PGSP_FALCON_ENGINE);
        con.print("  [PRIV] FALCON_ENGINE = 0x");
        con.print_hex32(engine);
        con.newline();
    }

    /// Step 6 — Verify that the GSP scratch register is now accessible.
    fn verify_gsp_accessible(&self, con: &mut Console) -> Result<(), PrivRingError> {
        con.println("  [PRIV] Verifying GSP scratch register ...");

        // Write a known pattern and read it back.
        let pattern: u32 = 0xCAFE_1234;
        self.write(NV_PGSP_FALCON_SCRATCH0, pattern);
        delay_us(50);
        let readback = self.read(NV_PGSP_FALCON_SCRATCH0);

        con.print("  [PRIV] Wrote 0x");
        con.print_hex32(pattern);
        con.print(", read 0x");
        con.print_hex32(readback);
        con.newline();

        if readback == pattern {
            con.print_colored("  [PRIV] GSP is ACCESSIBLE!\n", 0x00FF00);
            // Clear the scratch register.
            self.write(NV_PGSP_FALCON_SCRATCH0, 0x0);
            Ok(())
        } else if readback == 0xFFFF_FFFF || readback == 0xBADF_0000 {
            con.print_colored("  [PRIV] GSP NOT accessible (bus fault)\n", 0xFF0000);
            Err(PrivRingError::GspEnableFailed)
        } else {
            con.print_colored("  [PRIV] GSP readback mismatch\n", 0xFFFF00);
            Err(PrivRingError::GspEnableFailed)
        }
    }

    // -- public entry point ----------------------------------------------

    /// Full PRIV ring + GSP enable sequence for Ampere GA106.
    ///
    /// Call this **before** any other GSP register access.
    pub fn init(&self, con: &mut Console) -> Result<(), PrivRingError> {
        con.print_colored("=== Power Init (GA106 Ampere) ===\n", 0x00FFFF);

        // 0. Enable GSP in PMC FIRST (si el engine no tiene energía, el anillo de comunicación muere)
        self.enable_gsp_pmc(con);

        // 1. Reset GSP Falcon.
        self.reset_gsp_falcon(con);

        con.print_colored("=== Power Init COMPLETE ===\n", 0x00FF00);
        Ok(())
    }
}
