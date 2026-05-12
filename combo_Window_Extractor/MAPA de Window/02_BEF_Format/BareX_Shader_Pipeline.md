# BareX Shader Pipeline Specification v1.0

**Capa:** L2 (compilación y traducción de shaders)
**Hardware Target:** NVIDIA RTX 3060 (GA106, ISA SASS sm_86)
**Filosofía:** Un solo IR común (SPIR-V), un solo backend (NAK → SASS), cero compilación en runtime.

> **Reaprovecha frontends ya probados en miles de juegos** (DXVK, VKD3D-Proton, DXC, glslang) y reutiliza el backend NVK/NAK para emitir SASS GA106 nativo, identificado en `NVK_Shader_Pipeline_Analysis.md`.

---

## 1. Frontends soportados

| Lenguaje fuente | IR de entrada | Frontend reutilizado | Procedencia |
|---|---|---|---|
| **HLSL SM 6.8** | DXIL | `dxc` (DirectX Shader Compiler open-source MS) | Microsoft |
| **HLSL legacy SM ≤ 5.1** | DXBC | `dxvk-spirv` (DXBC→SPIR-V) | DXVK / Proton |
| **DXIL precompilado** | DXIL | `vkd3d-shader` (DXIL→SPIR-V) | VKD3D-Proton |
| **GLSL** | SPIR-V | `glslang` | Khronos |
| **WGSL** | SPIR-V | `naga` | wgpu |
| **Slang** | SPIR-V | `slangc` | NVIDIA |
| **Rust GPU** | SPIR-V | `rust-gpu` | Embark |
| **HLSL2021/HLSL202x** | DXIL | `dxc` | Microsoft |

Todos convergen en **SPIR-V 1.6** como IR canónico interno.

---

## 2. Pipeline de compilación

```diagram
   ┌───────────────────────────────────────────────────────┐
   │  TIEMPO DE BUILD (en máquina del desarrollador)       │
   │                                                        │
   │  HLSL ──▶ dxc ──▶ DXIL ──┐                            │
   │  HLSL5 ─▶ fxc ──▶ DXBC ──┼─▶ SPIR-V 1.6 ──▶ NAK ──▶ │
   │  GLSL ──▶ glslang ──────┘    (IR único)        │      │
   │                                                  ▼      │
   │                                         SASS GA106 bin │
   │                                                  │      │
   │                                                  ▼      │
   │                                    Empaquetado en .bef │
   │                                    (sección .shaders)  │
   └───────────────────────────────────────────────────────┘
                           │
                           ▼
   ┌───────────────────────────────────────────────────────┐
   │  TIEMPO DE EJECUCIÓN (RTX 3060 + FastOS)              │
   │                                                        │
   │  BEF loader ─▶ extrae SASS ─▶ GSP channel ─▶ GPU     │
   │                                                        │
   │  ⚠️  Cero compilación en runtime → cero stutter       │
   └───────────────────────────────────────────────────────┘
```

---

## 3. Herramienta `barexc` (compilador oficial)

```
barexc input.hlsl --target sm_86 --stage ps --output out.bef.shader
barexc input.dxbc --from dxbc --output out.bef.shader
barexc bundle/*.spv --bundle game.bef.pak
```

Flags clave:
- `--target sm_86` — fijo (GA106). `sm_89` reservado para futuro.
- `--stage {vs,ps,cs,ms,as,rgen,rmiss,rchit,rahit,rint,rcall,wg}`
- `--opt {0,1,2,3}` — pasa por NAK con O3 por defecto.
- `--debug` — incluye nombres y line info para `barex-trace`.
- `--validate` — corre validador SPIR-V + DXIL antes de bajar.

---

## 4. NAK (NVK Asm Kompiler) integrado

NAK es el backend SPIR-V → SASS de Mesa/NVK, escrito en Rust. BareX lo importa como crate:

```toml
[dependencies]
nak = { version = "0.6", features = ["sm_86", "raytracing", "mesh_shaders"] }
```

**Optimizaciones críticas habilitadas:**
- Register allocation con reasignación post-hoc (clave para GA106 que tiene 64K reg/SM).
- Scheduling de instrucciones para los 4 schedulers por SM.
- Coalesced memory access (LDG.E.128).
- Tensor Core intrinsics (HMMA, BMMA) para DirectML / DLSS.
- RT Core intrinsics (RTX, traceray opcodes nativos).

---

## 5. Reflexión y root signature implícita

A partir del SPIR-V resultante, BareX deriva automáticamente:
- **Bindings bindless** (descriptores indexados desde shader).
- **Push constants** → mapeados a UBO 0.
- **Sampler estáticos** detectados por uso constante.

Esto elimina ~80% del boilerplate de root signatures de DX12. El programador puede sobrescribir manualmente con `bx_root_sig::explicit(&desc)` si necesita control fino.

---

## 6. Traducción DXIL → SPIR-V (vía vkd3d-shader)

Reutilizamos `libvkd3d-shader` portado a Rust como crate `vkd3d-shader-rs`. Cobertura:

| Feature DXIL | Estado |
|---|---|
| Compute / Graphics shaders | ✅ |
| Mesh / Amplification shaders | ✅ |
| Raytracing (DXR 1.1) | ✅ |
| Wave intrinsics (SM 6.0+) | ✅ |
| 16-bit types | ✅ |
| Resource descriptor heap (SM 6.6) | ✅ |
| Work Graphs nodes | ✅ |
| Sampler Feedback | ✅ |
| Atomics 64-bit | ✅ |

Esto cubre **el 100% de juegos DX12 modernos** sin que el desarrollador toque nada — basta con que su `.dxil` viaje en el `.bef.shader`.

---

## 7. Cache de shaders persistente

Aunque la compilación es offline, hay un cache de fallback para casos extremos (Compat Shim L4 cuando el juego compila shaders en runtime vía D3DCompile):

- Ruta: `/system/shader_cache/{game_uuid}/{shader_hash}.sass`
- Formato: SASS crudo + metadata SPIR-V.
- Pre-warming: `barex-precompile <game.bef>` recorre todos los PSO declarados y genera SASS antes del primer launch.

---

## 8. Validación y debugging

- **`barex-trace`** — captura tipo PIX/RenderDoc, formato propietario `.bxtrace`.
- **`barex-validator`** — corre validador SPIR-V de Khronos + DXIL validator de MS.
- **GPU prints** — `printf` en HLSL SM 6.8 mapeado a buffer de salida BareX.

---

## 9. Métrica de éxito

| Métrica | Objetivo |
|---|---|
| Tiempo de compilación HLSL → SASS (1 shader medio) | < 80 ms |
| Stutter por compilación en runtime | **0 ms** (todo precompilado) |
| Cobertura de juegos DX12 modernos al traducir DXIL | ≥ 99% (paridad con VKD3D-Proton) |
| Cobertura de juegos DX11 al traducir DXBC | ≥ 99% (paridad con DXVK) |
| Overhead vs SASS escrito a mano | < 3% (mérito de NAK) |

---

## 10. Archivos relacionados

- `BareX_API_Spec.md` (L3, consume los `.bef.shader`)
- `BareX_Compat_Shim_Spec.md` (L4, usa runtime DXBC→SPIR-V para apps que compilen al vuelo)
- `BMO_Graphics_Layer_Spec.md` (L1, recibe SASS vía GSP channels)
- `BEF_Executable_Format_Spec.md` (sección `.shaders`)
- `NVK_Shader_Pipeline_Analysis.md` (análisis previo de NAK)
