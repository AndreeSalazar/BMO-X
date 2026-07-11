//! HPET Driver (Ring 0 HAL).
//!
//! High Precision Event Timer — preferred hardware timer for modern PCs.
//! Provides a 64-bit counter running at 10+ MHz with compare registers.
//!
//! HPET registers (memory-mapped, from ACPI HPET table):
//!   - ID: Revision, number of comparators, 64-bit mode
//!   - PERIOD: Counter tick period in femtoseconds
//!   - CFG: Main counter enable, legacy replacement
//!   - ISR: Interrupt Status Register
//!   - COUNTER: Main counter value (64-bit)
//!
//! Comparator registers (per-comparator, 0x10 stride):
//!   - VAL: Comparator value
//!   - CFG: Comparator config (mode, interrupt routing)
//!   - FSB: FSB interrupt routing (for MSI)

/// HPET register offsets.
const HPET_ID: usize = 0x00;
const HPET_PERIOD: usize = 0x04;
const HPET_CFG: usize = 0x10;
#[allow(dead_code)]
const HPET_ISR: usize = 0x20;
const HPET_COUNTER: usize = 0x0F0;

/// HPET CFG bits.
const HPET_CFG_ENABLE: u32 = 1 << 0;   // Main counter enable
const HPET_CFG_LEGACY: u32 = 1 << 1;   // Legacy replacement mode

/// HPET state.
#[derive(Debug)]
pub struct HpetState {
    pub mmio_base: u64,
    pub period_fs: u64,       // Counter period in femtoseconds
    pub frequency_hz: u64,    // Calculated frequency (10^15 / period_fs)
    pub comparator_count: u8, // Number of comparators
    pub is_64bit: bool,       // 64-bit counter mode
    pub enabled: bool,
}

static mut HPET: Option<HpetState> = None;

/// Read a 32-bit HPET register.
unsafe fn hpet_read(mmio: u64, offset: usize) -> u32 {
    let ptr = (mmio + offset as u64) as *const u32;
    core::ptr::read_volatile(ptr)
}

/// Write a 32-bit HPET register.
unsafe fn hpet_write(mmio: u64, offset: usize, val: u32) {
    let ptr = (mmio + offset as u64) as *mut u32;
    core::ptr::write_volatile(ptr, val);
}

/// Initialize HPET from MMIO base (detected from ACPI HPET table).
pub fn init() {
    // HPET MMIO base must be set externally via set_mmio_base()
    // after ACPI HPET table is parsed
    let mmio_base = get_mmio_base();
    if mmio_base == 0 {
        crate::dev::console::serial_write("[hpet] no MMIO base configured\n");
        return;
    }
    init_at(mmio_base);
}

/// Set the HPET MMIO base address (called from ACPI parsing).
static mut MMIO_BASE: u64 = 0;

pub fn set_mmio_base(base: u64) {
    unsafe { MMIO_BASE = base; }
}

pub fn get_mmio_base() -> u64 {
    unsafe { MMIO_BASE }
}

/// Initialize HPET at a specific MMIO base address.
pub fn init_at(mmio_base: u64) {
    unsafe {
        let id_reg = hpet_read(mmio_base, HPET_ID);
        let period = hpet_read(mmio_base, HPET_PERIOD) as u64;

        let comparator_count = ((id_reg >> 8) & 0x1F) as u8;
        let is_64bit = (id_reg >> 13) & 1 != 0;
        let frequency_hz = 1_000_000_000_000_000u64 / period;

        let mut state = HpetState {
            mmio_base,
            period_fs: period,
            frequency_hz,
            comparator_count,
            is_64bit,
            enabled: false,
        };

        // Enable main counter + legacy replacement
        let cfg = HPET_CFG_ENABLE | HPET_CFG_LEGACY;
        hpet_write(mmio_base, HPET_CFG, cfg);
        state.enabled = true;

        HPET = Some(state);

        crate::dev::console::serial_write("[hpet] initialized: freq=");
        crate::dev::console::serial_write_u64(frequency_hz / 1000, 10);
        crate::dev::console::serial_write(" kHz, comparators=");
        crate::dev::console::serial_write_u64(comparator_count as u64, 10);
        crate::dev::console::serial_write("\n");
    }
}

/// Check if HPET is available and initialized.
pub fn is_available() -> bool {
    unsafe { HPET.is_some() }
}

/// Get the current HPET counter value in nanoseconds.
pub fn now_ns() -> u64 {
    unsafe {
        match HPET {
            Some(ref hpet) => {
                let counter = hpet_read(hpet.mmio_base, HPET_COUNTER) as u64;
                counter * hpet.period_fs / 1_000_000 // femtoseconds → nanoseconds
            }
            None => 0,
        }
    }
}

/// Get HPET frequency in Hz.
pub fn frequency_hz() -> u64 {
    unsafe {
        HPET.as_ref().map(|h| h.frequency_hz).unwrap_or(0)
    }
}

/// Read the raw 64-bit counter value.
pub fn counter() -> u64 {
    unsafe {
        match HPET {
            Some(ref hpet) => hpet_read(hpet.mmio_base, HPET_COUNTER) as u64,
            None => 0,
        }
    }
}
