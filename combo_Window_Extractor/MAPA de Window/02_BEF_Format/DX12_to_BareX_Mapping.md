# DirectX 12 Ultimate / Agility SDK → BareX Mapping

**Versión de referencia:** DirectX 12 Agility SDK 1.614 + DXR 1.2 + Work Graphs 1.0 + DirectSR
**Fecha base:** Mayo 2026
**Propósito:** Tabla 1-a-1 entre cada concepto DirectX moderno y su equivalente BareX, indicando qué se renombra, qué se fusiona, qué se elimina y qué se añade.

---

## 1. Objetos núcleo

| DirectX 12 | BareX | Acción | Justificación |
|---|---|---|---|
| `IDXGIFactory7` | — | **Eliminado** | No hay enumeración multi-adapter. Hardware fijo. |
| `IDXGIAdapter4` | — | **Eliminado** | Idem. |
| `IDXGIOutput6` | `bx_display` | Renombrado y simplificado | Un solo display. |
| `IDXGISwapChain4` | `bx_swapchain` | Renombrado, sin DXGI | Conectado al compositor FastOS. |
| `ID3D12Device14` | `bx_device` | Renombrado, sin niveles de feature | GA106 es baseline. |
| `ID3D12CommandQueue` | `bx_queue` | Renombrado | Idem semantics. |
| `ID3D12CommandAllocator` | — | **Fusionado en `bx_cmdlist`** | Allocator interno automático. |
| `ID3D12GraphicsCommandList10` | `bx_cmdlist` | Renombrado, métodos consolidados | |
| `ID3D12PipelineState` + `ID3D12StateObject` | `bx_pso` | **Fusionados** | Un solo objeto para gráficos/compute/RT/mesh/work-graph. |
| `ID3D12RootSignature` | `bx_root_sig` (opcional) | Implícito por reflexión SPIR-V | Solo explícito si el dev quiere control. |
| `ID3D12DescriptorHeap` | `bx_global_heap` (singleton) | **Fusionados** en uno solo | Modelo SM 6.6 ResourceDescriptorHeap puro. |
| `ID3D12Resource2` | `bx_buffer` / `bx_texture` | Separados por tipo | Más claro y type-safe. |
| `ID3D12Heap1` | — | **Eliminado** | Allocator BareX automático con hints. |
| `ID3D12Fence1` | `bx_fence` | Renombrado | Timeline semaphore puro. |
| `ID3D12QueryHeap` | `bx_query_heap` | Renombrado | |
| `ID3D12CommandSignature` | `bx_indirect_sig` | Renombrado | Indirect draws/dispatch. |
| `ID3D12ProtectedResourceSession` | — | **Eliminado** | DRM/PlayReady no soportado. |
| `ID3D12VideoDevice3` | `bx_video_device` | Simplificado | NVENC/NVDEC GA106. |
| `ID3D12LifetimeTracker` | — | **Eliminado** | Rust ownership lo hace gratis. |

---

## 2. Barriers

| DX12 legacy | DX12 Enhanced | BareX |
|---|---|---|
| `D3D12_RESOURCE_STATES` | `D3D12_BARRIER_*` (sync/access/layout) | **`bx_barrier`** (único modo, idéntico a Enhanced) |

```rust
// Solo este modo existe en BareX:
bx_barrier::texture(
    &tex,
    Sync::ComputeShading, Sync::PixelShading,
    Access::UnorderedAccess, Access::ShaderResource,
    Layout::UnorderedAccess, Layout::ShaderResource,
);
```

Los barriers legacy NO existen. Esto fuerza buenas prácticas y elimina ~30% del código de validación.

---

## 3. Pipelines y shaders

