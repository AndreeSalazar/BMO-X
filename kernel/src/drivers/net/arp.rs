#![allow(unused_results, dead_code)]

//! ARP (Address Resolution Protocol) for FastOS.
//! Resolves IPv4 addresses to MAC addresses on the local LAN.
//!
//! RFC 826 — ARP request/reply over Ethernet (ethertype 0x0806).

use crate::drivers::net::rtl8168;

// ── ARP constants ─────────────────────────────────────────────────────
const ARP_HTYPE_ETHERNET: u16 = 1;
const ARP_PTYPE_IPV4: u16 = 0x0800;
const ARP_HLEN: u8 = 6;
const ARP_PLEN: u8 = 4;
const ARP_OP_REQUEST: u16 = 1;
const ARP_OP_REPLY: u16 = 2;

// ── ARP cache ─────────────────────────────────────────────────────────
const ARP_CACHE_SIZE: usize = 64;

#[derive(Clone, Copy)]
struct ArpEntry {
    ip: u32,
    mac: [u8; 6],
    valid: bool,
}

static mut ARP_CACHE: [ArpEntry; ARP_CACHE_SIZE] = [ArpEntry {
    ip: 0,
    mac: [0; 6],
    valid: false,
}; ARP_CACHE_SIZE];

/// Look up MAC address for an IPv4 address in the ARP cache.
pub fn lookup(ip: u32) -> Option<[u8; 6]> {
    unsafe {
        for entry in ARP_CACHE.iter() {
            if entry.valid && entry.ip == ip {
                return Some(entry.mac);
            }
        }
    }
    None
}

/// Add an entry to the ARP cache (from incoming ARP reply or gratuitous ARP).
pub fn cache_insert(ip: u32, mac: [u8; 6]) {
    unsafe {
        // Find existing or empty slot
        let mut empty_slot = None;
        for entry in ARP_CACHE.iter_mut() {
            if entry.valid && entry.ip == ip {
                entry.mac = mac;
                return;
            }
            if !entry.valid && empty_slot.is_none() {
                empty_slot = Some(entry as *mut ArpEntry);
            }
        }
        if let Some(slot) = empty_slot {
            (*slot).ip = ip;
            (*slot).mac = mac;
            (*slot).valid = true;
        }
    }
}

/// Build and send an ARP request for `target_ip`.
pub fn send_request(target_ip: u32) -> Result<(), &'static str> {
    let my_ip = super::stack::local_ip();
    let my_mac = unsafe {
        rtl8168::RTL_DRIVER.as_ref().map(|d| d.mac_address())
    }.ok_or("NIC not initialized")?;

    let mut pkt = [0u8; 42];

    // Ethernet header (14 bytes)
    pkt[0..6].copy_from_slice(&[0xFF; 6]); // Broadcast MAC
    pkt[6..12].copy_from_slice(&my_mac);
    pkt[12] = 0x08; pkt[13] = 0x06; // Ethertype: ARP

    // ARP header (28 bytes)
    pkt[14..16].copy_from_slice(&ARP_HTYPE_ETHERNET.to_be_bytes());
    pkt[16..18].copy_from_slice(&ARP_PTYPE_IPV4.to_be_bytes());
    pkt[18] = ARP_HLEN;
    pkt[19] = ARP_PLEN;
    pkt[20..22].copy_from_slice(&ARP_OP_REQUEST.to_be_bytes());

    // Sender MAC + IP
    pkt[22..28].copy_from_slice(&my_mac);
    pkt[28..32].copy_from_slice(&my_ip.to_be_bytes());

    // Target MAC (unknown) + Target IP
    pkt[32..38].copy_from_slice(&[0x00; 6]);
    pkt[38..42].copy_from_slice(&target_ip.to_be_bytes());

    unsafe { rtl8168::send_frame(&[0xFF; 6], 0x0806, &pkt[14..]) }
        .map(|_| ())
}

/// Send an ARP reply (response to an ARP request for our IP).
pub fn send_reply(target_mac: &[u8; 6], target_ip: u32) -> Result<(), &'static str> {
    let my_ip = super::stack::local_ip();
    let my_mac = unsafe {
        rtl8168::RTL_DRIVER.as_ref().map(|d| d.mac_address())
    }.ok_or("NIC not initialized")?;

    let mut pkt = [0u8; 42];

    // Ethernet header
    pkt[0..6].copy_from_slice(target_mac);
    pkt[6..12].copy_from_slice(&my_mac);
    pkt[12] = 0x08; pkt[13] = 0x06;

    // ARP header
    pkt[14..16].copy_from_slice(&ARP_HTYPE_ETHERNET.to_be_bytes());
    pkt[16..18].copy_from_slice(&ARP_PTYPE_IPV4.to_be_bytes());
    pkt[18] = ARP_HLEN;
    pkt[19] = ARP_PLEN;
    pkt[20..22].copy_from_slice(&ARP_OP_REPLY.to_be_bytes());

    // Sender
    pkt[22..28].copy_from_slice(&my_mac);
    pkt[28..32].copy_from_slice(&my_ip.to_be_bytes());

    // Target
    pkt[32..38].copy_from_slice(target_mac);
    pkt[38..42].copy_from_slice(&target_ip.to_be_bytes());

    unsafe { rtl8168::send_frame(target_mac, 0x0806, &pkt[14..]) }
        .map(|_| ())
}

/// Handle an incoming ARP packet (ethertype 0x0806).
pub fn handle_packet(eth_frame: &[u8]) {
    if eth_frame.len() < 42 {
        return;
    }

    let htype = u16::from_be_bytes([eth_frame[14], eth_frame[15]]);
    let ptype = u16::from_be_bytes([eth_frame[16], eth_frame[17]]);
    let op = u16::from_be_bytes([eth_frame[20], eth_frame[21]]);

    if htype != ARP_HTYPE_ETHERNET || ptype != ARP_PTYPE_IPV4 {
        return;
    }

    let sender_mac = &eth_frame[22..28];
    let sender_ip = u32::from_be_bytes([eth_frame[28], eth_frame[29], eth_frame[30], eth_frame[31]]);
    let target_ip = u32::from_be_bytes([eth_frame[38], eth_frame[39], eth_frame[40], eth_frame[41]]);

    let my_ip = super::stack::local_ip();

    // Always cache the sender
    let mut mac = [0u8; 6];
    mac.copy_from_slice(sender_mac);
    cache_insert(sender_ip, mac);

    match op {
        ARP_OP_REQUEST => {
            // Reply if someone is asking for our IP
            if target_ip == my_ip {
                let _ = send_reply(&mac, sender_ip);
            }
        }
        ARP_OP_REPLY => {
            // Already cached above
        }
        _ => {}
    }
}

