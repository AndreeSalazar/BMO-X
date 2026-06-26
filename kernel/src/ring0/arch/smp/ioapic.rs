//! I/O APIC driver — routes external IRQs to specific CPU cores.
//!
//! The I/O APIC is a separate device from the Local APIC. Each I/O APIC
//! has a set of redirection table entries (RTEs) that map IRQ lines to
//! interrupt vectors and target CPUs.
//!
//! Standard x86 I/O APIC register layout (MMIO):
//!   Register 0x00: ID
//!   Register 0x01: Version
//!   Register 0x10-0x3F: Redirection table entries (2 entries per 32-bit reg)
//!
//! RTE format (64-bit per entry):
//!   Bits [0-7]:   Vector (interrupt number)
//!   Bit  [8]:     Delivery Mode (0=fixed, 1=lowest priority, 2=SMI, 4=NMI, 7=INIT)
//!   Bit  [11]:    Delivery Status (0=Idle, 1=Pending)
//!   Bit  [12]:    Interrupt Polarity (0=active high, 1=active low)
//!   Bit  [13]:    Remote IRR (read-only)
//!   Bit  [14]:    Trigger Mode (0=edge, 1=level)
//!   Bit  [15]:    Interrupt Mask (0=unmasked, 1=masked)
//!   Bits [16-17]: Destination Field — Physical (0) or Logical (1)
//!   Bits [56-63]: Destination APIC ID (physical mode)

use core::arch::asm;

/// I/O APIC register offsets.
const IOREGSEL: u32 = 0x00;  // I/O Register Select (32-bit)
const IOWIN: u32 = 0x10;     // I/O Window (32-bit, data port)

/// I/O APIC register indices.
const IOAPICID: u32 = 0x00;
const IOAPICVER: u32 = 0x01;
const IOREDTBL_BASE: u32 = 0x10;

/// I/O APIC base address (MMIO-mapped).
static mut IOAPIC_BASE: u64 = 0;

/// Number of redirection table entries.
static mut IOAPIC_MAX_REDIRECT: u32 = 0;

/// I/O APIC ID.
static mut IOAPIC_ID: u32 = 0;

/// Write to an I/O APIC register.
unsafe fn ioapic_write(reg: u32, val: u32) {
    let base = IOAPIC_BASE as *mut u32;
    core::ptr::write_volatile(base.add(IOREGSEL as usize / 4), reg);
    core::ptr::write_volatile(base.add(IOWIN as usize / 4), val);
}

/// Read from an I/O APIC register.
unsafe fn ioapic_read(reg: u32) -> u32 {
    let base = IOAPIC_BASE as *mut u32;
    core::ptr::write_volatile(base.add(IOREGSEL as usize / 4), reg);
    core::ptr::read_volatile(base.add(IOWIN as usize / 4) as *const u32)
}

/// Read the I/O APIC version register.
/// Returns (max_redirect_entries, version).
pub fn version() -> (u32, u32) {
    unsafe {
        let val = ioapic_read(IOAPICVER);
        ((val >> 16) & 0xFF, val & 0xFF)
    }
}

/// Initialize the I/O APIC at the given MMIO base address.
///
/// This is typically called during boot after ACPI parsing (MADT)
/// or after probing the I/O APIC at the standard address.
pub unsafe fn init_ioapic(base: u64) {
    IOAPIC_BASE = base;

    crate::dev::console::serial_write("[ioapic] base=0x");
    crate::boot::serial::hex(base);

    let (max_redir, ver) = version();
    IOAPIC_MAX_REDIRECT = max_redir + 1;

    // Read current I/O APIC ID
    let id = ioapic_read(IOAPICID) >> 24;
    IOAPIC_ID = id;

    crate::dev::console::serial_write(" ver=");
    crate::boot::serial::u64_dec(ver as u64);
    crate::dev::console::serial_write(" max_irq=");
    crate::boot::serial::u64_dec(max_redir as u64);
    crate::dev::console::serial_write(" id=");
    crate::boot::serial::u64_dec(id as u64);
    crate::dev::console::serial_write("\n");

    // Mask all redirection entries (bit 15 = mask)
    for i in 0..IOAPIC_MAX_REDIRECT {
        let reg = IOREDTBL_BASE + i * 2;
        ioapic_write(reg + 1, 1 << 15); // mask high dword
    }

    crate::dev::console::serial_write("[ioapic] all IRQs masked\n");
}

