//! TLS 1.3 only. Sin SSLv2/3, TLS 1.0/1.1/1.2 (todo deprecado).
//! Sin OpenSSL (50 KLOC de pesadilla CVE), sin SChannel, sin GnuTLS.
//! Implementación nativa siguiendo RFC 8446. ~5 KLOC objetivo.

pub mod context;
pub mod cipher;

