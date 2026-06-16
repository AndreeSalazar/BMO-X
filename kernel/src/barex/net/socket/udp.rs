//! BareX Network — UDP Socket implementation.
//!
//! Wraps the kernel UDP stack into the BareX socket API.

#![allow(dead_code)]

use super::super::BxError;

/// UDP socket handle.
pub struct BxUdpSocket {
    bound_port: u16,
    bound: bool,
    rx_buf: [u8; 4096],
    rx_len: usize,
    rx_src_ip: u32,
    rx_src_port: u16,
}

impl BxUdpSocket {
    pub fn is_bound(&self) -> bool { self.bound }
    pub fn port(&self) -> u16 { self.bound_port }
}

/// Create a UDP socket.
pub fn udp_create() -> Result<BxUdpSocket, BxError> {
    Ok(BxUdpSocket {
        bound_port: 0,
        bound: false,
        rx_buf: [0; 4096],
        rx_len: 0,
        rx_src_ip: 0,
        rx_src_port: 0,
    })
}

/// Bind to a local port.
pub fn udp_bind(sock: &mut BxUdpSocket, port: u16) -> Result<(), BxError> {
    sock.bound_port = port;
    sock.bound = true;
    Ok(())
}

/// Send a UDP datagram.
pub fn udp_send(sock: &BxUdpSocket, dst_ip: u32, dst_port: u16, data: &[u8]) -> Result<usize, BxError> {
    if !sock.bound {
        return Err(BxError::NotInitialized);
    }

    match crate::drivers::net::udp::send(sock.bound_port, dst_port, dst_ip, data) {
        Ok(n) => Ok(n),
        Err(_) => Err(BxError::IoError),
    }
}

/// Receive a UDP datagram (non-blocking poll).
pub fn udp_recv(sock: &mut BxUdpSocket, buf: &mut [u8]) -> Result<(usize, u32, u16), BxError> {
    if !sock.bound {
        return Err(BxError::NotInitialized);
    }

    if sock.rx_len == 0 {
        return Ok((0, 0, 0)); // No data
    }

    let copy_len = buf.len().min(sock.rx_len);
    buf[..copy_len].copy_from_slice(&sock.rx_buf[..copy_len]);

    let src_ip = sock.rx_src_ip;
    let src_port = sock.rx_src_port;

    if copy_len < sock.rx_len {
        sock.rx_buf.copy_within(copy_len..sock.rx_len, 0);
    }
    sock.rx_len -= copy_len;

    Ok((copy_len, src_ip, src_port))
}

/// Close a UDP socket.
pub fn udp_close(sock: &mut BxUdpSocket) -> Result<(), BxError> {
    sock.bound = false;
    sock.rx_len = 0;
    Ok(())
}
