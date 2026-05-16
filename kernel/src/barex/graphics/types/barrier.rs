//! Enhanced Barrier — único modo permitido (sin `D3D12_RESOURCE_STATES` legacy).

bitflags::bitflags! {
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub struct Sync: u32 {
        const NONE          = 0;
        const ALL           = 1 << 0;
        const DRAW          = 1 << 1;
        const COMPUTE       = 1 << 2;
        const COPY          = 1 << 3;
        const PIXEL_SHADING = 1 << 4;
        const RENDER_TARGET = 1 << 5;
        const RAYTRACING    = 1 << 6;
        const MESH_SHADING  = 1 << 7;
    }

    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub struct Access: u32 {
        const NONE              = 0;
        const VERTEX_BUFFER     = 1 << 0;
        const INDEX_BUFFER      = 1 << 1;
        const CONSTANT_BUFFER   = 1 << 2;
        const SHADER_RESOURCE   = 1 << 3;
        const UNORDERED_ACCESS  = 1 << 4;
        const RENDER_TARGET     = 1 << 5;
        const DEPTH_STENCIL     = 1 << 6;
        const COPY_SOURCE       = 1 << 7;
        const COPY_DEST         = 1 << 8;
        const RAYTRACING_AS     = 1 << 9;
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Layout {
    Undefined,
    Common,
    RenderTarget,
    DepthStencil,
    ShaderResource,
    UnorderedAccess,
    CopySource,
    CopyDest,
    Present,
}

#[derive(Debug, Clone, Copy)]
pub struct BxBarrier {
    pub sync_before: Sync,
    pub sync_after: Sync,
    pub access_before: Access,
    pub access_after: Access,
    pub layout_before: Layout,
    pub layout_after: Layout,
}
