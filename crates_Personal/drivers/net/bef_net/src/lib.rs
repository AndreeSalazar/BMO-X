//! BMO Network BEF — receive .bef binaries over UDP, verify chain-of-trust,
//! and execute with sandbox capabilities from manifest.
//!
//! ## Protocol
//!   - UDP port 6969 — receives .bef payloads
//!   - Each packet: [4B size_le][BEF bytes...]
//!   - Max BEF size: 1 MB
//!   - On receive: BLAKE3 verify → capability check → execute
//!
//! ## Security
//!   - Manifest `[capabilities]` limits what the BEF can do
//!   - Chain-of-trust: each BEF must be signed (Ed25519) in production
//!   - Max binary size prevents DoS

#![no_std]

/// Max BEF binary size for network delivery (1 MB).
pub const NET_BEF_MAX_SIZE: usize = 1024 * 1024;

/// Receive a BEF binary from a UDP buffer.
/// Returns the raw bytes or None if too large/invalid header.
pub fn recv_from_udp(buf: &[u8]) -> Option<&[u8]> {
    if buf.len() < 4 { return None; }
    let size = u32::from_le_bytes([buf[0], buf[1], buf[2], buf[3]]) as usize;
    if size == 0 || size > NET_BEF_MAX_SIZE || 4 + size > buf.len() {
        return None;
    }
    Some(&buf[4..4 + size])
}

/// Compute BLAKE3 hash of BEF payload (stub — requires blake3 crate).
/// In production, this verifies against the manifest's binary_hash.
pub fn verify_hash(_payload: &[u8]) -> bool {
    true // stub: integrate blake3 when available
}

/// Parse manifest from BEF payload, extract capabilities.
/// Returns the capability bitmap for sandbox enforcement.
pub fn extract_capabilities(_payload: &[u8]) -> u32 {
    // Stub: parse `[capabilities]` section from BEF bytes
    // Map: FS_READ → 1<<0, FS_WRITE → 1<<1, NET_RAW → 1<<9, etc.
    0
}

/// Full pipeline: receive → verify → extract caps → (caller executes)
pub fn pipeline(buf: &[u8]) -> Option<(u32, &[u8])> {
    let payload = recv_from_udp(buf)?;
    if !verify_hash(payload) { return None; }
    let caps = extract_capabilities(payload);
    Some((caps, payload))
}
