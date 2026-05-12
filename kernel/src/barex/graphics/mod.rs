//! `barex::graphics` — L3 API gráfica nativa.
//!
//! Spec: `BareX_API_Spec.md`. Hereda DX12 Ultimate + Agility 1.614, con
//! Bindless puro, Enhanced Barriers como único modo, y cero COM.
//!
//! Esqueleto: define los 12 objetos núcleo como tipos opacos. La
//! implementación real sobre el GSP se conectará a `crate::drivers::gpu::fastgpu`
//! cuando el bridge BMO/GSP esté listo. Hasta entonces, todos los métodos
//! devuelven `BxError::NotImplemented`.

use crate::barex::{BxError, BxResult};

// ═══════════════════════════════════════════════════════════════════════
//   Objetos núcleo (12)
// ═══════════════════════════════════════════════════════════════════════

/// (1) Singleton: punto de entrada. Sustituye `ID3D12Device14` + DXGI.
pub struct BxDevice {
    _private: (),
}

/// (2) Cola de envío de comandos a la GPU.
pub struct BxQueue {
    pub kind: QueueKind,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum QueueKind { Graphics, Compute, Copy, VideoDecode, VideoEncode }

/// (3) Lista de comandos. Allocator interno (sin `ID3D12CommandAllocator`).
pub struct BxCmdList {
    _private: (),
}

/// (4) Pipeline State Object unificado (graphics/compute/RT/mesh/work-graph).
pub struct BxPso {
    _private: (),
}

/// (5) Root signature (opcional; default = derivado por reflexión SPIR-V).
pub struct BxRootSig {
    _private: (),
}

/// (6) Heap único bindless (modelo SM 6.6 `ResourceDescriptorHeap`).
pub struct BxGlobalHeap {
    pub capacity: u32,
}

/// (7) Fence/timeline semaphore.
pub struct BxFence {
    pub value: u64,
}

/// (8) Swapchain conectado al compositor FastOS (sin DXGI).
pub struct BxSwapchain {
    pub width: u32,
    pub height: u32,
    pub format: Format,
}

/// (9) Buffer (vertex/index/UBO/SSBO/raw).
pub struct BxBuffer {
    pub size_bytes: u64,
    pub hint: MemoryHint,
}

/// (10) Texture 1D/2D/3D/Cube/Array.
pub struct BxTexture {
    pub width: u32,
    pub height: u32,
    pub depth_or_array: u32,
    pub format: Format,
}

/// (11) Sampler.
pub struct BxSampler {
    _private: (),
}

/// (12) Query heap (timestamps, occlusion, pipeline stats).
pub struct BxQueryHeap {
    pub count: u32,
}

// ═══════════════════════════════════════════════════════════════════════
//   Tipos auxiliares (subset mínimo del spec)
// ═══════════════════════════════════════════════════════════════════════

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Format {
    Rgba8UnormSrgb,
    Bgra8UnormSrgb,
    Rgba16Float,
    Rgba32Float,
    D32Float,
    D24UnormS8Uint,
    Bc7UnormSrgb,
    Astc4x4UnormSrgb,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MemoryHint {
    /// VRAM exclusivo (heap default DX12).
    DeviceLocal,
    /// CPU-write, GPU-read (heap upload).
    Upload,
    /// CPU-read, GPU-write (heap readback).
    Readback,
    /// VRAM con mapping CPU vía ReBAR (Agility GPU Upload Heap).
    DeviceLocalUploadable,
}

/// Enhanced Barrier — único modo permitido (sin `D3D12_RESOURCE_STATES` legacy).
#[derive(Debug, Clone, Copy)]
pub struct BxBarrier {
    pub sync_before: Sync,
    pub sync_after: Sync,
    pub access_before: Access,
    pub access_after: Access,
    pub layout_before: Layout,
    pub layout_after: Layout,
}

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

// ═══════════════════════════════════════════════════════════════════════
//   API pública
// ═══════════════════════════════════════════════════════════════════════

impl BxDevice {
    /// Único punto de entrada — equivalente a `D3D12CreateDevice` pero sin
    /// adapter enum (target hardware fijo: GA106).
    pub fn primary() -> BxResult<Self> {
        // TODO: enlazar con `drivers::gpu::fastgpu` cuando el bridge esté listo.
        Err(BxError::NotImplemented)
    }

    pub fn create_queue(&self, _kind: QueueKind) -> BxResult<BxQueue> {
        Err(BxError::NotImplemented)
    }
}
