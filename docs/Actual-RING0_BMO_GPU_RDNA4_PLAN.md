# FastOS Ring 0 + BMO GPU + RDNA4 Plan

Este documento fija la separación de responsabilidades para que FastOS no se vuelva un kernel genérico lento. La prioridad actual es Ryzen 5 5600X + UEFI GOP; la ruta GPU nativa queda preparada para AMD RDNA4/RX 9060 XT.

## Principio principal

FastOS se compila para un perfil de hardware concreto. La ruta genérica sólo sirve para reconocer hardware, arrancar de forma segura y seleccionar el perfil correcto. El rendimiento viene de rutas específicas por CPU/GPU.

```text
Ring 3 apps/tests/games
        |
        | BMO GPU syscalls/API
        v
BMO GPU: BSF, recursos, pipelines, command buffers, fences
        |
        | backend validado
        v
Ring 0: drivers reales, MMIO, IRQ, memoria, scheduler, CPU profile
        |
        v
Hardware: Ryzen 5 5600X + AMD RDNA4
```

## Ring 0 no es “genérico”

Ring 0 es el kernel principal de hardware. Debe hacer poco, pero hacerlo bien:

- inicializar CPU, GDT, IDT, syscall, FPU, APIC;
- administrar memoria física/virtual;
- administrar IRQs, timers y scheduler;
- exponer drivers reales;
- validar límites de seguridad antes de tocar hardware;
- mantener perfiles explícitos por CPU.

Para el Ryzen 5 5600X, Ring 0 puede asumir Zen 3, 6 cores/12 threads, AVX2, FMA, AES, invariant TSC, 1 GiB pages y el layout de caché conocido. Para otro CPU, se debe agregar otro perfil en `kernel/src/ring0/platform/` en vez de llenar todo el hot path con fallbacks.

### Modelo de perfiles CPU

```text
ring0/platform/
  mod.rs          -> selecciona/detecta perfil
  cpu.rs          -> CPUID real + identidad
  r5_5600x        -> perfil actual Zen 3/Vermeer
  future_cpu      -> nuevo perfil explícito cuando exista hardware
```

La detección genérica debe responder: “¿este build puede correr aquí?”. Si no coincide, el instalador o boot manager debe escoger otro build/perfil.

## Boot Ring 0 refinado

El coordinator ya no debe inicializar todo dos veces. La única fuente de verdad son las fases:

| Fase | Dueño | Responsabilidad |
|---|---|---|
| 0 | arch/cpu | CPUID, GDT, IDT, syscall, FPU |
| 1 | mem | frame allocator, heap, VMM base |
| 2 | dev | ACPI/PCI discovery seguro; servicios frágiles diferidos |
| 3 | display | GOP framebuffer heredado de UEFI |
| 4 | proc | scheduler, APIC timer, STI |
| 5 | bmo_core | desktop/API CPU-side |

Regla: si un subsistema tiene fase, no se inicializa también en `coordinator::init()`.

## BMO Core

BMO Core es CPU-side: runtime, lenguaje, windowing, FS, audio, desktop y ABI. No debe convertirse en driver GPU. Puede crear trabajo lógico para GPU, pero lo entrega a BMO GPU.

## BMO GPU

BMO GPU es la capa entre Ring 3 y el driver real. Debe ser más completa que un stub, pero no debe tocar MMIO directamente salvo mediante Ring 0.

Responsabilidades:

- validar BSF (BMO Shader Format);
- administrar handles de device/context/queue/buffer/surface/shader/pipeline/fence;
- construir command buffers lógicos;
- validar que Ring 3 sólo use recursos propios;
- enviar trabajo al backend (`SoftwareGop` primero, `AmdGpu` después);
- devolver fences y estado.

### BSF v2 recomendado

BSF no debe guardar sólo SPIR-V. Debe describir target, recursos y política de validación.

Campos mínimos:

