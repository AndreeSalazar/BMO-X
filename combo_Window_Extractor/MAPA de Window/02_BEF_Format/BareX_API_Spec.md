# BareX API Specification v1.0

**Capa:** L3 (API pública gráfica de FastOS)
**Hardware Target:** NVIDIA RTX 3060 (GA106, Ampere SM 8.6) + AMD Ryzen 5 5600X
**Filosofía:** Hereda TODO lo moderno de DirectX 12 Ultimate + Agility SDK 1.614+, destilado a Rust nativo, ABI estable, cero COM, cero HRESULT, cero WDDM.

> **Objetivo cero-overhead:** Recuperar el 15–20% que Linux/Proton pierde por traducción API → Vulkan → driver → kernel. BareX habla **directo** al GSP de la 3060 vía BMO Graphics Layer (L1).

---

## 1. Principios de diseño

1. **Bindless por defecto.** No hay `DescriptorTable` heredado; todos los recursos viven en un único `bx_global_heap` indexado desde shader (modelo SM 6.6 ResourceDescriptorHeap).
2. **Enhanced Barriers obligatorios.** Sin `D3D12_RESOURCE_STATES` legacy. Solo `bx_barrier { sync, access, layout }` (modelo Agility SDK).
3. **Work Graphs nativos.** GPU-driven scheduling sin command lists para pipelines complejos (heredado de DX12 Work Graphs 1.0).
4. **Sin "feature levels".** GA106 es el baseline. Todas las capacidades asumidas: DXR 1.1, VRS Tier 2, Mesh Shaders, Sampler Feedback, Wave Ops, 16-bit types, Resource Heap Tier 2.
5. **DirectStorage integrado.** No es opcional ni una API aparte — `bx_io` es parte del core, con descompresión GDeflate en GPU.
6. **Shaders pre-compilados a SASS.** Ningún juego compila en runtime. El BEF transporta SASS GA106 nativo (cero stutter).
7. **Un solo display, un solo adapter.** Sin DXGI factory, sin enumeración. `bx_device::primary()` y listo.

---

## 2. Objetos núcleo (12)

| # | Objeto BareX | Equivalente DX12 | Diferencia clave |
|---|---|---|---|
| 1 | `bx_device` | `ID3D12Device14` | Singleton, sin adapter enum |
| 2 | `bx_queue` | `ID3D12CommandQueue` | Tipos: `Graphics`, `Compute`, `Copy`, `VideoDecode` |
| 3 | `bx_cmdlist` | `ID3D12GraphicsCommandList10` | Reseteable sin allocator separado |
| 4 | `bx_pso` | `ID3D12PipelineState` + `StateObject` | Unificado (graphics, compute, raytracing, mesh, work-graph) |
| 5 | `bx_root_sig` | `ID3D12RootSignature` | Implícito por reflexión SPIR-V; opcional explícito |
| 6 | `bx_global_heap` | `ResourceDescriptorHeap` SM 6.6 | Único, 1M descriptores, bindless |
| 7 | `bx_fence` | `ID3D12Fence1` | Timeline semaphores nativos |
| 8 | `bx_swapchain` | `IDXGISwapChain4` | Directo a framebuffer GSP, sin DXGI |
| 9 | `bx_buffer` | `ID3D12Resource` (buffer) | GPU Upload Heaps por defecto (ReBAR) |
| 10 | `bx_texture` | `ID3D12Resource` (tex) | Sampler Feedback + tiled resources Tier 2 |
| 11 | `bx_sampler` | `D3D12_SAMPLER_DESC` | Estático en root sig por defecto |
| 12 | `bx_query_heap` | `ID3D12QueryHeap` | Timestamps, occlusion, pipeline stats, predication |

---

## 3. ABI dual: Rust nativo + C estable

### 3.1 Rust idiomático (uso interno y apps Rust)

