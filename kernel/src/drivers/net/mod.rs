//! Network subsystem for FastOS.
//!
//! v1.4.0: stack ahora arranca solo vía `init()`. Antes era código
//! aislado — el driver RTL8168 nunca se llamaba.
//!
//! Modular network stack:
//!   - RTL8168: Ethernet NIC driver (L2) — vendor 0x10EC, device 0x8168
//!   - ARP:     Address Resolution Protocol (L2→L3)
//!   - IP:      Internet Protocol (L3)
//!   - ICMP:    Internet Control Message Protocol (ping)
//!   - UDP:     User Datagram Protocol (L4)
//!   - TCP:     Transmission Control Protocol (L4) — nuevo en v1.4.0
//!   - DHCP:    Dynamic Host Configuration Protocol (auto-IP)
//!   - Stack:   Unified network stack integration
//!   - Socket:  API unificada TCP/UDP — nuevo en v1.4.0

pub mod rtl8168;
pub mod arp;
pub mod ip;
pub mod icmp;
pub mod udp;
pub mod tcp;
pub mod dhcp;
pub mod stack;
pub mod socket;

/// Inicializa el subsistema de red: detecta la NIC y la deja lista.
///
/// Llamar una sola vez, **después** de Phase 2 (PCI scan + ECAM map)
/// y antes de cualquier uso del stack.
pub fn init() {
    use crate::drivers::serial::serial_write;
    serial_write("[net] init: scanning PCI for NICs\n");

    // Detectar la NIC por vendor/device ID
    unsafe {
        if let Some(driver) = rtl8168::Rtl8168Driver::detect() {
            serial_write("[net] RTL8168 detected, MAC=");
            let mac = driver.mac_address();
            for i in 0..6 {
                if i > 0 { serial_write(":"); }
                let b = mac[i];
                let hi = (b >> 4) & 0xF;
                let lo = b & 0xF;
                serial_write_u4(hi as u64);
                serial_write_u4(lo as u64);
            }
            serial_write("\n");

            rtl8168::RTL_DRIVER = Some(driver);
            serial_write("[net] RTL8168 driver ONLINE\n");
            // NOTA: stack::init() se llama desde boot::phases::phase4_scheduler
            // para tener el orden correcto: net detect → stack init.
        } else {
            serial_write("[net] WARNING: no supported NIC found\n");
            serial_write("[net]   supported: Realtek RTL8168/RTL8111 (10EC:8168)\n");
        }
    }
}

fn serial_write_u4(nibble: u64) {
    let c = if nibble < 10 { b'0' + nibble as u8 } else { b'A' + (nibble - 10) as u8 };
    crate::drivers::serial::serial_write_byte(c);
}

