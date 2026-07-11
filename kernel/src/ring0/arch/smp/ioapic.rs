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
    crate::serial::hex(base);

    let (max_redir, ver) = version();
    IOAPIC_MAX_REDIRECT = max_redir + 1;

    // Read current I/O APIC ID
    let id = ioapic_read(IOAPICID) >> 24;
    IOAPIC_ID = id;

    crate::dev::console::serial_write(" ver=");
    crate::serial::u64_dec(ver as u64);
    crate::dev::console::serial_write(" max_irq=");
    crate::serial::u64_dec(max_redir as u64);
    crate::dev::console::serial_write(" id=");
    crate::serial::u64_dec(id as u64);
    crate::dev::console::serial_write("\n");

    // Mask all redirection entries (bit 15 = mask)
    for i in 0..IOAPIC_MAX_REDIRECT {
        let reg = IOREDTBL_BASE + i * 2;
        ioapic_write(reg + 1, 1 << 15); // mask high dword
    }

    crate::dev::console::serial_write("[ioapic] all IRQs masked\n");
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
