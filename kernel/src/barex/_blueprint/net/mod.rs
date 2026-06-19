//! `barex::net` — `bx_net`, subsistema de red **agresivo zero-bloat** de FastOS.
//!
//! Spec: `BareX_Network_Spec.md`.
//!
//! ## Lo que **NO existe** aquí (eliminado por construcción)
//!
//! | Bloat eliminado            | Origen                | Reemplazo BMO                |
//! |----------------------------|-----------------------|------------------------------|
//! | Winsock (`WSAStartup`)     | Win32                 | `BxNetService::init()`       |
//! | `sockaddr_in/in6` zoo      | BSD/POSIX             | `Endpoint` 24 B              |
//! | `int fd` opaco             | POSIX                 | `BmoHandle` con generación   |
//! | `errno` / `WSAGetLastError`| C ABI                 | `BmoStatus` en RAX:RDX       |
//! | OpenSSL / SChannel         | bloat criptográfico   | `tls::` ring-based, TLS 1.3  |
//! | NetBIOS / WPAD / SMB       | legacy Windows        | (no implementado, prohibido) |
//! | `getaddrinfo`              | DNS plano (UDP/53)    | `dns::` DoH/DoT only         |
//! | epoll / kqueue / IOCP      | event loops legados   | `ring::` SQ/CQ io_uring-like |
//! | NDIS / AF_PACKET           | kernel-net bloat      | `driver::` directo PCIe MMIO |
//! | libcurl / WinHTTP          | HTTP cliente legacy   | `http::` HTTP/3 nativo       |
//!
//! ## Estructura modular (Sesión 10)
//!
//! ```
//!   net/
//!   ├── mod.rs            ← este archivo (re-exports + service)
//!   ├── capabilities.rs   ← NetCapabilities + sandbox
//!   ├── types/            ← IpAddr, Endpoint, MacAddr, Cidr
//!   ├── socket/           ← BxTcpSocket, BxUdpSocket (sin Winsock)
//!   ├── quic/             ← BxQuicEndpoint + BxQuicStream
//!   ├── tls/              ← TLS 1.3 only, sin OpenSSL
//!   ├── http/             ← HTTP/3 + HTTP/2 (sin WinHTTP)
//!   ├── dns/              ← DoH/DoT only (sin getaddrinfo)
//!   ├── ring/             ← SQ/CQ io_uring-style (sin epoll/IOCP)
//!   ├── driver/           ← Bridge a NIC (Realtek/Intel) directo MMIO
//!   └── bypass/           ← kernel-bypass DPDK-style para HFT/gaming
//! ```

#![allow(dead_code)]

use crate::barex::{BxError, BxResult};

pub mod capabilities;
pub mod types;
pub mod socket;
pub mod quic;
pub mod tls;
pub mod http;
pub mod dns;
pub mod ring;
pub mod driver;
pub mod bypass;

// ─── Re-exports planos ───────────────────────────────────────────────

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
    /// Kernel bypass: app habla directo a la NIC (sin checksum offload).
    Raw,
}

/// Servicio raíz de red. Singleton por proceso.
pub struct BxNetService {
    _private: (),
}

impl BxNetService {
    pub fn init() -> BxResult<Self> {
        // TODO: Realtek RTL8125B / Intel I225-V driver vía `driver::`.
        Err(BxError::NotImplemented)
    }
}

/// Versión del stack `bx_net` (ABI estable para apps que lo consumen).
pub const BX_NET_VERSION: (u8, u8, u8) = (1, 0, 0);
