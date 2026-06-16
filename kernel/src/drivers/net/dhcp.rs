#![allow(dead_code)]

//! DHCP client for FastOS.
//! Discovers and requests IP configuration from a DHCP server.
//!
//! RFC 2131 — BOOTP-compatible, message types: Discover, Offer, Request, Ack.

use crate::drivers::serial;

// ── DHCP constants ────────────────────────────────────────────────────
const DHCP_BOOTREQUEST: u8 = 1;
const DHCP_BOOTREPLY: u8 = 2;
const DHCP_MAGIC: [u8; 4] = [0x63, 0x82, 0x53, 0x63]; // RFC 1048 magic cookie

// DHCP options
const OPT_MSG_TYPE: u8 = 53;
const OPT_REQUESTED_IP: u8 = 50;
const OPT_SERVER_IP: u8 = 54;
const OPT_SUBNET: u8 = 1;
const OPT_ROUTER: u8 = 3;
const OPT_DNS: u8 = 6;
const OPT_LEASE_TIME: u8 = 51;
const OPT_END: u8 = 255;

// Message types
const DHCP_DISCOVER: u8 = 1;
const DHCP_OFFER: u8 = 2;
const DHCP_REQUEST: u8 = 3;
const DHCP_ACK: u8 = 5;

// ── DHCP state ────────────────────────────────────────────────────────
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum DhcpState {
    Idle,
    DiscoverSent,
    RequestSent,
    Bound,
}

pub static mut DHCP_STATE: DhcpState = DhcpState::Idle;
pub static mut DHCP_IP: u32 = 0;
pub static mut DHCP_SUBNET: u32 = 0;
pub static mut DHCP_ROUTER: u32 = 0;
pub static mut DHCP_DNS: u32 = 0;
pub static mut DHCP_SERVER: u32 = 0;
pub static mut DHCP_LEASE_SECS: u32 = 0;

/// DHCP transaction ID (incremented per request).
static mut DHCP_XID: u32 = 0xDEAD_BEEF;

/// Get current DHCP-assigned IP.
pub fn local_ip() -> u32 {
    unsafe { DHCP_IP }
}

/// Get subnet mask.
pub fn subnet_mask() -> u32 {
    unsafe { DHCP_SUBNET }
}

/// Get default gateway.
pub fn gateway() -> u32 {
    unsafe { DHCP_ROUTER }
}

/// Send a DHCP Discover broadcast.
pub fn send_discover() -> Result<(), &'static str> {
    unsafe {
        DHCP_XID = DHCP_XID.wrapping_add(1);
        DHCP_STATE = DhcpState::DiscoverSent;
    }

    let xid = unsafe { DHCP_XID };
    let my_mac = unsafe {
        crate::drivers::net::rtl8168::RTL_DRIVER.as_ref().map(|d| d.mac_address())
    }.ok_or("NIC not initialized")?;

    let mut pkt = build_bootp_base(DHCP_BOOTREQUEST, xid, &my_mac);

    // Options: DHCP Message Type = Discover, end
    let options_start = 240;
    pkt[options_start] = OPT_MSG_TYPE;
    pkt[options_start + 1] = 1;
    pkt[options_start + 2] = DHCP_DISCOVER;
    pkt[options_start + 3] = OPT_END;

    // Pad to minimum BOOTP size (300 bytes)
    let total = 300;

    serial::serial_write("[dhcp] Sending DISCOVER (xid=");
    serial_hex(xid);
    serial::serial_write(")\n");

    crate::drivers::net::udp::send(68, 67, 0xFFFFFFFF, &pkt[..total])
        .map(|_| ())
}

