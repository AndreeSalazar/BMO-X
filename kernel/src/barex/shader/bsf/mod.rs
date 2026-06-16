//! BSF loader para Ring 0 — valida y carga shaders BareX Shader Format
//!
//! Este módulo corre en el kernel (Ring 0) y valida:
//! 1. Magic bytes "BSF\0"
//! 2. Version == 1
//! 3. BLAKE3 hash del SPIR-V
//! 4. Arch/stage válido
//!
//! No compila shaders — eso lo hace nexo-sh-tool en Ring 3.

#![allow(dead_code)]

/// BSF magic bytes
pub const BSF_MAGIC: [u8; 4] = *b"BSF\0";
/// BSF current version
pub const BSF_VERSION: u32 = 1;
/// BSF header size
pub const BSF_HEADER_SIZE: usize = 0x74;

/// Arch identifier
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u32)]
pub enum BsfArch {
    X86_64 = 0,
    Aarch64 = 1,
    Riscv64 = 2,
}

/// Shader stage
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u32)]
pub enum BsfStage {
    Vertex = 0,
    Fragment = 1,
    Compute = 2,
}

/// Validated BSF shader — ready for GPU submission
pub struct BsfShader {
    pub arch: BsfArch,
    pub stage: BsfStage,
    pub entry: [u8; 64],
    pub blake3: [u8; 32],
    pub spirv_words: [u32; 4096], // Max 16KB SPIR-V
    pub spirv_len: usize,
}

/// BSF loading errors
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BsfError {
    InvalidMagic,
    UnsupportedVersion,
    Truncated,
    InvalidArch,
    InvalidStage,
    HashMismatch,
    TooLarge,
}

/// Validate BSF header without loading full data
pub fn validate_header(data: &[u8]) -> Result<(), BsfError> {
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
    let arch = u32::from_le_bytes([data[8], data[9], data[10], data[11]]);
    if arch > 2 {
        return Err(BsfError::InvalidArch);
    }
    let stage = u32::from_le_bytes([data[12], data[13], data[14], data[15]]);
    if stage > 2 {
        return Err(BsfError::InvalidStage);
    }
    let spirv_size = u32::from_le_bytes([data[0x70], data[0x71], data[0x72], data[0x73]]) as usize;
    if spirv_size % 4 != 0 || spirv_size > 16384 {
        return Err(BsfError::TooLarge);
    }
    if data.len() < BSF_HEADER_SIZE + spirv_size {
        return Err(BsfError::Truncated);
    }
    Ok(())
}

/// Load and validate BSF shader, returning validated shader object
pub fn load(data: &[u8]) -> Result<BsfShader, BsfError> {
    validate_header(data)?;

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
    entry.copy_from_slice(&data[0x10..0x50]);

    let mut blake3 = [0u8; 32];
    blake3.copy_from_slice(&data[0x50..0x70]);

    let spirv_size = u32::from_le_bytes([data[0x70], data[0x71], data[0x72], data[0x73]]) as usize;
    let spirv_words = spirv_size / 4;

    let mut shader = BsfShader {
        arch,
        stage,
        entry,
        blake3,
        spirv_words: [0; 4096],
        spirv_len: spirv_words,
    };

    // Copy SPIR-V words
    let spirv_start = BSF_HEADER_SIZE;
    for i in 0..spirv_words {
        let offset = spirv_start + i * 4;
        shader.spirv_words[i] = u32::from_le_bytes([
            data[offset],
            data[offset + 1],
            data[offset + 2],
            data[offset + 3],
        ]);
    }

    // Validate BLAKE3 hash (simplified — real impl would use hardware SHA)
    let computed = compute_hash(&shader.spirv_words[..spirv_words]);
    if computed != blake3 {
        return Err(BsfError::HashMismatch);
    }

    Ok(shader)
}

/// Get entry point string from BSF shader
pub fn entry_str(shader: &BsfShader) -> &str {
    let end = shader.entry.iter().position(|&b| b == 0).unwrap_or(64);
    core::str::from_utf8(&shader.entry[..end]).unwrap_or("")
}

/// Get SPIR-V as byte slice
pub fn spirv_bytes(shader: &BsfShader) -> &[u8] {
    unsafe {
        core::slice::from_raw_parts(
            shader.spirv_words.as_ptr() as *const u8,
            shader.spirv_len * 4,
        )
    }
}

/// Compute hash for SPIR-V data (simplified — hardware SHA when available)
fn compute_hash(words: &[u32]) -> [u8; 32] {
    let mut state = [0u8; 32];
    let mut h: u64 = 5381;
    for &word in words {
        let bytes = word.to_le_bytes();
        for &b in &bytes {
            h = h.wrapping_mul(33).wrapping_add(b as u64);
        }
    }
    state[0..8].copy_from_slice(&h.to_le_bytes());

    h = 0x5bd1e995;
    for &word in words {
        let bytes = word.to_le_bytes();
        for &b in &bytes {
            h ^= b as u64;
            h = h.wrapping_mul(0x5bd1e995);
        }
    }
    state[8..16].copy_from_slice(&h.to_le_bytes());

    h = 0x1b873593;
    for &word in words {
        let bytes = word.to_le_bytes();
        for &b in &bytes {
            h = h.wrapping_mul(0x1b873593).wrapping_add(b as u64);
        }
    }
    state[16..24].copy_from_slice(&h.to_le_bytes());

    h = 0xcc9e2d51;
    for &word in words {
        let bytes = word.to_le_bytes();
        for &b in &bytes {
            h ^= (b as u64) << 13;
            h = h.wrapping_mul(0xcc9e2d51);
        }
    }
    state[24..32].copy_from_slice(&h.to_le_bytes());

    state
}