```rust
use barex::*;

let dev = bx_device::primary()?;
let queue = dev.create_queue(QueueType::Graphics)?;

let pso = dev.create_pso(&PsoDesc::Graphics {
    vs: include_bef_shader!("triangle.vs.bef"),
    ps: include_bef_shader!("triangle.ps.bef"),
    rt_formats: &[Format::Rgba8UnormSrgb],
    depth_format: Format::D32Float,
    ..Default::default()
})?;

let mut cl = dev.create_cmdlist()?;
cl.begin();
cl.barrier(&[Barrier::texture(&backbuffer, Sync::None, Sync::RenderTarget,
                                Access::None, Access::RenderTargetWrite,
                                Layout::Undefined, Layout::RenderTarget)]);
cl.set_render_targets(&[&backbuffer.rtv()], None);
cl.set_pso(&pso);
cl.draw(3, 1, 0, 0);
cl.end();

queue.submit(&[&cl]);
swapchain.present(VSync::On)?;
```

### 3.2 C ABI plana (FFI, lenguajes no-Rust, compat shim)

```c
typedef struct bx_device bx_device;
typedef uint64_t bx_handle;

bx_result bx_device_primary(bx_device** out);
bx_result bx_device_create_queue(bx_device*, bx_queue_type, bx_handle* out);
bx_result bx_cmdlist_draw(bx_handle cl, uint32_t vtx, uint32_t inst,
                          uint32_t vtx_off, uint32_t inst_off);
/* ... ~180 funciones totales ... */
```

**Garantía:** ABI C congelada en v1.0. Rompimientos solo en versiones mayores.

---

## 4. Características heredadas de DX12 Ultimate / Agility SDK

| Feature DX12 | Estado en BareX | Notas |
|---|---|---|
| **DirectX Raytracing 1.1** | ✅ Core | `bx_blas`, `bx_tlas`, `RayQuery`, inline RT en cualquier shader |
| **DirectX Raytracing 1.2** (Opacity Micromaps, Shader Execution Reordering) | ✅ Core | Hardware GA106 lo soporta vía RT Cores Gen2 |
| **Variable Rate Shading Tier 2** | ✅ Core | Per-draw + per-primitive + screen-space image |
| **Mesh Shaders** | ✅ Core | `MS` + `AS` (amplification) reemplazan VS/HS/DS/GS legacy |
| **Sampler Feedback** | ✅ Core | Streaming de texturas tier 0.9 |
| **Work Graphs 1.0** | ✅ Core | GPU dispatch self-recursive, sin CPU roundtrip |
| **Enhanced Barriers** | ✅ **Único modo** | Sin barriers legacy en absoluto |
| **GPU Upload Heaps** (ReBAR) | ✅ Core | Heap default; Ryzen 5600X soporta ReBAR completo |
| **Independent Front/Back Stencil Refs** | ✅ Core | |
| **Triangle Fan Topology** | ✅ Core | |
| **Dynamic Depth Bias** | ✅ Core | |
| **MSAA 64KB Aligned Textures** | ✅ Core | |
| **Relaxed Format Casting** | ✅ Core | |
| **GPU Wait on CPU Fence** | ✅ Core | Timeline semaphores nativos |
| **DirectSR** (Super Resolution) | ✅ Wrapper | Backend: DLSS 2.x (la 3060 soporta DLSS Super Resolution + Reflex; **NO** Frame Generation que es Ada-only) |
| **DirectML** | ✅ Opcional | Wrapper sobre Tensor Cores Gen3 |
| **Video Encode/Decode** | ✅ Core | NVENC/NVDEC GA106 directo (H.264, HEVC, AV1 decode) |
| **HLSL SM 6.8** | ✅ Baseline | Wave matrix, Work Graphs, expanded comparison |
| **Shader Model 6.9 preview** | 🟡 Roadmap | Cooperative vectors |

---

## 5. DirectStorage nativo (`bx_io`)