- magic/version/header_size/total_size;
- vendor (`AMD = 0x1002`), family (`RDNA4`), target gfx;
- stage: vertex, fragment, compute, mesh/task futuro;
- IR kind: SPIR-V, DXIL, BMO shader IR, AMDGCN ISA offline;
- entrypoint;
- code offset/size;
- metadata offset/size;
- resource table;
- hash kind + hash real.

El kernel no debe compilar HLSL/GLSL/DXIL completo. Inicialmente se aceptan shaders compilados offline o SPIR-V validado por herramienta Ring 3. La compilación pesada vive en herramientas como `nexo-sh-tool` o un port futuro de Mesa/LLVM.

## API GPU para Ring 3

No usar `0x140` para GPU: ese rango ya pertenece a input en BMO API v2. Reservar un rango nuevo, por ejemplo:

| Rango | Uso |
|---|---|
| `0x200..0x23F` | GPU core/device/context |
| `0x240..0x27F` | recursos: buffers, surfaces, images |
| `0x280..0x2BF` | command buffers, submit, fences |
| `0x2C0..0x2FF` | debug/profiling |

Syscalls base:

- `gpu_query_device`
- `gpu_create_context`
- `gpu_create_buffer`
- `gpu_map_buffer`
- `gpu_load_bsf`
- `gpu_create_pipeline`
- `gpu_create_command_buffer`
- `gpu_cmd_copy`
- `gpu_cmd_clear`
- `gpu_cmd_dispatch`
- `gpu_cmd_draw`
- `gpu_submit`
- `gpu_wait_fence`
- `gpu_present`

Al inicio, Ring 3 no debe mandar paquetes PM4 crudos. Debe mandar comandos lógicos que BMO GPU pueda validar.

## Backend 1: SoftwareGop

Antes de RDNA4 real, implementar un backend software sobre GOP:

- surfaces en RAM;
- clear/copy/blit por CPU;
- present al framebuffer GOP;
- fences inmediatos;
- tests de Ring 3 sin tocar GPU nativa.

Esto permite terminar la API y BSF sin depender de firmware AMD.

## Backend 2: AMDGPU RDNA4

El driver real vive en `ring0/dev/amdgpu.rs`.

Orden realista:

1. PCI discovery AMD (`vendor_id = 0x1002`).
2. Habilitar memory space + bus mastering.
3. Mapear BAR MMIO.
4. Mapear doorbell BAR.
5. Mapear VRAM aperture si existe.
6. Leer VBIOS/discovery tables.
7. Detectar IP blocks: PSP, SMU, GMC, SDMA, GFX, DCN.
8. Cargar firmware AMD requerido.
9. Inicializar GMC/VRAM/GART/GPUVM mínimo.
10. Crear writeback page.
11. Inicializar SDMA ring.
12. Probar NOP + fence.
13. Probar copy GTT -> VRAM -> GTT.
14. Inicializar GFX/compute ring.
15. Ejecutar compute shader mínimo.
16. Present/render simple.
17. Display nativo DCN/DMUB después.

No empezar por modesetting nativo. Mantener GOP para sobrevivir; DCN/DMUB/DP/HDMI es un proyecto grande aparte.

## Roadmap recomendado

1. Dejar Ring 0 con boot por fases sin duplicación.
2. Documentar y congelar rangos syscall GPU.
3. Completar `bmo_gpu` con tipos/handles/backend `SoftwareGop`.
4. Rediseñar BSF v2.
5. Crear test Ring 3 que use BMO GPU sin GPU real.
6. Agregar `ring0/dev/amdgpu.rs` sólo para probe PCI/MMIO seguro.
7. Cuando llegue la RDNA4: firmware + SDMA + fences.
8. Después compute/render.

## Regla final

Ring 0 administra hardware. BMO GPU administra la abstracción GPU para Ring 3. BMO Core administra CPU/runtime/windowing/lenguajes. Esa separación evita que FastOS sea una “mula” genérica y permite usar todo el poder del hardware cuando el perfil existe.
