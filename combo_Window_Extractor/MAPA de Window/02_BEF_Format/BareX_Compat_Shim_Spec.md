# BareX Compatibility Shim Specification v1.0

**Capa:** L4 (compatibilidad opcional con binarios Windows DX9/10/11/12)
**Filosofía:** Permitir correr juegos no portados a BareX nativo, **sin** introducir Win32/WDDM en el kernel. Todo el shim vive en Ring 3, dentro de un sandbox BEF.

> Inspirado en **WINE + Proton (DXVK + VKD3D-Proton)**, pero adelgazado al mínimo: solo PE loader, COM thunks de DX/DXGI y un puñado de NT shims. Sin `kernel32`, sin `user32`, sin `gdi32`.

---

## 1. Alcance: qué se soporta

| Componente Windows | Soporte | Backend BareX |
|---|---|---|
| `d3d9.dll` / `d3d9ex.dll` | ✅ | DXVK-9 → DXBC → SPIR-V → SASS |
| `d3d10.dll` / `d3d10_1.dll` | ✅ | DXVK-10 → SPIR-V → SASS |
| `d3d11.dll` | ✅ | DXVK-11 → SPIR-V → SASS |
| `d3d12.dll` | ✅ | VKD3D-Proton → BareX directo |
| `dxgi.dll` (1.0–1.6) | ✅ | Wrapper sobre `bx_swapchain` |
| `dcomp.dll` (DirectComposition) | 🟡 | Stub (juegos modernos no lo necesitan) |
| `xinput1_4.dll` | ✅ | Mapeo a `bx_input` |
| `xaudio2_9.dll` | ✅ | Mapeo a `bx_audio` |
| `mfplat.dll` (Media Foundation) | 🟡 | Stub mínimo para video cinematics |
| `kernel32`, `user32`, `gdi32`, `ole32` | ⚠️ Stubs ultraligeros | Solo APIs realmente usadas por juegos (~120 funciones) |
| `ntdll` syscalls | ✅ | Traducidos a syscalls FastOS (`FastOS_Syscall_Table_Spec.md`) |
| **DRM** (Denuvo, anti-cheat, EAC, BattlEye) | ❌ | **No soportado por diseño**. Solo juegos sin anti-cheat de kernel. |

---

## 2. Arquitectura del shim

```diagram
╭───────────────────────────────────────────────────────────────╮
│  Proceso BEF "WindowsCompat"  (Ring 3, sandbox)               │
│                                                                │
│  ┌─────────────────────────────────────────────────────────┐  │
│  │  Juego Windows (game.exe — PE64)                        │  │
│  │  Linkado a d3d12.dll, dxgi.dll, kernel32.dll, ...       │  │
│  └────────────┬────────────────────────────────────────────┘  │
│               │ COM vtable calls / imports                    │
│               ▼                                                │
│  ┌─────────────────────────────────────────────────────────┐  │
│  │  PE Loader (mini-WINE)                                   │  │
│  │  - Mapea PE64, resuelve imports                          │  │
│  │  - Aplica relocations, TLS callbacks                     │  │
│  │  - Provee fake DLLs en memoria                           │  │
│  └────────────┬────────────────────────────────────────────┘  │
│               ▼                                                │
│  ┌─────────────────────────────────────────────────────────┐  │
│  │  Fake DLLs:                                              │  │
│  │   • d3d12.dll  ──▶ vkd3d-proton-rs ──▶ BareX            │  │
│  │   • d3d11.dll  ──▶ dxvk-rs ─────────▶ BareX            │  │
│  │   • d3d9.dll   ──▶ dxvk9-rs ────────▶ BareX            │  │
│  │   • dxgi.dll   ──▶ wrapper ─────────▶ bx_swapchain      │  │
│  │   • kernel32, user32, ntdll: stubs mínimos              │  │
│  └────────────┬────────────────────────────────────────────┘  │
│               ▼                                                │
│  ┌─────────────────────────────────────────────────────────┐  │
│  │  BareX C ABI (libbarex.so dentro del sandbox)            │  │
│  └────────────┬────────────────────────────────────────────┘  │
╰───────────────┼────────────────────────────────────────────────╯
                ▼
        FastOS Kernel (Ring 0) ──▶ GSP RTX 3060
```

---

## 3. PE Loader (`fastpe`)

Implementación Rust desde cero (~3000 líneas estimadas), cubre solo lo necesario para juegos:

- Carga PE64 (TE, IMAGE_NT_HEADERS64).
- Resuelve `IMAGE_IMPORT_DESCRIPTOR` con tabla de fake DLLs.
- Aplica relocations `IMAGE_REL_BASED_DIR64`.
- Ejecuta TLS callbacks y `DllMain` (DLL_PROCESS_ATTACH).
- Soporta SEH x64 (`__C_specific_handler`, `RtlUnwindEx` minimal).
- **No soporta:** drivers .sys, native subsystem, .NET CLR.

---

## 4. COM thunks: cómo se finge `ID3D12Device`

Los objetos COM son C++ vtables. El shim crea estructuras Rust con layout `#[repr(C)]` que imitan exactamente la vtable esperada:

