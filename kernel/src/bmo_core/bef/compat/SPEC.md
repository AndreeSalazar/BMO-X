# BEF compat/ — Universal App Intake Layer (roadmap)

> **Status: v1.8.8** — solo planificación. Cero código de compat real.

## Visión

BEF devora PE (Windows) y ELF (Linux) en una representación BEF
interna común. Para ejecutar apps reales se necesita una **capa de
compatibilidad** que traduzca los imports del binario devorado a
llamadas BMO API / BMO ABI.

```text
┌────────────────────────────────────────────────────┐
│  PE app (Windows)                                  │
│      user32.dll!CreateWindowExW                    │
│      ↓                                             │
│  compat/win32/user32.rs::CreateWindowExW (TODO)    │
│      ↓                                             │
│  BMO API: bmo_api::window::create_window           │
│      ↓                                             │
│  BMO Core → ventana real en BMO                    │
└────────────────────────────────────────────────────┘

┌────────────────────────────────────────────────────┐
│  ELF app (Linux)                                   │
│      write(1, "Hi", 2)                             │
│      ↓                                             │
│  compat/linux/libc.rs::write (TODO)                │
│      ↓                                             │
│  BMO ABI: NR_DEBUG_PRINT o NR_FS_WRITE             │
│      ↓                                             │
│  BMO Core → consola real                           │
└────────────────────────────────────────────────────┘
```

## Estado actual (v1.8.8)

| Componente | Estado | Líneas | Tests |
|------------|--------|--------|-------|
| `compat/mod.rs` | Stub | 0 | 0 |
| `compat/win32/*.rs` | Stubs con TODO | 0 | 0 |
| `compat/linux/*.rs` | Stubs con TODO | 0 | 0 |
| `compat/common/*.rs` | Stubs con TODO | 0 | 0 |

**Cero código de compat real**. Solo se documenta la intención.

## Qué falta para v1.9 (target)

### Minimum viable compat (hello world PE/ELF)

| Función | Plataforma | BMO target |
|---------|-----------|------------|
| `ExitProcess` | Win32 | `proc_exit` |
| `WriteFile` | Win32 | `debug_print` o `fs_write` |
| `GetStdHandle` | Win32 | `0xFFFF...` (stderr) |
| `write` | Linux | `debug_print` |
| `exit` | Linux | `proc_exit` |
| `_exit` | Linux | `proc_exit` |

**Total: 6 funciones**. Con esto un `hello world` PE/ELF corre.

### Windowed app PE (v2.0)

| Función | Plataforma | BMO target |
|---------|-----------|------------|
| `CreateWindowExW` | Win32 | `bmo_api::wm_create_window` |
| `RegisterClassExW` | Win32 | `bmo_api::wm_register_class` |
| `DefWindowProcW` | Win32 | (default wnd_proc) |
| `PeekMessageW` | Win32 | `NR_BEFCORE_RECV` |
| `DispatchMessageW` | Win32 | `bmo_api::wm_dispatch` |
| `GetMessageW` | Win32 | `NR_BEFCORE_RECV` (blocking) |
| `TranslateMessage` | Win32 | `NR_WM_TRANSLATE_MESSAGE` |
| `PostQuitMessage` | Win32 | `post_message(QUIT)` |

**Total: 8 funciones más**. Con esto un `notepad` simple corre.

### Games (v3.0+)

| DLL | Funciones clave | BMO target |
|-----|----------------|------------|
| d3d12.dll | `D3D12CreateDevice`, `CreateCommittedResource`, etc. | `bmo_gpu` |
| dxgi.dll | `CreateDXGIFactory`, `CreateSwapChain` | `bmo_gpu` |
| xinput1_4.dll | `XInputGetState`, `XInputSetState` | input syscalls |
| xaudio2_9.dll | `CreateAudioClient`, `Start` | `gustos` |

**Total: 100+ funciones**. Esto es meses de trabajo, NO objetivo de v1.8.8.

## Linux compat (v2.0+)

| Función | BMO target |
|---------|------------|
| `open` / `close` / `read` / `write` | `bmo_api::fs_*` |
| `mmap` / `munmap` / `brk` | `bmo_api::mem_*` |
| `clock_gettime` | `bmo_api::time_*` |
| `pthread_*` | scheduler |
| `epoll_*` / `select` / `poll` | events |
| `signal` | signal table |

**Total: 30+ funciones**.

## Filosofía

1. **No YAGNI**: no crear una capa hasta que una app real la necesite.
2. **Minimum viable primero**: hello world antes que notepad.
3. **BMO API > compat layer**: el compat layer es fino, delega a BMO API.
4. **Tests con apps reales**: cada función se testea con un PE/ELF
   que la use, no con un test unitario aislado.
5. **Documentar TODO**: cada stub tiene `// TODO v1.9: ...` explicando
   qué falta.

## Estructura de archivos

Cada archivo en `compat/` debe tener:

```rust
//! `compat::win32::kernel32` — Stubs de kernel32.dll.
//!
//! Estado: v1.8.8 stub. Funciones NO implementadas.
//! Roadmap: v1.9 (ExitProcess), v2.0 (LoadLibraryW), v3.0 (más).
//!
//! ## Funciones a implementar
//!
//! - v1.9: `ExitProcess`, `GetModuleHandleW`
//! - v2.0: `LoadLibraryW`, `GetProcAddress`, `VirtualAlloc`
//! - v3.0: resto de kernel32.dll

#![allow(dead_code)]

// TODO v1.9: implementar ExitProcess.
// TODO v1.9: implementar GetModuleHandleW.
// TODO v2.0: implementar LoadLibraryW.
// TODO v2.0: implementar GetProcAddress.
// TODO v2.0: implementar VirtualAlloc.
// TODO v3.0: resto de kernel32.

pub fn exit_process(_code: u32) -> ! {
    // v1.9: delegar a BMO API.
    loop { core::arch::asm!("hlt"); }
}
```

## Relación con otras carpetas

| Carpeta | Relación |
|---------|----------|
| `bmo_core/bef/loader/` | Carga BEF/PE/ELF → Image |
| `bmo_core/bef/compat/` | Resuelve imports de PE/ELF → BMO API |
| `bmo_core/bef/runtime.rs` | Tabla de símbolos runtime |
| `bmo_gpu/compat/` (futuro) | D3D/Vulkan → BMO GPU |
| `userland/app.rs` | Llama loader + compat + run |

## Commit que crea este SPEC

`v1.8.8`: solo planificación. Cero código de compat real.
En v1.9 se implementa `compat/win32/kernel32::exit_process`.
