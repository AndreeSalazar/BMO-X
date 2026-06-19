//! BareX Network — TCP Socket implementation.
//!
//! Wraps the kernel network stack into the BareX socket API.

#![allow(dead_code)]

use super::super::BxError;

/// TCP connection state.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum TcpState {
    Closed,
    Listening,
    SynSent,
    SynReceived,
    Established,
    FinWait1,
    FinWait2,
    CloseWait,
    Closing,
    LastAck,
    TimeWait,
}

/// TCP socket handle.
pub struct BxTcpSocket {
    state: TcpState,
    local_port: u16,
    remote_port: u16,
    remote_ip: u32,
    rx_buf: [u8; 4096],
    rx_len: usize,
    tx_buf: [u8; 4096],
    tx_len: usize,
}

impl BxTcpSocket {
    pub fn state(&self) -> TcpState { self.state }
    pub fn is_connected(&self) -> bool { self.state == TcpState::Established }
}

/// Create a TCP socket.
pub fn tcp_create() -> Result<BxTcpSocket, BxError> {
    Ok(BxTcpSocket {
        state: TcpState::Closed,
        local_port: 0,
        remote_port: 0,
        remote_ip: 0,
        rx_buf: [0; 4096],
        rx_len: 0,
        tx_buf: [0; 4096],
        tx_len: 0,
    })
}

/// Bind to a local port.
pub fn tcp_bind(sock: &mut BxTcpSocket, port: u16) -> Result<(), BxError> {
    if sock.state != TcpState::Closed {
        return Err(BxError::InvalidArgument);
    }
    sock.local_port = port;
    Ok(())
}

/// Listen for incoming connections.
pub fn tcp_listen(sock: &mut BxTcpSocket) -> Result<(), BxError> {
    if sock.state != TcpState::Closed && sock.state != TcpState::Closed {
        return Err(BxError::InvalidArgument);
    }
    sock.state = TcpState::Listening;
    Ok(())
}

/// Connect to a remote host.
pub fn tcp_connect(sock: &mut BxTcpSocket, ip: u32, port: u16) -> Result<(), BxError> {
    if sock.state != TcpState::Closed {
        return Err(BxError::InvalidArgument);
    }

    // In a full implementation, this would:
    // 1. Send TCP SYN via the IP stack
    // 2. Wait for SYN-ACK
    // 3. Send ACK
    // For now, mark as connecting (the network stack will handle the handshake)

    sock.remote_ip = ip;
    sock.remote_port = port;
    sock.state = TcpState::SynSent;

    // TODO: Actually send SYN via IP stack
    // crate::drivers::net::ip::send_packet(6, ip, &syn_packet);

    Ok(())
}

/// Send data over TCP.
pub fn tcp_send(sock: &mut BxTcpSocket, data: &[u8]) -> Result<usize, BxError> {
    if sock.state != TcpState::Established {
        return Err(BxError::InvalidArgument);
    }

    let copy_len = data.len().min(sock.tx_buf.len() - sock.tx_len);
    sock.tx_buf[sock.tx_len..sock.tx_len + copy_len].copy_from_slice(&data[..copy_len]);
    sock.tx_len += copy_len;

    // TODO: Actually transmit via IP stack
    // For now, we buffer and pretend it was sent
    let sent = copy_len;
    sock.tx_len = 0; // Reset buffer (in real impl, wait for ACK)

    Ok(sent)
}

/// Receive data from TCP.
pub fn tcp_recv(sock: &mut BxTcpSocket, buf: &mut [u8]) -> Result<usize, BxError> {
    if sock.state != TcpState::Established {
        return Err(BxError::InvalidArgument);
    }

    if sock.rx_len == 0 {
        return Ok(0); // No data available
    }

    let copy_len = buf.len().min(sock.rx_len);
    buf[..copy_len].copy_from_slice(&sock.rx_buf[..copy_len]);

    // Shift remaining data
    if copy_len < sock.rx_len {
        sock.rx_buf.copy_within(copy_len..sock.rx_len, 0);
    }
    sock.rx_len -= copy_len;

    Ok(copy_len)
}

/// Close a TCP socket.
pub fn tcp_close(sock: &mut BxTcpSocket) -> Result<(), BxError> {
    sock.state = TcpState::Closed;
    sock.rx_len = 0;
    sock.tx_len = 0;
    Ok(())
}
