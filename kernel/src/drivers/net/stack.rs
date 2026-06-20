#![allow(unused_results, dead_code)]

#![allow(dead_code)]

//! Network stack for FastOS.
//!
//! Integrates RTL8168 NIC + ARP + IP + ICMP + UDP + DHCP.
//! Provides a unified API for network initialization and polling.

#![allow(unused_results, dead_code)]

use crate::drivers::serial;

// ── Network state ─────────────────────────────────────────────────────
static mut NET_INITIALIZED: bool = false;

/// Returns true if the network stack has been initialized.
pub fn is_initialized() -> bool {
    unsafe { NET_INITIALIZED }
}

/// Get the local IPv4 address (from DHCP or fallback).
pub fn local_ip() -> u32 {
    super::dhcp::local_ip()
}

/// Get the default gateway.
pub fn gateway_ip() -> u32 {
    super::dhcp::gateway()
}

/// Get the subnet mask.
pub fn subnet_mask() -> u32 {
    super::dhcp::subnet_mask()
}

/// Get the DNS server.
pub fn dns_ip() -> u32 {
    unsafe { super::dhcp::DHCP_DNS }
}

/// Initialize the network stack.
/// - Detects RTL8168 NIC (already done in Phase 2)
/// - Sends DHCP Discover to obtain IP configuration
pub fn init() -> Result<(), &'static str> {
    if unsafe { NET_INITIALIZED } {
        return Ok(());
    }

    // Check NIC is present
    let nic_up = unsafe {
        super::rtl8168::RTL_DRIVER.as_mut().map(|d| d.is_link_up()).unwrap_or(false)
    };

    if !nic_up {
        serial::serial_write("[net] NIC not present or link down\n");
        return Err("NIC not ready");
    }

    serial::serial_write("[net] Network stack initializing...\n");

    // Start DHCP
    super::dhcp::send_discover()?;

    unsafe { NET_INITIALIZED = true; }

    serial::serial_write("[net] DHCP Discover sent, waiting for configuration...\n");
    Ok(())
}

/// Poll the network stack — call this periodically from the idle loop or scheduler.
/// Processes incoming packets, handles ARP timeouts, renews DHCP.
pub fn poll() {
    let mut buf = [0u8; 2048];

    // Receive a frame from the NIC
    let frame_len = match unsafe {
        super::rtl8168::RTL_DRIVER.as_mut().and_then(|d| d.recv(&mut buf))
    } {
        Some(len) => len,
        None => return, // No packet
    };

    if frame_len < 14 {
        return;
    }

    // Parse Ethernet type
    let ethertype = ((buf[12] as u16) << 8) | (buf[13] as u16);

    match ethertype {
        0x0806 => super::arp::handle_packet(&buf[..frame_len]),
        0x0800 => super::ip::handle_packet(&buf[..frame_len]),
        _ => {} // Unknown ethertype — drop
    }
}

/// Print network status to serial.
pub fn print_status() {
    let ip = local_ip();
    let gw = gateway_ip();
    let mask = subnet_mask();

    serial::serial_write("[net] IP: ");
    serial_ip(ip);
    serial::serial_write(" GW: ");
    serial_ip(gw);
    serial::serial_write(" Mask: ");
    serial_ip(mask);
    serial::serial_write(" State: ");

    match unsafe { super::dhcp::DHCP_STATE } {
        super::dhcp::DhcpState::Bound => serial::serial_write("BOUND"),
        super::dhcp::DhcpState::Idle => serial::serial_write("IDLE"),
        super::dhcp::DhcpState::DiscoverSent => serial::serial_write("DISCOVER"),
        super::dhcp::DhcpState::RequestSent => serial::serial_write("REQUEST"),
    }
    serial::serial_write("\n");
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

