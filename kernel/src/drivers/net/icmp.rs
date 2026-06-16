#![allow(dead_code)]

//! ICMP (Internet Control Message Protocol) for FastOS.
//! Handles ICMP Echo Request/Reply (ping).
//!
//! RFC 792 — Type 8 (Echo Request), Type 0 (Echo Reply).

use crate::drivers::serial;

// ── ICMP constants ────────────────────────────────────────────────────
const ICMP_ECHO_REQUEST: u8 = 8;
const ICMP_ECHO_REPLY: u8 = 0;

/// ICMP statistics (diagnostic).
pub static mut ICMP_STATS: IcmpStats = IcmpStats {
    requests_received: 0,
    replies_sent: 0,
    errors: 0,
};

pub struct IcmpStats {
    pub requests_received: u64,
    pub replies_sent: u64,
    pub errors: u64,
}

/// Compute ICMP checksum (same algorithm as IP checksum).
fn icmp_checksum(data: &[u8]) -> u16 {
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

/// Handle an incoming ICMP packet (IP proto 1).
///
/// `src_ip`: source IPv4 address of the IP packet
/// `payload`: ICMP message (after IP header)
pub fn handle_packet(src_ip: u32, payload: &[u8]) {
    if payload.len() < 8 {
        return;
    }

    let msg_type = payload[0];
    let _code = payload[1];

    match msg_type {
        ICMP_ECHO_REQUEST => {
            unsafe { ICMP_STATS.requests_received += 1; }

            // Build echo reply: swap type, recompute checksum
            let mut reply = payload.to_vec();
            reply[0] = ICMP_ECHO_REPLY;
            reply[1] = 0; // code
            reply[2] = 0; reply[3] = 0; // clear checksum
            let cksum = icmp_checksum(&reply);
            reply[2] = (cksum >> 8) as u8;
            reply[3] = cksum as u8;

            // Send via IP
            let _ = super::ip::send_packet(1, src_ip, &reply);

            unsafe { ICMP_STATS.replies_sent += 1; }

            // Diagnostic: log ping replies to serial
            let mut ip_buf = [0u8; 16];
            ip_to_str(src_ip, &mut ip_buf);
            serial::serial_write("[icmp] reply to ");
            serial::serial_write(core::str::from_utf8(&ip_buf).unwrap_or("?"));
            serial::serial_write("\n");
        }
        ICMP_ECHO_REPLY => {
            // Reply to our ping — no-op for now
        }
        _ => {
            unsafe { ICMP_STATS.errors += 1; }
        }
    }
}

/// Send an ICMP Echo Request (ping) to `target_ip`.
pub fn ping(target_ip: u32) -> Result<(), &'static str> {
    let id = 0xBEEF;
    let seq = 1;

    let mut payload = [0u8; 64];
    payload[0] = ICMP_ECHO_REQUEST;
    payload[1] = 0; // code
    payload[2] = 0; payload[3] = 0; // checksum (filled below)
    payload[4] = (id >> 8) as u8;
    payload[5] = id as u8;
    payload[6] = (seq >> 8) as u8;
    payload[7] = seq as u8;

    // Fill with pattern data
    for i in 8..64 {
        payload[i] = (i & 0xFF) as u8;
    }

    let cksum = icmp_checksum(&payload);
    payload[2] = (cksum >> 8) as u8;
    payload[3] = cksum as u8;

    super::ip::send_packet(1, target_ip, &payload).map(|_| ())
}

/// Format an IPv4 address as "a.b.c.d" into `buf`.
fn ip_to_str(ip: u32, buf: &mut [u8; 16]) {
    let b = ip.to_be_bytes();
    let mut pos = 0;
    for (idx, &octet) in b.iter().enumerate() {
        let val = octet;
        if val >= 100 {
            buf[pos] = b'0' + val / 100;
            pos += 1;
            buf[pos] = b'0' + (val / 10) % 10;
            pos += 1;
        } else if val >= 10 {
            buf[pos] = b'0' + val / 10;
            pos += 1;
        }
        buf[pos] = b'0' + val % 10;
        pos += 1;
        if idx < 3 {
            buf[pos] = b'.';
            pos += 1;
        }
    }
    for i in pos..16 {
        buf[i] = 0;
    }
}
