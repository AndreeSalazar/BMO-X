//! `barex::net` — `bx_net`, subsistema de red de FastOS.
//!
//! Spec: `BareX_Network_Spec.md`. Stack TCP/IP/QUIC propio en Rust con
//! io_uring-style SQ/CQ y kernel bypass opcional. Default HTTP/3 + TLS 1.3
//! + DoH. Sin Winsock, sin SChannel, sin NetBIOS, sin BITS, sin WPAD.

use crate::barex::{BxError, BxResult};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Protocol {
    Tcp,
    Udp,
    Quic,
    Tls13,
    Http2,
    Http3,
    WebSocket,
    WebTransport,
}

#[derive(Debug, Clone, Copy)]
pub struct PortV4 {
    pub octets: [u8; 4],
    pub port: u16,
}

pub struct BxNetService {
    _private: (),
}

pub struct BxTcpSocket {
    _private: (),
}

pub struct BxUdpSocket {
    _private: (),
}

pub struct BxQuicEndpoint {
    _private: (),
}

impl BxNetService {
    pub fn init() -> BxResult<Self> {
        // TODO: Realtek RTL8125B / Intel I225-V driver + smoltcp-rs base.
        Err(BxError::NotImplemented)
    }
}

/// Capabilities declaradas por la app en su `manifest.bef.toml`.
#[derive(Debug, Clone, Copy, Default)]
pub struct NetCapabilities {
    pub allow_outbound: bool,
    pub allow_inbound: bool,
    pub allow_raw_kernel_bypass: bool,
}
