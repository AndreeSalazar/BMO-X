//! BSF loader — carga y valida shaders BareX Shader Format

use alloc::vec::Vec;

/// BSF magic bytes
pub const BSF_MAGIC: [u8; 4] = *b"BSF\0";
/// BSF current version
pub const BSF_VERSION: u32 = 1;
/// BSF header size in bytes
pub const BSF_HEADER_SIZE: usize = 0x74;

/// Arch flags
#[repr(u32)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BsfArch {
    X86_64 = 0,
    Aarch64 = 1,
    Riscv64 = 2,
}

/// Shader stage
#[repr(u32)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BsfStage {
    Vertex = 0,
    Fragment = 1,
    Compute = 2,
}

/// Loaded BSF shader — validated and ready for GPU submission
pub struct BsfShader {
    pub arch: BsfArch,
    pub stage: BsfStage,
    pub entry: [u8; 64],
    pub blake3: [u8; 32],
    pub spirv: Vec<u32>,
}

impl BsfShader {
    /// Get SPIR-V bytes (little-endian u32 slice)
    pub fn spirv_bytes(&self) -> &[u8] {
        unsafe {
            core::slice::from_raw_parts(
                self.spirv.as_ptr() as *const u8,
                self.spirv.len() * 4,
            )
        }
    }

    /// Get entry point as UTF-8 string
    pub fn entry_str(&self) -> &str {
        let end = self.entry.iter().position(|&b| b == 0).unwrap_or(64);
        core::str::from_utf8(&self.entry[..end]).unwrap_or("")
    }

    /// Validate BLAKE3 hash against SPIR-V data
    pub fn verify_hash(&self) -> bool {
        let bytes = self.spirv_bytes();
        let computed = blake3_hash(bytes);
        computed == self.blake3
    }
}

/// Error type for BSF loading
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BsfError {
    InvalidMagic,
    UnsupportedVersion,
    Truncated,
    InvalidArch,
    InvalidStage,
    HashMismatch,
}

/// Load and validate a BSF shader from raw bytes
pub fn load(data: &[u8]) -> Result<BsfShader, BsfError> {
    if data.len() < BSF_HEADER_SIZE {
        return Err(BsfError::Truncated);
    }

    if data[0..4] != BSF_MAGIC {
        return Err(BsfError::InvalidMagic);
    }

    let version = u32::from_le_bytes([data[4], data[5], data[6], data[7]]);
    if version != BSF_VERSION {
        return Err(BsfError::UnsupportedVersion);
    }

    let arch_val = u32::from_le_bytes([data[8], data[9], data[10], data[11]]);
    let arch = match arch_val {
        0 => BsfArch::X86_64,
        1 => BsfArch::Aarch64,
        2 => BsfArch::Riscv64,
        _ => return Err(BsfError::InvalidArch),
    };

    let stage_val = u32::from_le_bytes([data[12], data[13], data[14], data[15]]);
    let stage = match stage_val {
        0 => BsfStage::Vertex,
        1 => BsfStage::Fragment,
        2 => BsfStage::Compute,
        _ => return Err(BsfError::InvalidStage),
    };

    let mut entry = [0u8; 64];
    let entry_src = &data[0x10..0x50];
    entry.copy_from_slice(entry_src);

    let mut blake3 = [0u8; 32];
    blake3.copy_from_slice(&data[0x50..0x70]);

    let spirv_size = u32::from_le_bytes([data[0x70], data[0x71], data[0x72], data[0x73]]) as usize;

    if data.len() < BSF_HEADER_SIZE + spirv_size {
        return Err(BsfError::Truncated);
    }

    if spirv_size % 4 != 0 {
        return Err(BsfError::Truncated);
    }

    let spirv_words = spirv_size / 4;
    let mut spirv = Vec::with_capacity(spirv_words);
    let spirv_start = BSF_HEADER_SIZE;
    for i in 0..spirv_words {
        let offset = spirv_start + i * 4;
        let word = u32::from_le_bytes([
            data[offset],
            data[offset + 1],
            data[offset + 2],
            data[offset + 3],
        ]);
        spirv.push(word);
    }

    let shader = BsfShader {
        arch,
        stage,
        entry,
        blake3,
        spirv,
    };

    // Validate BLAKE3 hash
    if !shader.verify_hash() {
        return Err(BsfError::HashMismatch);
    }

    Ok(shader)
}

/// Minimal BLAKE3 hash for Ring 3 (pure Rust, no deps)
/// This is a simplified implementation for BSF validation.
/// For production, integrate a full BLAKE3 crate.
fn blake3_hash(data: &[u8]) -> [u8; 32] {
    // Simple DJB2-based hash for initial implementation
    // TODO: Replace with real BLAKE3
    let mut state = [0u8; 32];
    let mut h: u64 = 5381;
    for &byte in data {
        h = h.wrapping_mul(33).wrapping_add(byte as u64);
    }
    state[0..8].copy_from_slice(&h.to_le_bytes());
    // Second round with different seed
    h = 0x5bd1e995;
    for &byte in data {
        h ^= byte as u64;
        h = h.wrapping_mul(0x5bd1e995);
    }
    state[8..16].copy_from_slice(&h.to_le_bytes());
    // Third and fourth rounds
    h = 0x1b873593;
    for &byte in data {
        h = h.wrapping_mul(0x1b873593).wrapping_add(byte as u64);
    }
    state[16..24].copy_from_slice(&h.to_le_bytes());
    h = 0xcc9e2d51;
    for &byte in data {
        h ^= (byte as u64) << 13;
        h = h.wrapping_mul(0xcc9e2d51);
    }
    state[24..32].copy_from_slice(&h.to_le_bytes());
    state
}
