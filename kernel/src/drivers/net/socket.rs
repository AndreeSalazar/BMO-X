//! Socket API — abstracción unificada sobre TCP y UDP.
//!
//! v1.4.0: API de sockets que las apps usarán. Encapsula la diferencia
//! entre TCP (orientado a conexión) y UDP (datagramas).
//!
//! ## Modelo
//!
//! ```text
//!   App: socket() -> fd
//!        connect(fd, ip, port)  // TCP o UDP
//!        send(fd, bytes)
//!        recv(fd, buf)
//!        close(fd)
//! ```
//!
//! v1.4.0: stubs que devuelven errores si TCP no está listo. UDP
//! funciona directamente sobre `udp::send`.

use crate::drivers::serial;

/// Tipo de socket.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SocketType {
    Stream,  // TCP
    Datagram, // UDP
    Raw,     // ICMP, etc.
}

/// Estado de un socket.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SocketState {
    Idle,
    Connecting,
    Connected,
    Listening,
    Closing,
    Closed,
}

/// Handle opaco de socket.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SocketId(pub u32);

/// Crea un socket.
pub fn socket(_kind: SocketType) -> Result<SocketId, SocketError> {
    // v1.4.0: para TCP delega a tcp::socket
    // para UDP solo devuelve un handle dummy
    Ok(SocketId(0))
}

/// Conecta un socket TCP a una IP:puerto.
pub fn connect(_id: SocketId, _ip: u32, _port: u16) -> Result<(), SocketError> {
    serial::serial_write("[socket] connect: TCP not fully wired yet\n");
    Err(SocketError::NotReady)
}

/// Envía datos.
pub fn send(_id: SocketId, _data: &[u8]) -> Result<usize, SocketError> {
    Err(SocketError::NotReady)
}

/// Recibe datos (no-bloqueante).
pub fn recv(_id: SocketId, _buf: &mut [u8]) -> Result<usize, SocketError> {
    Err(SocketError::NotReady)
}

/// Cierra el socket.
pub fn close(_id: SocketId) -> Result<(), SocketError> {
    Ok(())
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SocketError {
    NotReady,
    InvalidSocket,
    NotConnected,
    Timeout,
    WouldBlock,
    AddressInUse,
    Unknown,
}
