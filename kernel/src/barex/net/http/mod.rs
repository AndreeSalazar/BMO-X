//! HTTP. **Default HTTP/3 (QUIC)**. HTTP/2 como fallback. HTTP/1.x **prohibido**
//! en código nuevo (legacy textual, headers ambiguos, 50 RFCs de parches).

pub mod client;
pub mod server;
pub mod version;

