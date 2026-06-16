//! BSF — BareX Shader Format
//!
//! Layout binario (sin dependencias, `repr(C)`):
//! ```text
//! Offset  Size   Field
//! 0x00    4      magic       "BSF\0"
//! 0x04    4      version     (1)
//! 0x08    4      arch        (x86_64=0, aarch64=1, riscv64=2)
//! 0x0C    4      stage       (vertex=0, fragment=1, compute=2)
//! 0x10    64     entry       (UTF-8 null-padded)
//! 0x50    32     blake3      (SHA-256 del SPIR-V)
//! 0x70    4      spirv_size  (bytes)
//! 0x74    N      spirv       (SPIR-V bytecode)
//! ```

pub const BSF_MAGIC: [u8; 4] = *b"BSF\0";
pub const BSF_VERSION: u32 = 1;
pub const BSF_HEADER_SIZE: usize = 0x74; // 116 bytes

#[repr(u32)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BsfArch {
    X86_64 = 0,
    Aarch64 = 1,
    Riscv64 = 2,
}

#[repr(u32)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BsfStage {
    Vertex = 0,
    Fragment = 1,
    Compute = 2,
}

pub struct BsfFile<'a> {
    pub arch: BsfArch,
    pub stage: naga::ShaderStage,
    pub entry: &'a str,
    pub blake3: [u8; 32],
    pub spirv: &'a [u8],
}

impl<'a> BsfFile<'a> {
    pub fn to_bytes(&self) -> Vec<u8> {
        let mut buf = Vec::with_capacity(BSF_HEADER_SIZE + self.spirv.len());

        // magic
        buf.extend_from_slice(&BSF_MAGIC);
        // version
        buf.extend_from_slice(&BSF_VERSION.to_le_bytes());
        // arch
        buf.extend_from_slice(&(self.arch as u32).to_le_bytes());
        // stage
        let stage_val = match self.stage {
            naga::ShaderStage::Vertex => BsfStage::Vertex,
            naga::ShaderStage::Fragment => BsfStage::Fragment,
            naga::ShaderStage::Compute => BsfStage::Compute,
            _ => BsfStage::Fragment, // default for unknown stages
        };
        buf.extend_from_slice(&(stage_val as u32).to_le_bytes());
        // entry (64 bytes, null-padded)
        let entry_bytes = self.entry.as_bytes();
        let entry_len = entry_bytes.len().min(63);
        buf.extend_from_slice(&entry_bytes[..entry_len]);
        buf.resize(BSF_HEADER_SIZE - 36, 0); // pad to entry end
        // blake3 (32 bytes)
        buf.extend_from_slice(&self.blake3);
        // spirv_size
        buf.extend_from_slice(&(self.spirv.len() as u32).to_le_bytes());
        // spirv data
        buf.extend_from_slice(self.spirv);

        buf
    }
}

pub fn validate_header(data: &[u8]) -> Result<BsfHeaderInfo, &'static str> {
    if data.len() < BSF_HEADER_SIZE {
        return Err("BSF too short");
    }
    if data[0..4] != BSF_MAGIC {
        return Err("invalid BSF magic");
    }
    let version = u32::from_le_bytes([data[4], data[5], data[6], data[7]]);
    if version != BSF_VERSION {
        return Err("unsupported BSF version");
    }
    let arch = u32::from_le_bytes([data[8], data[9], data[10], data[11]]);
    let stage = u32::from_le_bytes([data[12], data[13], data[14], data[15]]);

    let mut entry = [0u8; 64];
    let entry_end = 0x50;
    let entry_src = &data[0x10..entry_end];
    let entry_len = entry_src.iter().position(|&b| b == 0).unwrap_or(64);
    entry[..entry_len].copy_from_slice(&entry_src[..entry_len]);

    let mut blake3 = [0u8; 32];
    blake3.copy_from_slice(&data[0x50..0x70]);

    let spirv_size = u32::from_le_bytes([data[0x70], data[0x71], data[0x72], data[0x73]]) as usize;

    if data.len() < BSF_HEADER_SIZE + spirv_size {
        return Err("BSF truncated");
    }

    Ok(BsfHeaderInfo {
        version,
        arch,
        stage,
        entry,
        blake3,
        spirv_offset: BSF_HEADER_SIZE,
        spirv_size,
    })
}

pub struct BsfHeaderInfo {
    pub version: u32,
    pub arch: u32,
    pub stage: u32,
    pub entry: [u8; 64],
    pub blake3: [u8; 32],
    pub spirv_offset: usize,
    pub spirv_size: usize,
}