/// Send a DHCP Request (after receiving an Offer).
pub fn send_request(server_ip: u32, requested_ip: u32) -> Result<(), &'static str> {
    unsafe {
        DHCP_XID = DHCP_XID.wrapping_add(1);
        DHCP_STATE = DhcpState::RequestSent;
    }

    let xid = unsafe { DHCP_XID };
    let my_mac = unsafe {
        crate::drivers::net::rtl8168::RTL_DRIVER.as_ref().map(|d| d.mac_address())
    }.ok_or("NIC not initialized")?;

    let mut pkt = build_bootp_base(DHCP_BOOTREQUEST, xid, &my_mac);

    let mut opt = 240;

    // DHCP Message Type = Request
    pkt[opt] = OPT_MSG_TYPE; pkt[opt + 1] = 1; pkt[opt + 2] = DHCP_REQUEST; opt += 3;

    // Requested IP
    pkt[opt] = OPT_REQUESTED_IP; pkt[opt + 1] = 4;
    pkt[opt + 2..opt + 6].copy_from_slice(&requested_ip.to_be_bytes()); opt += 6;

    // Server IP
    pkt[opt] = OPT_SERVER_IP; pkt[opt + 1] = 4;
    pkt[opt + 2..opt + 6].copy_from_slice(&server_ip.to_be_bytes()); opt += 6;

    // Parameter list: subnet, router, DNS
    pkt[opt] = 55; pkt[opt + 1] = 3;
    pkt[opt + 2] = OPT_SUBNET; pkt[opt + 3] = OPT_ROUTER; pkt[opt + 4] = OPT_DNS; opt += 5;

    pkt[opt] = OPT_END;

    let total = 300;

    serial::serial_write("[dhcp] Sending REQUEST (xid=");
    serial_hex(xid);
    serial::serial_write(")\n");

    crate::drivers::net::udp::send(68, 67, 0xFFFFFFFF, &pkt[..total])
        .map(|_| ())
}

/// Handle a DHCP packet received via UDP.
pub fn handle_udp_packet(src_ip: u32, _src_port: u16, _dst_port: u16, payload: &[u8]) {
    if payload.len() < 240 {
        return;
    }

    // Verify magic cookie
    if payload[236..240] != DHCP_MAGIC {
        return;
    }

    let _msg_type_byte = payload[0]; // op code
    let xid = u32::from_be_bytes([payload[4], payload[5], payload[6], payload[7]]);

    // Parse options
    let mut opt = 240;
    let mut dhcp_msg_type = 0u8;
    let mut offered_ip = 0u32;
    let mut server_id = 0u32;
    let mut subnet = 0u32;
    let mut router = 0u32;
    let mut dns = 0u32;
    let mut lease = 0u32;

    while opt < payload.len() {
        let code = payload[opt];
        if code == OPT_END {
            break;
        }
        if code == 0 {
            opt += 1;
            continue;
        }
        if opt + 1 >= payload.len() {
            break;
        }
        let len = payload[opt + 1] as usize;
        if opt + 2 + len > payload.len() {
            break;
        }
        let data = &payload[opt + 2..opt + 2 + len];

        match code {
            OPT_MSG_TYPE if len == 1 => { dhcp_msg_type = data[0]; }
            OPT_REQUESTED_IP if len == 4 => {
                offered_ip = u32::from_be_bytes([data[0], data[1], data[2], data[3]]);
            }
            OPT_SERVER_IP if len == 4 => {
                server_id = u32::from_be_bytes([data[0], data[1], data[2], data[3]]);
            }
            OPT_SUBNET if len == 4 => {
                subnet = u32::from_be_bytes([data[0], data[1], data[2], data[3]]);
            }
            OPT_ROUTER if len >= 4 => {
                router = u32::from_be_bytes([data[0], data[1], data[2], data[3]]);
            }
            OPT_DNS if len >= 4 => {
                dns = u32::from_be_bytes([data[0], data[1], data[2], data[3]]);
            }
            OPT_LEASE_TIME if len == 4 => {
                lease = u32::from_be_bytes([data[0], data[1], data[2], data[3]]);
            }
            _ => {}
        }
        opt += 2 + len;
    }

    // Also read yiaddr (your IP) from BOOTP header field at offset 16
    let yiaddr = u32::from_be_bytes([payload[16], payload[17], payload[18], payload[19]]);

    // Verify XID matches our request
    if xid != unsafe { DHCP_XID } {
        return;
    }

    match dhcp_msg_type {
        DHCP_OFFER => {
            serial::serial_write("[dhcp] Received OFFER: ");
            serial_ip(yiaddr);
            serial::serial_write("\n");

            let target_ip = if yiaddr != 0 { yiaddr } else { offered_ip };
            let srv = if server_id != 0 { server_id } else { src_ip };

            // Send Request
            let _ = send_request(srv, target_ip);
        }
        DHCP_ACK => {
            let final_ip = if yiaddr != 0 { yiaddr } else { offered_ip };

            unsafe {
                DHCP_IP = final_ip;
                DHCP_SUBNET = subnet;
                DHCP_ROUTER = router;
                DHCP_DNS = dns;
                DHCP_SERVER = server_id;
                DHCP_LEASE_SECS = lease;
                DHCP_STATE = DhcpState::Bound;
            }

            serial::serial_write("[dhcp] BOUND: IP=");
            serial_ip(final_ip);
            serial::serial_write(" GW=");
            serial_ip(router);
            serial::serial_write(" DNS=");
            serial_ip(dns);
            serial::serial_write(" LEASE=");
            serial_u32(lease);
            serial::serial_write("s\n");
        }
        _ => {}
    }
}

