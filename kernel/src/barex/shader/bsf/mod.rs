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

    // Build a contiguous SPIR-V byte buffer for hashing.
    // The shader is stored as words internally for fast decode, but the
    // BLAKE3 check operates on the on-wire byte stream.
    let mut spirv_bytes_data = [0u8; 16384];
    let spirv_bytes_len = spirv_words * 4;
    for i in 0..spirv_words {
        let w = u32::from_le_bytes([
            data[BSF_HEADER_SIZE + i * 4],
            data[BSF_HEADER_SIZE + i * 4 + 1],
            data[BSF_HEADER_SIZE + i * 4 + 2],
            data[BSF_HEADER_SIZE + i * 4 + 3],
        ]);
        let wb = w.to_le_bytes();
        spirv_bytes_data[i * 4..i * 4 + 4].copy_from_slice(&wb);
        shader.spirv_words[i] = w;
    }

    // Validate BLAKE3 hash of the SPIR-V bytecode.
    // Real BLAKE3 (from bef::blake3) — spec 20211102, single-pass, no_std.
    let computed = crate::bef::blake3::hash(&spirv_bytes_data[..spirv_bytes_len]);
    if computed != blake3 {
        crate::diag::warn("bsf", "BLAKE3 mismatch — shader payload tampered or wrong hash");
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

/// Compute BLAKE3 hash for SPIR-V data — re-exported from bef::blake3.
///
/// Previously this function implemented a fake hash (DJB2 + variants) that
/// was flagged as "simplified — real impl would use hardware SHA" in the
/// code. The fake has been replaced with the real no_std BLAKE3
/// implementation (spec 20211102) available at `crate::bef::blake3`.
///
/// This re-export exists for source compatibility with any external code
/// that imported the old function name.
pub fn compute_hash(words: &[u32]) -> [u8; 32] {
    let mut buf = [0u8; 16384];
    let mut len = 0;
    for &w in words {
        if len + 4 > buf.len() { break; }
        let wb = w.to_le_bytes();
        buf[len..len + 4].copy_from_slice(&wb);
        len += 4;
    }
    crate::bef::blake3::hash(&buf[..len])
}
