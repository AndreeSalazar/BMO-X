#![allow(unused_results, dead_code)]

#![allow(dead_code)]

//! IPv4 layer for FastOS.
//! Handles IP datagram receive/transmit, ICMP echo (ping), UDP send.
//!
//! Minimal implementation: no fragmentation, no options, no IPsec.

#![allow(unused_results, dead_code)]

use crate::drivers::net::rtl8168;

// ── IP constants ──────────────────────────────────────────────────────
const IP_PROTO_ICMP: u8 = 1;
const IP_PROTO_UDP: u8 = 17;

// ── IP header (20 bytes, no options) ──────────────────────────────────
#[repr(C, packed)]
struct IpHeader {
    ver_ihl: u8,
    dscp_ecn: u8,
    total_len: u16,
    id: u16,
    flags_frag: u16,
    ttl: u8,
    protocol: u8,
    checksum: u16,
    src_ip: u32,
    dst_ip: u32,
}

impl IpHeader {
    fn to_bytes(&self) -> [u8; 20] {
        let mut buf = [0u8; 20];
        unsafe {
            core::ptr::copy_nonoverlapping(
                self as *const Self as *const u8,
                buf.as_mut_ptr(),
                20,
            );
        }
        buf
    }
}

/// Compute IP header checksum (RFC 1071).
fn ip_checksum(data: &[u8]) -> u16 {
    let mut sum = 0u32;
    let mut i = 0;
    while i + 1 < data.len() {
        sum += ((data[i] as u32) << 8) | (data[i + 1] as u32);
        i += 2;
    }
    if i < data.len() {
        sum += (data[i] as u32) << 8;
    }
    while sum > 0xFFFF {
        sum = (sum & 0xFFFF) + (sum >> 16);
    }
    !sum as u16
}

/// Build and send an IP packet.
///
/// `proto`: IP protocol number (1=ICMP, 17=UDP, 6=TCP)
/// `dst_ip`: destination IPv4 address
/// `payload`: upper-layer payload (ICMP msg, UDP datagram, etc.)
pub fn send_packet(proto: u8, dst_ip: u32, payload: &[u8]) -> Result<usize, &'static str> {
    let my_ip = super::stack::local_ip();
    let _my_mac = unsafe {
        rtl8168::RTL_DRIVER.as_ref().map(|d| d.mac_address())
    }.ok_or("NIC not initialized")?;

    // Resolve destination MAC via ARP
    let dst_mac = if is_broadcast(dst_ip) || is_localnet(dst_ip, my_ip) {
        // Check ARP cache first, else use broadcast
        super::arp::lookup(dst_ip).unwrap_or([0xFF; 6])
    } else {
        super::arp::lookup(dst_ip).ok_or("ARP: no entry for target")?
    };

    let total_len = 20 + payload.len();

    let mut ip_hdr = IpHeader {
        ver_ihl: 0x45,     // IPv4, 5 × 4 = 20 bytes
        dscp_ecn: 0,
        total_len: total_len as u16,
        id: 0x1234,
        flags_frag: 0x4000, // Don't Fragment
        ttl: 64,
        protocol: proto,
        checksum: 0,
        src_ip: my_ip,
        dst_ip,
    };

    let mut pkt = [0u8; 1500];
    let hdr_bytes = ip_hdr.to_bytes();
    pkt[..20].copy_from_slice(&hdr_bytes);
    pkt[20..20 + payload.len()].copy_from_slice(payload);

    // Compute checksum over header
    ip_hdr.checksum = ip_checksum(&pkt[..20]);
    let hdr_bytes = ip_hdr.to_bytes();
    pkt[..20].copy_from_slice(&hdr_bytes);

    unsafe { rtl8168::send_frame(&dst_mac, 0x0800, &pkt[..total_len]) }
}

/// Handle an incoming IP packet (ethertype 0x0800).
pub fn handle_packet(eth_frame: &[u8]) {
    if eth_frame.len() < 34 {
        return; // Minimum: 14 (ETH) + 20 (IP)
    }

    let ip_start = 14;
    let ver_ihl = eth_frame[ip_start];
    let ihl = (ver_ihl & 0x0F) as usize * 4;
    if ihl < 20 || eth_frame.len() < ip_start + ihl {
        return;
    }

    let proto = eth_frame[ip_start + 9];
    let src_ip = u32::from_be_bytes([
        eth_frame[ip_start + 12],
        eth_frame[ip_start + 13],
        eth_frame[ip_start + 14],
        eth_frame[ip_start + 15],
    ]);
    let dst_ip = u32::from_be_bytes([
        eth_frame[ip_start + 16],
        eth_frame[ip_start + 17],
        eth_frame[ip_start + 18],
        eth_frame[ip_start + 19],
    ]);

    let my_ip = super::stack::local_ip();
    if dst_ip != my_ip && !is_broadcast(dst_ip) {
        return; // Not for us
    }

    let payload = &eth_frame[ip_start + ihl..];

    match proto {
        IP_PROTO_ICMP => super::icmp::handle_packet(src_ip, payload),
        IP_PROTO_UDP => super::udp::handle_packet(src_ip, payload),
        _ => {} // Drop unknown protocols
    }
}

fn is_broadcast(ip: u32) -> bool {
    ip == 0xFFFFFFFF || (ip & 0xFF) == 0xFF
}

fn is_localnet(ip: u32, my_ip: u32) -> bool {
    (ip >> 24) == (my_ip >> 24)
}