/// Build a base BOOTP packet (common fields for Discover/Request).
fn build_bootp_base(op: u8, xid: u32, mac: &[u8; 6]) -> [u8; 400] {
    let mut pkt = [0u8; 400];

    pkt[0] = op;                         // op: BOOTREQUEST
    pkt[1] = 1;                          // htype: Ethernet
    pkt[2] = 6;                          // hlen: 6
    pkt[3] = 0;                          // hops: 0
    pkt[4..8].copy_from_slice(&xid.to_be_bytes()); // xid
    pkt[10..12].copy_from_slice(&[0; 2]); // secs
    pkt[12..14].copy_from_slice(&[0; 2]); // flags (broadcast)

    // ciaddr, yiaddr, siaddr, giaddr = 0 (all zeros)

    // chaddr: client MAC (offset 28)
    pkt[28..34].copy_from_slice(mac);

    // sname, file = 0 (zeroed)

    // Magic cookie
    pkt[236..240].copy_from_slice(&DHCP_MAGIC);

    pkt
}

fn serial_hex(val: u32) {
    let hex = b"0123456789ABCDEF";
    let mut buf = [0u8; 8];
    for i in 0..8 {
        buf[7 - i] = hex[((val >> (i * 4)) & 0xF) as usize];
    }
    serial::serial_write(core::str::from_utf8(&buf).unwrap_or("00000000"));
}

fn serial_ip(ip: u32) {
    let b = ip.to_be_bytes();
    let mut buf = [0u8; 16];
    let mut pos = 0;
    for (idx, &octet) in b.iter().enumerate() {
        let val = octet;
        if val >= 100 { buf[pos] = b'0' + val / 100; pos += 1; }
        if val >= 10 { buf[pos] = b'0' + (val / 10) % 10; pos += 1; }
        buf[pos] = b'0' + val % 10; pos += 1;
        if idx < 3 { buf[pos] = b'.'; pos += 1; }
    }
    serial::serial_write(core::str::from_utf8(&buf[..pos]).unwrap_or("?"));
}

fn serial_u32(val: u32) {
    let mut buf = [0u8; 10];
    let mut i = buf.len();
    let mut v = val;
    if v == 0 { i -= 1; buf[i] = b'0'; }
    else { while v > 0 { i -= 1; buf[i] = b'0' + (v % 10) as u8; v /= 10; } }
    serial::serial_write(core::str::from_utf8(&buf[i..]).unwrap_or("0"));
}
