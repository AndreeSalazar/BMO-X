//! `barex::graphics` — L3 API gráfica nativa.
//!
//! Spec: `BareX_API_Spec.md`. Hereda DX12 Ultimate + Agility 1.614, con
//! Bindless puro, Enhanced Barriers como único modo, y cero COM.
//!
//! ## Filosofía
//!
//! Estos módulos **NO implementan** un driver gráfico — eso lo hace
//! `drivers::gpu::fastgpu` (bridge BMO/GSP del usuario, intocable) y
//! futuras integraciones con NAGA. Aquí sólo viven las **firmas BMO ABI**
//! de los 12 objetos núcleo, para que apps Ring 3 sepan a qué llamar.
//!
//! ## Estructura modular (Sesión 13) — una carpeta por objeto, sin monolitos
//!
//! ```
//!   graphics/
//!   ├── mod.rs       ← este archivo (re-exports)
//!   ├── types/       ← Format, MemoryHint, Sync/Access/Layout, BxBarrier
//!   ├── device/      ← (1)  BxDevice singleton
//!   ├── queue/       ← (2)  BxQueue + QueueKind
//!   ├── cmdlist/     ← (3)  BxCmdList
//!   ├── pso/         ← (4)  BxPso
//!   ├── rootsig/     ← (5)  BxRootSig
//!   ├── heap/        ← (6)  BxGlobalHeap (bindless SM 6.6)
//!   ├── fence/       ← (7)  BxFence (timeline)
//!   ├── swapchain/   ← (8)  BxSwapchain (compositor FastOS, sin DXGI)
//!   ├── buffer/      ← (9)  BxBuffer
//!   ├── texture/     ← (10) BxTexture
//!   ├── sampler/     ← (11) BxSampler
//!   └── queryheap/   ← (12) BxQueryHeap
//! ```

#![allow(dead_code)]

pub mod types;
pub mod device;
pub mod queue;
pub mod cmdlist;
pub mod pso;
pub mod rootsig;
pub mod heap;
pub mod fence;
pub mod swapchain;
pub mod buffer;
pub mod texture;
pub mod sampler;
pub mod queryheap;

// ─── Re-exports planos ───────────────────────────────────────────────
pub use types::{Format, MemoryHint, BxBarrier, Sync, Access, Layout};
pub use device::BxDevice;
pub use queue::{BxQueue, QueueKind};
pub use cmdlist::BxCmdList;
pub use pso::BxPso;
pub use rootsig::BxRootSig;
pub use heap::BxGlobalHeap;
pub use fence::BxFence;
pub use swapchain::BxSwapchain;
pub use buffer::BxBuffer;
pub use texture::BxTexture;
pub use sampler::BxSampler;
pub use queryheap::BxQueryHeap;
