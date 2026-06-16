//! Network subsystem for FastOS.
//!
//! Modular network stack:
//!   - RTL8168: Ethernet NIC driver (L2)
//!   - ARP: Address Resolution Protocol (L2→L3)
//!   - IP: Internet Protocol (L3)
//!   - ICMP: Internet Control Message Protocol (ping)
//!   - UDP: User Datagram Protocol (L4)
//!   - DHCP: Dynamic Host Configuration Protocol (auto-IP)
//!   - Stack: Unified network stack integration

pub mod rtl8168;
pub mod arp;
pub mod ip;
pub mod icmp;
pub mod udp;
pub mod dhcp;
pub mod stack;
