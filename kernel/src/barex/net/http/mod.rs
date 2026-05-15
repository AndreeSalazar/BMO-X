//! HTTP. **Default HTTP/3 (QUIC)**. HTTP/2 como fallback. HTTP/1.x **prohibido**
//! en código nuevo (legacy textual, headers ambiguos, 50 RFCs de parches).

pub mod client;
pub mod server;
pub mod version;

pub use client::Http3Client;
pub use server::Http3Server;
pub use version::HttpVersion;
