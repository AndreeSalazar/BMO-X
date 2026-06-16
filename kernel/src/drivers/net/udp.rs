#![allow(dead_code)]

//! UDP (User Datagram Protocol) for FastOS.
//! Minimal send/receive — used by DHCP client.
//!
//! RFC 768 — 8-byte header: src_port, dst_port, length, checksum.

use alloc::vec::Vec;

/// UDP statistics.
pub static mut UDP_STATS: UdpStats = UdpStats {
    packets_sent: 0,
    packets_received: 0,
};

pub struct UdpStats {
    pub packets_sent: u64,
    pub packets_received: u64,
}

/// Send a UDP datagram.
///
/// `src_port`, `dst_port`: port numbers
/// `dst_ip`: destination IPv4
/// `payload`: UDP payload (checksum computed over pseudo-header + header + data)
pub fn send(src_port: u16, dst_port: u16, dst_ip: u32, payload: &[u8]) -> Result<usize, &'static str> {
    let my_ip = super::stack::local_ip();

    let udp_len = 8 + payload.len();
    let mut pkt = [0u8; 1500];

    // UDP header
    pkt[0] = (src_port >> 8) as u8;
    pkt[1] = src_port as u8;
    pkt[2] = (dst_port >> 8) as u8;
    pkt[3] = dst_port as u8;
    pkt[4] = (udp_len >> 8) as u8;
    pkt[5] = udp_len as u8;
    pkt[6] = 0; pkt[7] = 0; // checksum (optional for UDP over IPv4, but we compute it)

    // Payload
    pkt[8..8 + payload.len()].copy_from_slice(payload);

    // Compute UDP checksum (pseudo-header + header + data)
    let mut pseudo = [0u8; 12];
    pseudo[0..4].copy_from_slice(&my_ip.to_be_bytes());
    pseudo[4..8].copy_from_slice(&dst_ip.to_be_bytes());
    pseudo[8] = 0;
    pseudo[9] = 17; // UDP protocol
    pseudo[10..12].copy_from_slice(&(udp_len as u16).to_be_bytes());

    let mut cksum_data = Vec::new();
    cksum_data.extend_from_slice(&pseudo);
    cksum_data.extend_from_slice(&pkt[..udp_len]);

    let cksum = udp_checksum(&cksum_data);
    pkt[6] = (cksum >> 8) as u8;
    pkt[7] = cksum as u8;

    let result = super::ip::send_packet(17, dst_ip, &pkt[..udp_len]);

    if result.is_ok() {
        unsafe { UDP_STATS.packets_sent += 1; }
    }

    result
}

/// Handle an incoming UDP datagram.
///
/// `src_ip`: source IP of the IP packet
/// `payload`: UDP segment (header + data)
pub fn handle_packet(src_ip: u32, payload: &[u8]) {
    if payload.len() < 8 {
        return;
    }

    let src_port = ((payload[0] as u16) << 8) | (payload[1] as u16);
    let dst_port = ((payload[2] as u16) << 8) | (payload[3] as u16);
    let _udp_len = ((payload[4] as u16) << 8) | (payload[5] as u16);
    let data = &payload[8..];

    unsafe { UDP_STATS.packets_received += 1; }

    // Route to handler based on destination port
    match dst_port {
        67 | 68 => super::dhcp::handle_udp_packet(src_ip, src_port, dst_port, data),
        _ => {} // Unknown port — drop
    }
}

/// Compute UDP checksum (with pseudo-header already included in `data`).
fn udp_checksum(data: &[u8]) -> u16 {
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