/// Set a redirection table entry. Maps an IRQ to a specific vector
/// and target CPU.
///
/// `irq`: IRQ number (0-based index into the redirection table).
/// `vector`: Interrupt vector (0-255).
/// `target_apic_id`: Target CPU's APIC ID (physical mode).
/// `level_triggered`: true for level-triggered, false for edge-triggered.
/// `active_low`: true for active-low, false for active-high.
/// `masked`: true to mask (disable) this IRQ.
pub unsafe fn set_redirect(
    irq: u32,
    vector: u8,
    target_apic_id: u32,
    level_triggered: bool,
    active_low: bool,
    masked: bool,
) {
    if irq >= IOAPIC_MAX_REDIRECT {
        return;
    }

    let reg = IOREDTBL_BASE + irq * 2;

    // Low dword: vector + flags
    let mut lo = vector as u32;
    // Delivery mode = 0 (fixed)
    // Bit 12: interrupt polarity
    if active_low { lo |= 1 << 13; }
    // Bit 14: trigger mode
    if level_triggered { lo |= 1 << 14; }
    // Bit 15: mask
    if masked { lo |= 1 << 15; }

    // High dword: destination APIC ID (physical mode, bits 56-63)
    let hi = (target_apic_id & 0xFF) << 24;

    // Disable interrupts while programming
    let was_enabled = crate::cpu::irqs_enabled();
    crate::cpu::cli();

    // Write high dword first (while low dword is masked)
    ioapic_write(reg + 1, hi | (1 << 15)); // mask during setup
    ioapic_write(reg, lo);
    // Now write the actual high dword (unmasked if needed)
    if !masked {
        ioapic_write(reg + 1, hi);
    }

    if was_enabled {
        asm!("sti", options(nostack));
    }
}

/// Mask an IRQ (disable it).
pub unsafe fn mask_irq(irq: u32) {
    if irq >= IOAPIC_MAX_REDIRECT { return; }
    let reg = IOREDTBL_BASE + irq * 2;
    let hi = ioapic_read(reg + 1);
    ioapic_write(reg + 1, hi | (1 << 15));
}

/// Unmask an IRQ (enable it).
pub unsafe fn unmask_irq(irq: u32) {
    if irq >= IOAPIC_MAX_REDIRECT { return; }
    let reg = IOREDTBL_BASE + irq * 2;
    let hi = ioapic_read(reg + 1);
    ioapic_write(reg + 1, hi & !(1 << 15));
}

/// Send EOI to the I/O APIC (required for level-triggered IRQs).
/// For edge-triggered IRQs, the Local APIC EOI is sufficient.
pub unsafe fn eoi() {
    // I/O APIC doesn't have a direct EOI register.
    // Level-triggered IRQs need the EOI sent to the Local APIC.
    // The caller should call super::super::apic::apic_eoi() instead.
    // This function is a no-op for the I/O APIC.
}

/// Get the number of redirection table entries.
pub fn max_redirect_entries() -> u32 {
    unsafe { IOAPIC_MAX_REDIRECT }
}

/// Get the I/O APIC base address.
pub fn base() -> u64 {
    unsafe { IOAPIC_BASE }
}

/// Probe for I/O APIC at the standard addresses.
/// Returns the base address if found, None otherwise.
///
/// Standard addresses:
///   - 0xFEC00000 (default, from MPC table)
///   - From ACPI MADT (preferred)
pub unsafe fn probe() -> Option<u64> {
    const CANDIDATES: [u64; 2] = [0xFEC0_0000, 0xFEC0_1000];

    for &base in &CANDIDATES {
        let ptr = base as *const u32;
        // Try reading the ID register
        let val = core::ptr::read_volatile(ptr);
        if val != 0xFFFFFFFF && val != 0 {
            return Some(base);
        }
    }
    None
}
