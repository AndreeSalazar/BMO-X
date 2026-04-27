# GSP-RM Implementation Guide for FastOS — RTX 3060 (GA106)

> Basado en análisis de: `nvidia-open-gpu-kernel-modules`, `nouveau` (Linux kernel),
> y datos extraídos por SigDead-BIB de `gsp_ga10x.bin`.

---

## Resumen Ejecutivo

**¿Pop!_OS nvidia-open es suficiente?** — **Sí, pero necesitas AMBAS fuentes:**

| Fuente | Repo | Qué te da |
|--------|------|-----------|
| **nvidia-open** | `github.com/NVIDIA/open-gpu-kernel-modules` | Estructuras exactas, RPC IDs, WPR layout, HAL dispatch, class headers |
| **nouveau** | `drivers/gpu/drm/nouveau/` en kernel Linux | Implementación limpia y legible del boot + RPC + display en C |
| **SigDead-BIB** | Tu proyecto | Validación: 13 ELFs, class IDs, RPC strings, dispatch tables |

---

## Lo que ya tienes vs lo que falta

### ✅ COMPLETO
- `nv_regs` — Registros BAR0 (PMC, PFIFO, PGRAPH, PDISPLAY, FALCON, etc.)
- `nv_hal` — MMIO, PCI config, DMA buffers, Platform trait
- `nv_error` — Códigos NV_ERR_*
- `nv_firmware` — ELF parser básico, FALCON engines, secciones GSP
- SigDead — 13 ELFs RISC-V, 47 class IDs, RPC header, dispatch tables

### ⚠️ INCORRECTO (necesita corrección)
1. **`GspRpcHeader`** — Tu struct tiene campos inventados (`msg_type`, `payload_size`). 
   El formato real es `rpc_message_header_v03_00` (ver abajo).
2. **`gsp_rpc::MSG_*`** — Tus IDs son inventados. Los reales son: ALLOC=103, CONTROL=76, FREE=10, etc.
3. **`falcon_load()`** — Carga firmware FALCON clásico (IMEM/DMEM). GSP Ampere usa RISC-V boot via SEC2.
4. **`GspBootParams`** — Struct inventada. La real es `GspFwWprMeta` (256 bytes, ver abajo).
5. **`GspCmdRing`** — Layout incorrecto. Son dos ring buffers separados (cmdq + msgq) con `GSP_MSG_QUEUE_ELEMENT`.

### ❌ FALTA IMPLEMENTAR
1. `GspFwWprMeta` — Layout de VRAM para firmware (256 bytes)
2. Radix3 page table — 3 niveles para cargar ELF en VRAM
3. SEC2 authenticated boot — FWSEC + Booter Load
4. LibOS init arguments — 4 memory regions (LOGINIT, LOGINTR, LOGRM, RMARGS)
5. Message queue — cmdq + msgq ring buffers con checksum
6. RPC protocol completo — GSP_MSG_QUEUE_ELEMENT + nvfw_gsp_rpc
7. Display via GSP — Todo modeset va por RPC, NO por MMIO directo
8. FIFO channels via GSP — Channels se crean por RPC (GSP_RM_ALLOC)
9. BAR2 virtual window — Page table en VRAM

---

## Estructuras Correctas (de nvidia-open + nouveau)

### 1. GspFwWprMeta (256 bytes) — Layout de firmware en VRAM

```rust
/// Fuente: nvidia-open/src/nvidia/arch/nvalloc/common/inc/gsp/gsp_fw_wpr_meta.h
#[repr(C)]
pub struct GspFwWprMeta {
    pub magic: u64,                    // 0xdc3aae21371a60b3
    pub revision: u64,                 // = 1
    pub sysmem_addr_of_radix3_elf: u64,
    pub size_of_radix3_elf: u64,
    pub sysmem_addr_of_bootloader: u64,
    pub size_of_bootloader: u64,
    pub bootloader_code_offset: u64,
    pub bootloader_data_offset: u64,
    pub bootloader_manifest_offset: u64,
    pub sysmem_addr_of_signature: u64,
    pub size_of_signature: u64,
    // FB (VRAM) layout — calculado top-down desde fb_size:
    pub gsp_fw_rsvd_start: u64,
    pub non_wpr_heap_offset: u64,
    pub non_wpr_heap_size: u64,
    pub gsp_fw_wpr_start: u64,         // 128KB aligned
    pub gsp_fw_heap_offset: u64,       // 1MB aligned
    pub gsp_fw_heap_size: u64,
    pub gsp_fw_offset: u64,            // 64KB aligned, ELF en VRAM
    pub boot_bin_offset: u64,          // 4KB aligned
    pub frts_offset: u64,
    pub frts_size: u64,
    pub gsp_fw_wpr_end: u64,
    pub fb_size: u64,
    pub vga_workspace_offset: u64,
    pub vga_workspace_size: u64,
    pub boot_count: u64,
    pub verified: u64,                 // 0xa0a0a0a0a0a0a0a0 cuando verificado
    pub flags: u8,
    pub _pad: [u8; 7],
}
// static_assert size == 256
```

