//! ABI definitions for nexo Ring 3 runtime

/// BMO handle type for shader objects
#[repr(transparent)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ShaderHandle(pub u64);

impl ShaderHandle {
    pub const NULL: Self = Self(0);

    pub fn is_null(self) -> bool {
        self.0 == 0
    }
}

/// GPU pipeline stage identifiers
#[repr(u32)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PipelineStage {
    Vertex = 0,
    Fragment = 1,
    Compute = 2,
    Geometry = 3,
    TessControl = 4,
    TessEval = 5,
}

/// Shader compilation request from Ring 3
#[repr(C)]
pub struct ShaderCompileRequest {
    pub source_ptr: *const u8,
    pub source_len: u32,
    pub stage: PipelineStage,
    pub entry_ptr: *const u8,
    pub entry_len: u32,
}

/// Shader compilation result
#[repr(C)]
pub struct ShaderCompileResult {
    pub handle: ShaderHandle,
    pub spirv_ptr: *const u8,
    pub spirv_len: u32,
    pub entry_ptr: *const u8,
    pub entry_len: u32,
    pub blake3: [u8; 32],
}
