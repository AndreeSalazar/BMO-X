//! BLAKE3 hashing for BSF signing

pub fn blake3(data: &[u8]) -> [u8; 32] {
    *blake3::hash(data).as_bytes()
}
