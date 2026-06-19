//! Tipos canónicos de red BMO. Reemplazan el zoo C de:
//! `sockaddr`, `sockaddr_in`, `sockaddr_in6`, `in_addr`, `in6_addr`,
//! `IN_ADDR`, `IPADDR`, `SOCKADDR_STORAGE`, `inet_aton`, `inet_pton`.

pub mod ip;
pub mod endpoint;
pub mod mac;
pub mod cidr;
pub mod port;

pub use ip::IpAddr;
pub use endpoint::Endpoint;
pub use mac::MacAddr;