```rust
#[repr(C)]
pub struct ID3D12Device_Vtbl {
    pub QueryInterface: extern "stdcall" fn(*mut c_void, *const GUID, *mut *mut c_void) -> HRESULT,
    pub AddRef:         extern "stdcall" fn(*mut c_void) -> u32,
    pub Release:        extern "stdcall" fn(*mut c_void) -> u32,
    // ... 44 métodos de ID3D12Device, 14 de ID3D12Device14
    pub CreateCommandQueue: extern "stdcall" fn(
        this: *mut c_void,
        desc: *const D3D12_COMMAND_QUEUE_DESC,
        riid: *const GUID,
        out: *mut *mut c_void,
    ) -> HRESULT,
    // ...
}
```

Cada método llama directamente a la API BareX equivalente. **Cero llamadas a Windows**, cero traducción a Vulkan intermedio.

---

## 5. NT Syscall shims

WINE traduce ~600 syscalls. BareX shim solo necesita ~80 (las realmente usadas por juegos modernos):

| Categoría NT | Mapeo FastOS |
|---|---|
| `NtAllocateVirtualMemory` / `NtFreeVirtualMemory` | `fos_mmap` / `fos_munmap` |
| `NtCreateFile` / `NtReadFile` / `NtWriteFile` | VFS BareX (`FastOS_VFS_Spec.md`) |
| `NtCreateThread` / `NtTerminateThread` | Scheduler FastOS |
| `NtWaitForSingleObject` / `NtSetEvent` | Futex FastOS (`FastOS_Locking_Primitives.md`) |
| `NtQuerySystemInformation` | Lectura simulada (CPU info hardcoded a Ryzen 5600X) |
| `NtCreateSection` (memory mapped files) | `fos_mmap_file` |
| Registry (`NtOpenKey`, `NtQueryValueKey`) | KV store en `/system/registry.kv` (mock) |

---

## 6. Estrategia de compatibilidad gradual

| Fase | Hitos |
|---|---|
| **C0** | PE loader carga `notepad.exe` y dibuja un mensaje (sin gráficos). |
| **C1** | Demo D3D11: corre `D3D11Tutorial.exe` (Microsoft samples). |
| **C2** | Juego DX11 indie pequeño (ej. *Hollow Knight*). |
| **C3** | Juego DX12 AAA sin anti-cheat (ej. *Cyberpunk 2077*, *DOOM Eternal*). |
| **C4** | Paridad con Proton para top-100 Steam sin anti-cheat. |
| **C5** | Soporte XAudio2 + XInput completo + Media Foundation para cinematics. |

---

## 7. Diferencias vs Proton/WINE (por qué será más rápido)

| Punto | Proton/Linux | BareX Compat Shim |
|---|---|---|
| Capas API | DX12 → VKD3D → Vulkan → Mesa/NVK → DRM/KMS → GPU | DX12 → VKD3D-rs → **BareX** → GSP → GPU |
| Scheduler | Linux CFS + sched_realtime opcional | Scheduler FastOS especializado en juegos (`FastOS_Scheduler_Spec.md`) |
| File I/O | NTFS-3g / ext4 + page cache + WINE NT path translation | NVMe directo + DirectStorage en `bx_io` |
| Compilación shaders en runtime | Sí (causa stutters) | Pre-cache automático + GDeflate |
| Overhead syscall WINE→Linux | Doble syscall en hot paths | Una sola syscall FastOS por operación |
| Memoria virtual | mmap Linux con TLB shootdowns globales | Espacio de direcciones dedicado por proceso BEF |

**Estimación de rendimiento:**
- Proton vs Windows nativo: **0.85–0.90x** (15–10% peor).
- BareX Compat Shim vs Windows nativo: **0.95–1.02x** (objetivo: paridad o mejor en algunos casos por DirectStorage real).
- BareX nativo (juego portado) vs Windows: **1.05–1.10x** (mejor).

---

## 8. Sandbox y seguridad

Cada juego corre en su propio proceso BEF con:
- Espacio de direcciones aislado.
- Acceso solo a `/games/{game_id}/` y `/saves/{game_id}/`.
- Sin acceso a otros procesos ni al kernel.
- Capacidades GSP limitadas a un subset de canales.

Esto está alineado con `FastOS_App_Sandbox.md` y `FastOS_Security_Model.md`.

---

## 9. Lo que NO se hará

- **Anti-cheat de kernel** (EAC kernel mode, BattlEye kernel, Vanguard, nProtect): violaría el modelo zero-legacy. Si la industria libera SDKs userspace, se evaluará.
- **DirectInput legacy** (XP-era).
- **Managed DirectX / WPF / GDI+**.
- **Windows Media Player APIs**.
- **Compatibilidad x86 32-bit**: solo PE64. Sin WoW64.

---

## 10. Archivos relacionados

- `BareX_API_Spec.md` (L3, backend del shim)
- `BareX_Shader_Pipeline.md` (L2, traduce DXBC/DXIL en runtime cuando el juego compila al vuelo)
- `FastOS_Syscall_Table_Spec.md` (NT syscalls → FastOS)
- `FastOS_App_Sandbox.md` (aislamiento)
- `FastOS_VFS_Spec.md` (path translation)
- `Win32_Minimum_Surface.md` (qué APIs Win32 valen la pena emular)