| DX12 | BareX |
|---|---|
| `D3D12_GRAPHICS_PIPELINE_STATE_DESC` | `PsoDesc::Graphics { ... }` |
| `D3D12_COMPUTE_PIPELINE_STATE_DESC` | `PsoDesc::Compute { ... }` |
| `CD3DX12_PIPELINE_STATE_STREAM` | (no necesario; struct nativo) |
| Mesh shader pipeline (PSO stream) | `PsoDesc::Mesh { as: ?, ms, ps }` |
| `D3D12_STATE_OBJECT_DESC` (RT) | `PsoDesc::Raytracing { libs, hit_groups, max_recursion, ... }` |
| `D3D12_WORK_GRAPH_DESC` | `PsoDesc::WorkGraph { nodes, entry }` |
| HLSL Shader Model 6.8 | ✅ Soportado (frontend `dxc`) |
| DXIL bytecode | ✅ Aceptado (traducido por `vkd3d-shader-rs`) |
| DXBC bytecode | ✅ Aceptado (traducido por `dxvk-spirv-rs`) |
| Wave Intrinsics | ✅ Mapeados a equivalentes SASS |
| 16-bit types (`half`, `int16`) | ✅ Nativo |
| Sampler Feedback | ✅ `bx_sampler_feedback` |

---

## 4. Raytracing (DXR 1.1 + 1.2)

| DXR | BareX |
|---|---|
| `BuildRaytracingAccelerationStructure` | `bx_cmdlist::build_blas / build_tlas` |
| `RayQuery<>` (inline RT) | ✅ Idéntico en HLSL, traducido vía DXIL |
| `DispatchRays` | `bx_cmdlist::dispatch_rays` |
| Shader Table (sbt) | `bx_sbt` (helper que la construye) |
| Opacity Micromap (OMM) — DXR 1.2 | `bx_omm` |
| Shader Execution Reordering (SER) — DXR 1.2 | ✅ Auto vía intrinsic `ReorderThread` |
| `D3D12_RAYTRACING_TIER` query | — (eliminado, asumimos Tier 1.2) |

---

## 5. Mesh / Amplification shaders

| DX12 | BareX |
|---|---|
| Amplification shader (AS) | `Stage::Amplification` |
| Mesh shader (MS) | `Stage::Mesh` |
| `DispatchMesh(x,y,z)` | `bx_cmdlist::dispatch_mesh(x,y,z)` |
| `EmitMeshTasksEXT` (Vulkan) | (no aplicable, abstracción nativa BareX) |

---

## 6. Variable Rate Shading

| DX12 | BareX |
|---|---|
| `RSSetShadingRate(rate, combiners)` | `bx_cmdlist::set_shading_rate(rate, combiners)` |
| `RSSetShadingRateImage(tex)` | `bx_cmdlist::set_shading_rate_image(&tex)` |
| Per-primitive VRS via SV_ShadingRate | ✅ |

---

## 7. Memory & Heaps

| DX12 | BareX |
|---|---|
| `D3D12_HEAP_TYPE_DEFAULT` | `MemoryHint::DeviceLocal` |
| `D3D12_HEAP_TYPE_UPLOAD` | `MemoryHint::Upload` (CPU-write, GPU-read) |
| `D3D12_HEAP_TYPE_READBACK` | `MemoryHint::Readback` |
| `D3D12_HEAP_TYPE_GPU_UPLOAD` (Agility) | **`MemoryHint::DeviceLocalUploadable`** (default cuando hay ReBAR) |
| Custom heaps + memory pool L0/L1 | `MemoryHint::Custom { ... }` |
| Reserved/tiled resources | `bx_buffer::reserved`, `bx_texture::tiled` |
| `D3D12_RESOURCE_FLAG_*` | `BufferUsage` / `TextureUsage` bitflags |

ReBAR (Resizable BAR) es **default** porque el Ryzen 5600X + chipset 500-series + RTX 3060 lo soporta.

---

## 8. DirectStorage

| DStorage 1.2 | BareX |
|---|---|
| `IDStorageFactory` | `bx_io::open()` |
| `IDStorageQueue2` | `bx_io_queue` |
| `IDStorageFile` | `bx_io_file` |
| `DSTORAGE_REQUEST` | `StorageRequest { ... }` |
| GDeflate GPU decompression | ✅ Compute shader BareX integrado |
| Memory destination | ✅ |
| Buffer destination | ✅ |
| Texture region destination | ✅ |
| Multi-frame queue | ✅ |

