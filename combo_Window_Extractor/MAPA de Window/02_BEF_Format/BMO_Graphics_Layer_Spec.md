# BMO Graphics Layer (Native Translation API)
**Target Reader:** OPUS (Lead GSP Firmware Engineer)
**Target Hardware:** NVIDIA Ampere GA106 (RTX 3060)
**Context:** FastOS utiliza el formato BEF. Las aplicaciones 3D (juegos, OBS) compilan asumiendo una API gráfica de alto nivel (como DirectX 12). Este documento especifica cómo el BMO Graphics Layer interceptará estas llamadas y las traducirá a comandos RPC nativos (GSP Firmware) sin la intermediación de Vulkan ni de Windows.

---

## FASE 1: Anatomía de Wine / VKD3D-Proton (Cómo roban la ejecución)

Proton no emula una GPU, hace **VTable Hooking** y **Shader Transpilation**. FastOS aplicará la misma filosofía pero apuntando a *Bare Metal*.

### 1. Reimplementación de `d3d12.dll`
En Wine/Proton, el archivo `d3d12.dll` no es de Microsoft. Es un binario falso compilado para Linux.
- **Técnica (VTable Overriding):** DX12 usa objetos COM (Component Object Model). Proton expone una interfaz COM idéntica (ej. `ID3D12Device`). Cuando el juego llama a `device->CreateCommandList()`, en realidad salta a una función C++ de Proton que mapea esa orden a `vkAllocateCommandBuffers()`.
- **En BMO:** Haremos exactamente lo mismo. El `BmoProcessEnv` inyectará en la aplicación objetos COM falsos que apuntan a nuestro código Rust en el Kernel.

### 2. El Problema de los Shaders (DXBC/DXIL -> SPIR-V)
Proton no puede enviar shaders de Windows (HLSL compilado) a los drivers de Linux. 
- **VKD3D-Proton:** Usa una librería para transpilar el bytecode DXIL de DirectX a SPIR-V de Vulkan en tiempo real.
- **En BMO (Reto para OPUS):** No podemos usar SPIR-V porque no tenemos Vulkan. BMO tendrá que usar una herramienta AOT (Ahead-Of-Time) o compilar HLSL directamente a **PTX / SASS (NVIDIA Native Instruction Set)**, y enviarlo crudo vía DMA al GSP.

---

## FASE 2: El Mínimo Vital de DirectX 12 (Para FastOS)

DX12 tiene miles de funciones. Para que el 90% de las apps (y motores como Unreal/Unity) dibujen un triángulo o rendericen un frame, solo se necesita implementar la columna vertebral (El Happy Path). 

FastOS/Opus SOLO necesita implementar la traducción nativa para estas 5 operaciones:

1. **`D3D12CreateDevice`**: Inicializa la GPU física y la memoria lógica.
2. **`ID3D12CommandQueue::ExecuteCommandLists`**: Donde el juego envía los "Push Buffers" llenos de órdenes (cambiar shaders, dibujar).
3. **`IDXGISwapChain::Present`**: El "Flip" de la pantalla (Cambiar el front-buffer por el back-buffer para mostrar la imagen en el monitor).
4. **`ID3D12GraphicsCommandList::DrawInstanced` / `DrawIndexedInstanced`**: La orden real de pintar triángulos.
5. **`ID3D12Device::CreateCommittedResource`**: Petición de Memoria VRAM (Texturas, Vertex Buffers).

---

## FASE 3: Diseño del BMO Graphics Layer

Aquí muere DirectX y nace **BMO Graphics**. Esta es la tabla de traducción directa desde la petición del ejecutable `.bef` hasta la instrucción física que OPUS enviará al GSP (Falcon).

### Tabla de Traducción Directa (Sin Vulkan)

| Llamada del Juego (.bef) | BMO Native Translation (Rust Kernel) | Comando Físico (NVIDIA GSP RM RPC) |
| :--- | :--- | :--- |
| `D3D12CreateDevice()` | `bmo_gfx_init_device()` | `MSG_INIT` + Arranque de LibOS (LOGINTR, LOGRM). Asignar GSP Context. |
| `CreateCommittedResource()` | `bmo_gfx_alloc_vram()` | `NV_RM_RPC_ALLOC_MEMORY` (Petición de VRAM pura mapeada al BAR1). |
| `DrawInstanced()` | `bmo_gfx_push_draw()` | Inyección de Method `NV9097_DRAW_VERTEX_ARRAY` en el Ring Buffer DMA. |
| `ExecuteCommandLists()` | `bmo_gfx_submit()` | Escribir al registro MMIO (Doorbell) para que el SEC2 consuma el DMA Queue. |
| `Present()` | `bmo_gfx_flip_display()` | Interacción directa con el Display Engine. Cambiar el puntero del Framebuffer en VRAM. |

### Las Estructuras del BMO Graphics API (Rust)

Esta es la API que tu cargador inyectará. Cero Windows, todo nativo:

```rust
// bmo_gfx_api.rs

#[repr(C)]
pub struct BmoGpuDevice {
    pub gsp_handle: u64,         // Puntero opaco al contexto del GSP en el Kernel
    pub vram_capacity: u64,
}

#[repr(C)]
pub struct BmoCommandQueue {
    pub ring_buffer_ptr: *mut u32, // Memoria DMA mapeada por el Kernel para los Push Buffers
    pub put_offset: u32,
    pub doorbell_register: u64,    // Dirección MMIO física para despertar al GPU
}

/// La VTable inyectada al proceso BEF (El "Falso" d3d12.dll)
#[repr(C)]
pub struct BmoGfxApiVTable {
    // Retorna el handle al hardware inicializado por Opus
    pub init_device: extern "C" fn() -> *mut BmoGpuDevice,
    
    // Aloca memoria VRAM mapeada y devuelve un puntero usable en CPU (BAR1)
    pub alloc_vram: extern "C" fn(device: *mut BmoGpuDevice, size: u64) -> *mut u8,
    
    // Toma un shader (ya en formato nativo PTX) y lo sube al GSP
    pub load_shader: extern "C" fn(device: *mut BmoGpuDevice, ptx_binary: *const u8, len: usize),
    
    // Dispara el Ring Buffer (ExecuteCommandLists)
    pub submit_queue: extern "C" fn(queue: *mut BmoCommandQueue),
    
    // El Flip de pantalla (Present)
    pub present_frame: extern "C" fn(device: *mut BmoGpuDevice, frame_buffer_addr: u64),
}
```

### Integración con el Cargador BEF (`BmoProcessEnv`)

Para que el programa tenga acceso a esto, el Loader del Kernel (Fase 12) modifica el `BmoProcessEnv` inyectando un puntero directo a esta VTable.

```rust
#[repr(C)]
pub struct BmoProcessEnv {
    // ... campos estándar de BEF ...
    pub gsp_enabled: bool,
    
    // Puntero a la API Gráfica Bare Metal.
    // Si la app llama a esto, salta directo al driver GSP de OPUS en Ring 0.
    pub gfx_api: *const BmoGfxApiVTable, 
}
```

### Notas para OPUS (NVIDIA GSP)
*El objetivo de esta capa es que el juego compile pensando que habla con la tarjeta gráfica, pero realmente habla con este `BmoGfxApiVTable`. Tu trabajo (Opus) es garantizar que cuando el juego llame a `submit_queue()`, tu código Rust escriba los comandos DMA correctos en el Ring Buffer y golpee el MMIO Doorbell del GA106 (RTX 3060) para despertar al Falcon SEC2.*