**Layout en VRAM (top-down):**
```
fb_size (tope de VRAM = 12GB)
├── VGA workspace (128KB)
├── PMU reserved
├── FRTS data (establecido por FWSEC)
├── Boot binary (SK + BL)        ← boot_bin_offset
├── GSP-RM ELF                   ← gsp_fw_offset
├── GSP FW Heap (WPR)            ← gsp_fw_heap_offset
├── ── WPR2 START ──             ← gsp_fw_wpr_start (128KB aligned)
├── GspFwWprMeta copy
├── Non-WPR Heap                 ← non_wpr_heap_offset
├── ── Reserved start ──         ← gsp_fw_rsvd_start
└── Normal VRAM (tu framebuffer, channels, etc.)
```

### 2. GSP_MSG_QUEUE_ELEMENT — Wrapper de transporte

```rust
/// Fuente: nvidia-open/src/nvidia/inc/kernel/gpu/gsp/message_queue_priv.h
#[repr(C)]
pub struct GspMsgQueueElement {
    pub auth_tag_buffer: [u8; 16],  // AES-GCM (futuro, zeros por ahora)
    pub aad_buffer: [u8; 16],       // Additional auth data (zeros)
    pub checksum: u32,               // XOR de todos los u64 del elemento = 0
    pub sequence: u32,               // Monotónico por queue
    pub elem_count: u32,             // Páginas 4KB que ocupa (1-16)
    pub pad: u32,
    pub rpc: RpcMessageHeader,       // Aligned a 8 bytes
    // ... payload sigue
}
// Tamaño mínimo = 4096 (1 página), máximo = 65536 (16 páginas)
```

### 3. RPC Message Header — El formato REAL

```rust
/// Fuente: nvidia-open/src/nvidia/generated/g_rpc-message-header.h
/// NOTA: La signature es "VRPC" = 0x43505256, NO "VNKV"
#[repr(C)]
pub struct RpcMessageHeader {
    pub header_version: u32,          // 0x03000000
    pub signature: u32,               // 0x43505256 ("VRPC" en LE)
    pub length: u32,                  // sizeof(header) + payload
    pub function: u32,                // NV_VGPU_MSG_FUNCTION_*
    pub rpc_result: u32,              // 0 = success
    pub rpc_result_private: u32,
    pub sequence: u32,
    pub spare: u32,                   // cpuRmGfid
    // pub data: [u8],               // payload variable
}

pub const RPC_SIGNATURE: u32 = 0x43505256; // "VRPC"
pub const RPC_HEADER_VERSION: u32 = 0x03000000;
```

### 4. RPC Function IDs Reales

```rust
/// Fuente: nvidia-open/src/nvidia/inc/kernel/vgpu/rpc_global_enums.h
pub mod rpc_fn {
    pub const FREE: u32                     = 10;
    pub const UNLOADING_GUEST_DRIVER: u32   = 47;
    pub const GET_GSP_STATIC_INFO: u32      = 65;
    pub const CONTINUATION_RECORD: u32      = 71;
    pub const GSP_SET_SYSTEM_INFO: u32      = 72;
    pub const SET_REGISTRY: u32             = 73;
    pub const GSP_RM_CONTROL: u32           = 76;  // Wraps any NVxxxx_CTRL_CMD_*
    pub const GSP_RM_ALLOC: u32             = 103; // Allocates any RM object
    
    // Events (GSP → CPU, en status queue)
    pub const EVENT_GSP_INIT_DONE: u32      = 0x1001;
    pub const EVENT_POST_EVENT: u32         = 0x1003;
    pub const EVENT_RC_TRIGGERED: u32       = 0x1004;
    pub const EVENT_MMU_FAULT: u32          = 0x1005;
    pub const EVENT_RUN_CPU_SEQ: u32        = 0x1006;
    pub const EVENT_OS_ERROR_LOG: u32       = 0x1007;
}
```

### 5. GSP_RM_ALLOC Payload — Para crear CUALQUIER objeto