```rust
let storage = bx_io::open()?;
let req = StorageRequest {
    file: asset_pak,
    offset_in_file: chunk.offset,
    size: chunk.size,
    destination: Destination::Texture {
        tex: &world_atlas,
        subresource: 0,
        region: chunk.region,
    },
    decompression: Decompression::GDeflate, // descomprime en GPU
};
storage.enqueue(&req)?;
storage.submit_and_signal(&fence, target_value)?;
```

**Path bare-metal:**
```diagram
NVMe SSD ──PCIe P2P──▶ VRAM RTX 3060 ──Compute Shader GDeflate──▶ Texture
   │           │              │
   └─ NVMe driver FastOS (NVMe_Driver_Spec)
                                          ▲
                                          └─ Cero copia por RAM del sistema
```
Ventaja vs Windows DirectStorage: sin pasar por NTFS, sin filtro WDDM, sin copia intermedia.

---

## 6. Compositor y swapchain

`bx_swapchain` no usa DXGI. Se conecta directo al **FastOS Window Compositor** (`05_UserSpace/FastOS_Window_Compositor.md`) que ya tiene acceso al framebuffer GSP. Tres modos:

- `Exclusive` — fullscreen, scanout directo, latencia mínima (~1 frame).
- `Composited` — el compositor hace blit con su propio shader (overhead < 0.2 ms).
- `Borderless` — composited pero con flip-model si la app es la única visible.

---

## 7. Comparación de overhead (proyección)

| Path | Overhead estimado | Notas |
|---|---|---|
| **DX12 nativo en Windows** | baseline (1.00x) | Driver NVIDIA + WDDM + DXGI |
| **DXVK / VKD3D-Proton sobre Linux** | 1.15–1.20x | Traducción + Vulkan + DRM/KMS |
| **BareX nativo en FastOS** | **0.92–0.95x** (objetivo) | Cero traducción, GSP directo, sin scheduler WDDM |

Las palancas para superar a Windows: (a) shaders pre-traducidos a SASS en BEF, (b) sin scheduler de driver de modo usuario, (c) DirectStorage sin filesystem legacy, (d) sin overhead de COM.

---

## 8. Tabla de funciones (resumen de superficie)

```
bx_device_*        : 12 funciones
bx_queue_*         :  6 funciones
bx_cmdlist_*       : ~80 funciones (draw, dispatch, barrier, copy, RT, mesh, work graph)
bx_pso_*           :  4 funciones
bx_heap_*          :  6 funciones
bx_resource_*      : 14 funciones (buffer + texture)
bx_fence_*         :  6 funciones
bx_swapchain_*     :  8 funciones
bx_io_*            : 10 funciones (DirectStorage)
bx_query_*         :  4 funciones
bx_video_*         : 12 funciones (encode/decode)
bx_dlss_*          :  6 funciones (DirectSR wrapper)
─────────────────────────────────────────
TOTAL              : ~168 funciones C ABI
```
Comparado con DX12 + DXGI + D3DX (~600 funciones COM), BareX es ~3.5x más pequeña en superficie.

---

## 9. Versionado

- **v1.0** — Esta spec. Congelada para SM 6.8 / Agility 1.614.
- **v1.x** — Adiciones compatibles (nuevos tipos, extensions opt-in).
- **v2.0** — Solo si hay cambio de hardware target (ej. RTX 50xx).

---

## 10. Archivos relacionados

- L1: `BMO_Graphics_Layer_Spec.md` (NVIDIA GSP)
- L2: `BareX_Shader_Pipeline.md` (HLSL/DXBC/DXIL → SPIR-V → SASS)
- L4: `BareX_Compat_Shim_Spec.md` (correr binarios DX11/DX12 de Windows)
- Mapeo: `DX12_to_BareX_Mapping.md`
- BEF: `BEF_Executable_Format_Spec.md` (transporta shaders SASS)