Path mejorado: **NVMe → PCIe P2P → VRAM** sin pasar por filesystem legacy.

---

## 9. DirectSR / DirectML / DLSS

| MS API | BareX | Backend GA106 |
|---|---|---|
| DirectSR (super resolution agnóstica) | `bx_dlss::upscale(...)` | DLSS 2.x SDK NVIDIA |
| DirectML (inferencia ML) | `bx_dml` | Tensor Cores Gen3 directo |
| Frame Generation (DLSS 3) | ❌ | **No disponible en GA106** (requiere Optical Flow Accelerator de Ada). Documentado como limitación de hardware. |
| Reflex Low Latency | `bx_reflex` | Soportado en Ampere |

---

## 10. Video

| DX12 Video | BareX |
|---|---|
| H.264 decode | `bx_video::decode_h264` |
| H.265/HEVC decode | `bx_video::decode_hevc` |
| AV1 decode | `bx_video::decode_av1` (GA106 lo soporta) |
| H.264 encode | `bx_video::encode_h264` (NVENC) |
| HEVC encode | `bx_video::encode_hevc` (NVENC) |
| AV1 encode | ❌ (Ada-only en NVIDIA) |
| Motion estimation | `bx_video::motion_estimate` |

---

## 11. APIs eliminadas (deuda histórica que NO se hereda)

| Eliminado | Razón |
|---|---|
| Feature levels 9.x / 10.x / 11.x | GA106 es el único target. |
| `ID3D12RootSignatureDeserializer` | Root sig implícito por defecto. |
| Legacy barriers | Solo Enhanced Barriers. |
| Multi-GPU (`ID3D12Device::CreateSharedHandle` cross-adapter) | Una sola GPU. |
| Tiled Resources Tier 1 (parcial) | GA106 es Tier 2 — solo eso se expone. |
| `D3D11On12` | Si necesitas DX11, el shim L4 se encarga. |
| `D3D12SDKLayers` debug layer | Sustituido por `barex-validator` (más rápido, en Rust). |
| `PIX` markers como API runtime | Sustituido por `barex-trace`. |
| Stream Output (DX10-era) | Reemplazado por compute shaders + UAV. |
| `D3D12_FEATURE_*` queries | Constantes hardcoded. |

---

## 12. APIs **añadidas** que no existen en DX12

| Nueva en BareX | Por qué |
|---|---|
| `bx_io` con GDeflate como ciudadano de primera clase | DirectStorage en Windows depende de NTFS/WDDM. Aquí es nativo. |
| `bx_compositor_link` | Diálogo directo con `FastOS_Window_Compositor`. |
| `bx_persistent_pso_cache` | Cache global del sistema, compartido entre apps con la misma firma de shader. |
| `bx_telemetry` (opt-in del dev) | Frame timing, GPU residency, sin overhead vía counters GSP nativos. |
| `bx_zero_copy_present` | Modo exclusivo con scanout directo desde el render target. |

---

## 13. Resumen ejecutivo

```
DX12 + DXGI + D3DCompile + DStorage + DirectML + Video12   ≈ 600 funciones COM
                              │
                              ▼   (destilación)
                          BareX v1.0   ≈ 168 funciones C ABI + Rust idiomático
                              │
                              ▼
                          - Sin COM
                          - Sin HRESULT
                          - Sin DXGI
                          - Sin feature levels
                          - Bindless puro
                          - Enhanced Barriers único
                          - DirectStorage core
                          - Pre-compilación SASS
                          - Cero overhead WDDM
```

---

## 14. Archivos relacionados

- `BareX_API_Spec.md`
- `BareX_Shader_Pipeline.md`
- `BareX_Compat_Shim_Spec.md`
- `BMO_Graphics_Layer_Spec.md`
- `BEF_Executable_Format_Spec.md`