```rust
#[repr(C)]
pub struct RpcGspRmAlloc {
    pub h_client: u32,
    pub h_parent: u32,
    pub h_object: u32,
    pub h_class: u32,        // e.g. 0xC670 para display
    pub params_size: u32,
    // params: [u8] sigue
}
```

### 6. GSP_RM_CONTROL Payload — Para CUALQUIER control call

```rust
#[repr(C)]
pub struct RpcGspRmControl {
    pub h_client: u32,
    pub h_object: u32,
    pub cmd: u32,            // e.g. NV2080_CTRL_CMD_*
    pub status: u32,
    pub params_size: u32,
    // params: [u8] sigue
}
```

### 7. Display Class IDs (GA10x Ampere)

```rust
pub mod display_class {
    pub const NVC670_DISPLAY: u32              = 0xC670; // Container
    pub const NVC67D_CORE_CHANNEL_DMA: u32     = 0xC67D; // Core channel
    pub const NVC67A_CURSOR_CHANNEL: u32       = 0xC67A; // Per-head cursor
    pub const NVC67B_WINDOW_CHANNEL: u32       = 0xC67B; // Window/overlay
    pub const NVC67E_WINDOW_IMM_CHANNEL: u32   = 0xC67E; // Immediate flip
}

#[repr(C)]
pub struct Nvc670AllocationParams {
    pub num_heads: u32,
    pub num_sors: u32,
    pub num_dsis: u32,
}
```

### 8. FIFO Channel Class (Ampere)

```rust
pub const AMPERE_CHANNEL_GPFIFO: u32 = 0xC36F;

#[repr(C)]
pub struct ChannelGpfifoAllocParams {
    pub gp_fifo_offset: u64,
    pub gp_fifo_entries: u32,
    pub flags: u32,
    pub h_va_space: u32,
    pub engine_type: u32,      // NV2080_ENGINE_TYPE_GR0, etc.
    pub instance_mem: MemoryInfo,
    pub userd_mem: MemoryInfo,
    pub ramfc_mem: MemoryInfo,
    pub mthdbuf_mem: MemoryInfo,
    pub internal_flags: u32,
}

#[repr(C)]
pub struct MemoryInfo {
    pub base: u64,
    pub size: u64,
    pub address_space: u32,   // 1=SYSMEM, 2=FBMEM
    pub cache_attrib: u32,
}
```

---

## Registros GSP que faltan en nv_regs

```rust
pub mod pgsp {
    pub const FALCON_MAILBOX0: u32  = 0x00110040; // Lo32 de libos args PA
    pub const FALCON_MAILBOX1: u32  = 0x00110044; // Hi32 de libos args PA
    pub const QUEUE_HEAD_BASE: u32  = 0x00110C00; // Doorbell base
    
    pub const fn QUEUE_HEAD(q: u32) -> u32 { QUEUE_HEAD_BASE + q * 8 }
    pub const fn QUEUE_TAIL(q: u32) -> u32 { QUEUE_HEAD_BASE + q * 8 + 4 }
    
    // RISCV boot control
    pub const RISCV_CPUCTL: u32     = 0x00110388;
    pub const RISCV_BR_ADDR: u32    = 0x00110390; // Branch address
}
```

---

## Plan de Implementación (Orden de dependencias)

### Fase 1: Corregir estructuras existentes
1. Reemplazar `GspRpcHeader` con `RpcMessageHeader` real
2. Reemplazar `gsp_rpc::MSG_*` con IDs reales
3. Reemplazar `GspBootParams` con `GspFwWprMeta`
4. Agregar registros PGSP (mailbox, queue head, RISCV)

### Fase 2: Firmware loading pipeline
1. Parsear ELF completo: extraer `.fwimage` y `.fwsignature_ga10x`
2. Implementar Radix3 page table builder (3 niveles, 4KB páginas)
3. Cargar bootloader y booter_load blobs
4. Implementar `GspFwWprMeta` population (cálculo top-down de VRAM layout)

### Fase 3: SEC2 authenticated boot
1. Extraer FWSEC ucode de VBIOS ROM
2. Ejecutar FWSEC-FRTS en SEC2 (establece WPR2)
3. Reset GSP a modo RISC-V
4. Escribir libos args en MAILBOX0/1
5. Ejecutar Booter Load en SEC2 (copia ELF a WPR2, arranca GSP)

### Fase 4: Message queue + RPC
1. Alocar cmdq (256KB) + msgq (256KB) en SYSMEM
2. Inicializar headers de queue (tx/rx, entryOff, msgSize, msgCount)
3. Implementar send: fill element → checksum → write to ring → doorbell
4. Implementar recv: poll msgq → read element → handle events vs responses
5. Poll para `GSP_INIT_DONE` (0x1001)

