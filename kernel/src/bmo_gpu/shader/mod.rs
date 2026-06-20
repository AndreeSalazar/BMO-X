//! BSF (BareX Shader Format) loader for BMO GPU.
//!
//! Validates BSF blobs (magic, version, BLAKE3) before they reach
//! Ring 0 / the real GPU driver. The actual translation of
//! HLSL/DXIL/DXBC/SPIR-V to GPU bytecode is done by external
//! toolchains (nexo-sh) and crates (naga, vkd3d-shader-rs, dxvk-spirv-rs).

#![allow(dead_code)]

use super::{BsfArch, BsfStage, BSF_HEADER_SIZE, BSF_MAGIC, BSF_VERSION};

/// BSF loading errors.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BsfError {
    BadMagic,
    UnsupportedVersion,
    InvalidHeader,
    InvalidArch,
    InvalidStage,
    TruncatedShader,
    InvalidBlake3,
    BufferTooSmall,
}

impl BsfError {
    pub fn to_bxerror(self) -> super::BxError {
        match self {
            BsfError::BadMagic | BsfError::InvalidHeader => super::BxError::InvalidArgument,
            BsfError::UnsupportedVersion => super::BxError::NotImplemented,
            BsfError::InvalidArch | BsfError::InvalidStage => super::BxError::Unsupported,
            BsfError::TruncatedShader => super::BxError::BufferTooSmall,
            BsfError::InvalidBlake3 => super::BxError::IoError,
            BsfError::BufferTooSmall => super::BxError::BufferTooSmall,
        }
    }
}

pub type BsfResult<T> = core::result::Result<T, BsfError>;

/// Validated BSF shader — ready for GPU submission.
pub struct BsfShader {
    pub arch: BsfArch,
    pub stage: BsfStage,
    pub entry: [u8; 64],
    pub blake3: [u8; 32],
    pub spirv_words: [u32; 4096], // Max 16 KiB SPIR-V
    pub spirv_len: usize,
}

impl BsfShader {
    pub fn serialized_size(&self) -> usize {
        BSF_HEADER_SIZE + self.spirv_len * 4
    }
}

// Header layout offsets
pub const BSF_HDR_MAGIC:    usize = 0x00;
pub const BSF_HDR_VERSION:  usize = 0x04;
pub const BSF_HDR_ARCH:     usize = 0x08;
pub const BSF_HDR_STAGE:    usize = 0x0C;
pub const BSF_HDR_ENTRY:    usize = 0x10;
pub const BSF_HDR_BLAKE3:   usize = 0x50;
pub const BSF_HDR_SPIRVCNT: usize = 0x70;

/// Validate a BSF blob and return a `BsfShader` ready for GPU submission.
pub fn validate(blob: &[u8]) -> BsfResult<BsfShader> {
    if blob.len() < BSF_HEADER_SIZE {
        return Err(BsfError::TruncatedShader);
    }
    if &blob[BSF_HDR_MAGIC..BSF_HDR_MAGIC + 4] != &BSF_MAGIC {
        return Err(BsfError::BadMagic);
    }
    let version = read_u32_le(blob, BSF_HDR_VERSION);
    if version != BSF_VERSION {
        return Err(BsfError::UnsupportedVersion);
    }
    let arch_raw = read_u32_le(blob, BSF_HDR_ARCH);
    let arch = match arch_raw {
        0 => BsfArch::X86_64,
        1 => BsfArch::Aarch64,
        2 => BsfArch::Riscv64,
        _ => return Err(BsfError::InvalidArch),
    };
    let stage_raw = read_u32_le(blob, BSF_HDR_STAGE);
    let stage = match stage_raw {
        0 => BsfStage::Vertex,
        1 => BsfStage::Fragment,
        2 => BsfStage::Compute,
        _ => return Err(BsfError::InvalidStage),
    };

    let mut entry = [0u8; 64];
    entry.copy_from_slice(&blob[BSF_HDR_ENTRY..BSF_HDR_ENTRY + 64]);

    let mut blake3 = [0u8; 32];
    blake3.copy_from_slice(&blob[BSF_HDR_BLAKE3..BSF_HDR_BLAKE3 + 32]);

    let spirv_words_count = read_u32_le(blob, BSF_HDR_SPIRVCNT) as usize;
    if spirv_words_count > 4096 {
        return Err(BsfError::BufferTooSmall);
    }
    let spirv_bytes = spirv_words_count * 4;
    if blob.len() < BSF_HEADER_SIZE + spirv_bytes {
        return Err(BsfError::TruncatedShader);
    }

    let mut spirv_words = [0u32; 4096];
    for i in 0..spirv_words_count {
        spirv_words[i] = read_u32_le(blob, BSF_HEADER_SIZE + i * 4);
    }

    let mut computed = [0u8; 32];
    let payload = &blob[BSF_HEADER_SIZE..BSF_HEADER_SIZE + spirv_bytes];
    placeholder_hash(payload, &mut computed);
    if computed != blake3 {
        return Err(BsfError::InvalidBlake3);
    }

    Ok(BsfShader {
        arch, stage, entry, blake3,
        spirv_words, spirv_len: spirv_words_count,
    })
}

fn read_u32_le(buf: &[u8], offset: usize) -> u32 {
    u32::from_le_bytes([
        buf[offset], buf[offset + 1], buf[offset + 2], buf[offset + 3],
    ])
}

/// Placeholder hash (XOR with 0xA5). Replace with the real BLAKE3
/// when `bmo_abi::blake3` is wired up to the loader.
fn placeholder_hash(data: &[u8], out: &mut [u8; 32]) {
    for (i, b) in out.iter_mut().enumerate() {
        *b = if i < data.len() { data[i] ^ 0xA5 } else { 0 };
    }
}