### Fase 5: Init RPCs
1. `GSP_SET_SYSTEM_INFO` (fn 72) — BAR addresses, ACPI info
2. `SET_REGISTRY` (fn 73) — Registry key/value pairs
3. `GET_GSP_STATIC_INFO` (fn 65) — Get internal handles
4. Handle `RUN_CPU_SEQUENCER` events si GSP los envía

### Fase 6: Display via GSP
1. `GSP_RM_ALLOC` class `NVC670_DISPLAY`
2. `GSP_RM_ALLOC` class `NVC67D_CORE_CHANNEL_DMA`
3. `NV2080_CTRL_CMD_INTERNAL_DISPLAY_GET_STATIC_INFO`
4. `NV0073_CTRL_CMD_SYSTEM_GET_NUM_HEADS`
5. Para DP: `NV0073_CTRL_CMD_DP_CTRL` (link training)

### Fase 7: FIFO channels via GSP
1. `GSP_RM_ALLOC` class `0xC36F` (Ampere GPFIFO channel)
2. `NVA06F_CTRL_CMD_BIND` — bind a engine (GR, CE)
3. `NVA06F_CTRL_CMD_GPFIFO_SCHEDULE` — enable scheduling

---

## Archivos de firmware necesarios

Para GA106 (RTX 3060), necesitas 4 blobs del paquete nvidia:

```
nvidia/ga106/gsp/gsp-535.113.01.bin          ← GSP-RM ELF principal (72MB)
nvidia/ga106/gsp/bootloader-535.113.01.bin   ← Bootloader (SK+BL)
nvidia/ga106/gsp/booter_load-535.113.01.bin  ← Booter Load (para SEC2)
nvidia/ga106/gsp/booter_unload-535.113.01.bin← Booter Unload (shutdown)
```

Estos se distribuyen con el driver Linux de NVIDIA y están en `/lib/firmware/nvidia/`.
Tu `gsp_ga10x.bin` existente ES el blob GSP-RM principal.

---

## Fuentes de referencia (archivos clave)

### nvidia-open-gpu-kernel-modules
- `src/nvidia/src/kernel/gpu/gsp/kernel_gsp.c` — Boot sequence principal
- `src/nvidia/src/kernel/gpu/gsp/arch/turing/kernel_gsp_tu102.c` — Bootstrap TU102+ (Ampere hereda)
- `src/nvidia/src/kernel/gpu/gsp/message_queue_cpu.c` — Message queue init
- `src/nvidia/inc/kernel/gpu/gsp/message_queue_priv.h` — Queue element struct
- `src/nvidia/generated/g_rpc-message-header.h` — RPC header
- `src/nvidia/inc/kernel/vgpu/rpc_global_enums.h` — RPC function IDs
- `src/nvidia/arch/nvalloc/common/inc/gsp/gsp_fw_wpr_meta.h` — WPR meta (256B)
- `src/common/sdk/nvidia/inc/class/clc670.h` — Display class
- `src/common/sdk/nvidia/inc/class/clc67d.h` — Core channel DMA

### nouveau (kernel Linux)
- `drivers/gpu/drm/nouveau/nvkm/subdev/gsp/tu102.c` — Boot flow completo
- `drivers/gpu/drm/nouveau/nvkm/subdev/gsp/rm/r535/rpc.c` — RPC send/recv
- `drivers/gpu/drm/nouveau/nvkm/subdev/gsp/rm/r535/disp.c` — Display via RPC
- `drivers/gpu/drm/nouveau/nvkm/subdev/gsp/rm/r535/fifo.c` — FIFO via RPC

---

## Conclusión

**Sí, nvidia-open + nouveau + tu SigDead es SUFICIENTE para implementar el driver completo.**

Lo que SigDead te dio (class IDs, ELF offsets, RPC strings, dispatch tables) es la **validación** 
de que lo que dice nvidia-open/nouveau es correcto para tu hardware específico. 

El camino es claro: **todo pasa por GSP-RM via RPC**. No hay acceso directo a display ni FIFO 
en Ampere — el driver CPU es un thin proxy que manda comandos al RISC-V que corre dentro del GPU.

Tus módulos `nv_display` y `nv_cmd` actuales intentan acceso MMIO directo, que funcionaba en 
GPUs pre-Turing pero NO funciona en Ampere con GSP. Necesitan reescribirse para usar RPC.
