# BMO/FastOS — Plan Maestro: Independencia de Windows

## Objetivo
Crear un sistema operativo 100% independiente de Windows, inspirándose en sus APIs
pero reimplementando todo de cero en Rust (`no_std`), con separación Ring 0/Ring 3.

## Arquitectura

```
┌─────────────────────────────────────────────────────────┐
│                    Ring 3 (User Mode)                    │
│  ┌─────────────┐  ┌─────────────┐  ┌─────────────┐    │
│  │  BMO API    │  │  BareX      │  │  Apps       │    │
│  │  (Win32     │  │  (DX9-12    │  │  (Games,    │    │
│  │   equiv)    │  │   equiv)    │  │   Tools)    │    │
│  └──────┬──────┘  └──────┬──────┘  └──────┬──────┘    │
│         │                │                │            │
│  ┌──────┴────────────────┴────────────────┴──────┐    │
│  │           BMO ABI (System Calls)               │    │
│  └──────────────────────┬────────────────────────┘    │
├─────────────────────────┼─────────────────────────────┤
│                    Ring 0 (Kernel Mode)                 │
│  ┌─────────────┐  ┌────┴────┐  ┌─────────────┐      │
│  │  BMO Core   │  │ BareX   │  │  Drivers    │      │
│  │  (Process,  │  │ (GPU,   │  │  (Storage,  │      │
│  │   Memory,   │  │  Audio, │  │   Network,  │      │
│  │   VFS)      │  │  Input) │  │   USB)      │      │
│  └─────────────┘  └─────────┘  └─────────────┘      │
└─────────────────────────────────────────────────────┘
```

---

## PARTE 1: BMO API (Equivalente a Win32 API)

### 1.1 Process Management (kernel32 equiv)

| Win32 API | BMO API | Estado | Ring |
|-----------|---------|--------|------|
| `CreateProcess` | `bx_process_create` | 🔴 Stub | 0 |
| `ExitProcess` | `bx_process_exit` | 🔴 Stub | 0 |
| `TerminateProcess` | `bx_process_kill` | 🔴 Stub | 0 |
| `GetCurrentProcess` | `bx_process_current` | 🔴 Stub | 0 |
| `GetProcessId` | `bx_process_id` | 🔴 Stub | 0 |
| `OpenProcess` | `bx_process_open` | 🔴 Stub | 0 |
| `WaitForSingleObject` | `bx_wait` | 🔴 Stub | 0 |
| `WaitForMultipleObjects` | `bx_wait_multi` | 🔴 Stub | 0 |
| `CreateThread` | `bx_thread_create` | 🔴 Stub | 0 |
| `ExitThread` | `bx_thread_exit` | 🔴 Stub | 0 |
| `SuspendThread` | `bx_thread_suspend` | 🔴 Stub | 0 |
| `ResumeThread` | `bx_thread_resume` | 🔴 Stub | 0 |
| `SetThreadPriority` | `bx_thread_priority` | 🔴 Stub | 0 |
| `GetCurrentThreadId` | `bx_thread_id` | 🔴 Stub | 0 |

### 1.2 Memory Management

| Win32 API | BMO API | Estado | Ring |
|-----------|---------|--------|------|
| `VirtualAlloc` | `bx_mem_alloc` | 🟡 Página demand | 0 |
| `VirtualFree` | `bx_mem_free` | 🔴 Stub | 0 |
| `VirtualProtect` | `bx_mem_protect` | 🔴 Stub | 0 |
| `VirtualQuery` | `bx_mem_query` | 🔴 Stub | 0 |
| `HeapCreate` | `bx_heap_create` | 🔴 Stub | 0 |
| `HeapAlloc` | `bx_heap_alloc` | 🟡 Bump only | 0 |
| `HeapFree` | `bx_heap_free` | 🔴 Stub | 0 |
| `MapViewOfFile` | `bx_mmap` | 🔴 Stub | 0 |
| `UnmapViewOfFile` | `bx_munmap` | 🔴 Stub | 0 |
| `CreateFileMapping` | `bx_shm_create` | 🔴 Stub | 0 |
| `GetPhysicallyInstalledSystemMemory` | `bx_mem_info` | 🔴 Stub | 0 |

### 1.3 File System (NTFS-inspired)

| Win32 API | BMO API | Estado | Ring |
|-----------|---------|--------|------|
| `CreateFile` | `bx_fs_open` | 🔴 Stub | 0 |
| `ReadFile` | `bx_fs_read` | 🟡 FAT32 RO | 0 |
| `WriteFile` | `bx_fs_write` | 🔴 Stub | 0 |
| `DeleteFile` | `bx_fs_delete` | 🔴 Stub | 0 |
| `MoveFile` | `bx_fs_move` | 🔴 Stub | 0 |
| `CopyFile` | `bx_fs_copy` | 🔴 Stub | 0 |
| `FindFirstFile` | `bx_fs_find_first` | 🔴 Stub | 0 |
| `FindNextFile` | `bx_fs_find_next` | 🔴 Stub | 0 |
| `GetFileSize` | `bx_fs_size` | 🔴 Stub | 0 |
| `SetFilePointer` | `bx_fs_seek` | 🔴 Stub | 0 |
| `CreateDirectory` | `bx_fs_mkdir` | 🔴 Stub | 0 |
| `RemoveDirectory` | `bx_fs_rmdir` | 🔴 Stub | 0 |
| `GetCurrentDirectory` | `bx_fs_cwd` | 🔴 Stub | 0 |
| `SetCurrentDirectory` | `bx_fs_chdir` | 🔴 Stub | 0 |
| `GetTempPath` | `bx_fs_tmp` | 🔴 Stub | 0 |

### 1.4 Synchronization

| Win32 API | BMO API | Estado | Ring |
|-----------|---------|--------|------|
| `CreateMutex` | `bx_mutex_create` | 🔴 Stub | 0 |
| `ReleaseMutex` | `bx_mutex_release` | 🔴 Stub | 0 |
| `CreateEvent` | `bx_event_create` | 🔴 Stub | 0 |
| `SetEvent` | `bx_event_set` | 🔴 Stub | 0 |
| `ResetEvent` | `bx_event_reset` | 🔴 Stub | 0 |
| `CreateSemaphore` | `bx_sem_create` | 🔴 Stub | 0 |
| `ReleaseSemaphore` | `bx_sem_release` | 🔴 Stub | 0 |
| `InitializeCriticalSection` | `bx_crit_init` | 🔴 Stub | 0 |
| `EnterCriticalSection` | `bx_crit_enter` | 🔴 Stub | 0 |
| `LeaveCriticalSection` | `bx_crit_leave` | 🔴 Stub | 0 |
| `Sleep` | `bx_sleep` | 🔴 Stub | 0 |
| `QueryPerformanceCounter` | `bx_time_counter` | 🔴 Stub | 0 |
| `QueryPerformanceFrequency` | `bx_time_freq` | 🔴 Stub | 0 |

### 1.5 String & Encoding

| Win32 API | BMO API | Estado | Ring |
|-----------|---------|--------|------|
| `lstrlen` | `bx_str_len` | 🔴 Stub | 3 |
| `lstrcpy` | `bx_str_copy` | 🔴 Stub | 3 |
| `lstrcmp` | `bx_str_cmp` | 🔴 Stub | 3 |
| `CharUpper` | `bx_str_upper` | 🔴 Stub | 3 |
| `CharLower` | `bx_str_lower` | 🔴 Stub | 3 |
| `wsprintf` | `bx_fmt_sprintf` | 🔴 Stub | 3 |
| `MultiByteToWideChar` | `bx_utf8_to_utf16` | 🔴 Stub | 3 |
| `WideCharToMultiByte` | `bx_utf16_to_utf8` | 🔴 Stub | 3 |

### 1.6 Registry (Windows Registry equiv)

| Win32 API | BMO API | Estado | Ring |
|-----------|---------|--------|------|
| `RegOpenKeyEx` | `bx_reg_open` | 🔴 Stub | 0 |
| `RegQueryValueEx` | `bx_reg_read` | 🔴 Stub | 0 |
| `RegSetValueEx` | `bx_reg_write` | 🔴 Stub | 0 |
| `RegCloseKey` | `bx_reg_close` | 🔴 Stub | 0 |
| `RegCreateKeyEx` | `bx_reg_create` | 🔴 Stub | 0 |
| `RegDeleteValue` | `bx_reg_delete` | 🔴 Stub | 0 |

**Nota**: BMO usaría un Key-Value store en disco (no registry hive binaria).

### 1.7 Dynamic Link Library

| Win32 API | BMO API | Estado | Ring |
|-----------|---------|--------|------|
| `LoadLibrary` | `bx_mod_load` | 🔴 Stub | 3 |
| `GetProcAddress` | `bx_mod_sym` | 🔴 Stub | 3 |
| `FreeLibrary` | `bx_mod_free` | 🔴 Stub | 3 |
| `GetModuleHandle` | `bx_mod_self` | 🔴 Stub | 3 |

**Formato**: BEF (BMO Executable Format) en vez de PE/COFF.

### 1.8 Window Management (user32 equiv)

| Win32 API | BMO API | Estado | Ring |
|-----------|---------|--------|------|
| `CreateWindowEx` | `bx_win_create` | 🔴 Stub | 0 |
| `DestroyWindow` | `bx_win_destroy` | 🔴 Stub | 0 |
| `ShowWindow` | `bx_win_show` | 🔴 Stub | 0 |
| `UpdateWindow` | `bx_win_update` | 🔴 Stub | 0 |
| `GetMessage` | `bx_msg_get` | 🔴 Stub | 0 |
| `PeekMessage` | `bx_msg_peek` | 🔴 Stub | 0 |
| `DispatchMessage` | `bx_msg_dispatch` | 🔴 Stub | 0 |
| `PostQuitMessage` | `bx_msg_quit` | 🔴 Stub | 0 |
| `DefWindowProc` | `bx_msg_default` | 🔴 Stub | 0 |
| `SetWindowPos` | `bx_win_move` | 🔴 Stub | 0 |
| `GetClientRect` | `bx_win_rect` | 🔴 Stub | 0 |
| `InvalidateRect` | `bx_win_invalidate` | 🔴 Stub | 0 |
| `SetCapture` | `bx_win_capture` | 🔴 Stub | 0 |
| `ReleaseCapture` | `bx_win_release` | 🔴 Stub | 0 |

**Nota**: BMO usaría compositor en Ring 0, ventanas en Ring 3.

### 1.9 Input (DirectInput/XInput equiv)

| Win32 API | BMO API | Estado | Ring |
|-----------|---------|--------|------|
| `GetAsyncKeyState` | `bx_input_key` | 🟡 Estructura OK | 0 |
| `GetKeyState` | `bx_input_key_state` | 🔴 Stub | 0 |
| `GetKeyboardState` | `bx_input_all_keys` | 🔴 Stub | 0 |
| `GetCursorPos` | `bx_input_mouse_pos` | 🔴 Stub | 0 |
| `SetCursorPos` | `bx_input_mouse_set` | 🔴 Stub | 0 |
| `GetRawInputData` | `bx_input_raw` | 🔴 Stub | 0 |
| `XInputGetState` | `bx_gamepad_state` | 🔴 Stub | 0 |
| `XInputSetState` | `bx_gamepad_rumble` | 🔴 Stub | 0 |

### 1.10 Networking (Winsock equiv)

| Win32 API | BMO API | Estado | Ring |
|-----------|---------|--------|------|
| `WSAStartup` | `bx_net_init` | 🟡 RTL8168 OK | 0 |
| `socket` | `bx_net_socket` | 🟡 UDP OK | 0 |
| `bind` | `bx_net_bind` | 🔴 Stub | 0 |
| `listen` | `bx_net_listen` | 🔴 Stub | 0 |
| `accept` | `bx_net_accept` | 🔴 Stub | 0 |
| `connect` | `bx_net_connect` | 🔴 Stub | 0 |
| `send` | `bx_net_send` | 🟡 UDP OK | 0 |
| `recv` | `bx_net_recv` | 🟡 UDP OK | 0 |
| `sendto` | `bx_net_sendto` | 🟡 UDP OK | 0 |
| `recvfrom` | `bx_net_recvfrom` | 🟡 UDP OK | 0 |
| `closesocket` | `bx_net_close` | 🔴 Stub | 0 |
| `gethostbyname` | `bx_net_dns` | 🔴 Stub | 0 |
| `select` | `bx_net_select` | 🔴 Stub | 0 |

### 1.11 Security

| Win32 API | BMO API | Estado | Ring |
|-----------|---------|--------|------|
| `CreateProcessAsUser` | `bx_sec_runas` | 🔴 Stub | 0 |
| `OpenProcessToken` | `bx_sec_token` | 🔴 Stub | 0 |
| `AdjustTokenPrivileges` | `bx_sec_priv` | 🔴 Stub | 0 |
| `GetFileSecurity` | `bx_sec_file` | 🔴 Stub | 0 |

**Nota**: BMO usa ByteDefender + sandboxing nativo.

---

## PARTE 2: BareX API (Equivalente a DirectX 9-12)

### 2.1 Core Graphics (DXGI equiv)

| DirectX | BareX API | Estado | Ring |
|---------|-----------|--------|------|
| `IDXGIFactory` | `BxFactory` | 🔴 Stub | 3 |
| `IDXGIAdapter` | `BxAdapter` | 🔴 Stub | 3 |
| `IDXGISwapChain` | `BxSwapchain` | 🟡 Present no-op | 3 |
| `IDXGIDevice` | `BxDevice` | 🟡 Software raster | 3 |
| `IDXGISurface` | `BxSurface` | 🔴 Stub | 3 |
| `IDXGIOutput` | `BxOutput` | 🔴 Stub | 3 |

### 2.2 Device & Command Lists (D3D11/D3D12 equiv)

| DirectX | BareX API | Estado | Ring |
|---------|-----------|--------|------|
| `ID3D11Device` | `BxDevice::create` | 🟡 Software | 3 |
| `ID3D11DeviceContext` | `BxCmdList` | 🟡 Basic 2D | 3 |
| `ID3D12CommandQueue` | `BxQueue` | 🟡 Definitions | 3 |
| `ID3D12CommandAllocator` | `BxCmdAllocator` | 🔴 Stub | 3 |
| `ID3D12CommandList` | `BxCmdList` | 🟡 Basic 2D | 3 |
| `ID3D12RootSignature` | `BxRootSig` | 🔴 Stub | 3 |
| `ID3D12PipelineState` | `BxPso` | 🔴 Stub | 3 |
| `ID3D12Fence` | `BxFence` | 🔴 Stub | 3 |

### 2.3 Resources (Buffers & Textures)

| DirectX | BareX API | Estado | Ring |
|---------|-----------|--------|------|
| `ID3D11Buffer` | `BxBuffer` | 🟡 CPU alloc | 3 |
| `ID3D11Texture2D` | `BxTexture` | 🟡 Basic | 3 |
| `ID3D12Resource` | `BxResource` | 🔴 Stub | 3 |
| `CreateBuffer` | `BxBuffer::new` | 🟡 Works | 3 |
| `CreateTexture2D` | `BxTexture::new` | 🟡 Basic | 3 |
| `Map/Unmap` | `BxBuffer::map` | 🟡 CPU access | 3 |
| `CopyResource` | `cmd_copy` | 🔴 Stub | 3 |

### 2.4 Shader Pipeline

| DirectX | BareX API | Estado | Ring |
|---------|-----------|--------|------|
| `D3DCompile` | `BxShader::compile` | 🔴 Stub | 3 |
| `ID3D11VertexShader` | `BxShader::Vertex` | 🔴 Stub | 3 |
| `ID3D11PixelShader` | `BxShader::Pixel` | 🔴 Stub | 3 |
| `ID3D12PipelineState` | `BxPso::new` | 🔴 Stub | 3 |
| DXBC → SPIR-V | `dxbc::translate` | 🔴 Stub | 3 |
| DXIL → SPIR-V | `dxil::translate` | 🔴 Stub | 3 |
| SPIR-V → Native | `spirv::translate` | 🔴 Stub | 3 |

**Pipeline de shaders**:
```
DXBC/DXIL (Windows) → SPIR-V (estándar) → Native IR (BMO)
     ↓                    ↓                     ↓
  Offline              Translator           Runtime
```

### 2.5 Rendering Commands

| DirectX | BareX API | Estado | Ring |
|---------|-----------|--------|------|
| `ClearRenderTargetView` | `cmd_clear` | 🟡 Funciona | 3 |
| `Draw` | `cmd_draw` | 🔴 Stub | 3 |
| `DrawIndexed` | `cmd_draw_indexed` | 🔴 Stub | 3 |
| `Dispatch` | `cmd_dispatch_compute` | 🔴 Stub | 3 |
| `CopyBufferRegion` | `cmd_copy` | 🔴 Stub | 3 |
| `CopyTextureRegion` | `cmd_blit` | 🟡 Magenta | 3 |
| `RSSetViewports` | `cmd_set_viewport` | 🔴 Stub | 3 |
| `RSSetScissorRects` | `cmd_set_scissor` | 🔴 Stub | 3 |
| `OMSetRenderTargets` | `cmd_set_target` | 🔴 Stub | 3 |
| `IASetVertexBuffers` | `cmd_set_vb` | 🔴 Stub | 3 |
| `IASetIndexBuffer` | `cmd_set_ib` | 🔴 Stub | 3 |
| `VSSetShader` | `cmd_set_vs` | 🔴 Stub | 3 |
| `PSSetShader` | `cmd_set_ps` | 🔴 Stub | 3 |

### 2.6 Audio (XAudio2/WASAPI equiv)

| DirectX | BareX API | Estado | Ring |
|---------|-----------|--------|------|
| `IXAudio2` | `BxAudioEngine` | 🟡 Estructura OK | 3 |
| `IXAudio2SourceVoice` | `BxVoice` | 🟡 Estructura OK | 3 |
| `IXAudio2MasteringVoice` | `BxMaster` | 🔴 Stub | 3 |
| `XAudio2Create` | `bx_audio_init` | 🔴 Stub | 0 |
| `CreateSourceVoice` | `bx_audio_voice` | 🔴 Stub | 0 |
| `Start/Stop/Flush` | `bx_audio_*` | 🔴 Stub | 0 |
| `SubmitSourceBuffer` | `bx_audio_submit` | 🔴 Stub | 0 |
| `SetVolume` | `bx_audio_volume` | 🔴 Stub | 0 |
| `SetFrequencyRatio` | `bx_audio_pitch` | 🔴 Stub | 0 |

### 2.7 Input (DirectInput/XInput equiv)

| DirectX | BareX API | Estado | Ring |
|---------|-----------|--------|------|
| `DirectInput8Create` | `bx_input_init` | 🟡 Estructura OK | 3 |
| `IDirectInputDevice8` | `BxInputDevice` | 🔴 Stub | 3 |
| `Acquire/Unacquire` | `bx_input_acquire` | 🔴 Stub | 3 |
| `GetDeviceState` | `bx_input_state` | 🟡 256 keys | 3 |
| `XInputGetState` | `bx_gamepad_*` | 🔴 Stub | 3 |
| `GetRawInputData` | `bx_input_raw` | 🔴 Stub | 3 |

### 2.8 Networking (Winsock equiv)

| DirectX | BareX API | Estado | Ring |
|---------|-----------|--------|------|
| `WSAStartup` | `bx_net_init` | 🟡 Basic | 3 |
| `socket` | `bx_net_socket` | 🟡 UDP | 3 |
| `send/recv` | `bx_net_*` | 🟡 UDP | 3 |
| `TCP connect` | `bx_net_tcp` | 🔴 Stub | 3 |
| `DNS resolve` | `bx_net_dns` | 🔴 Stub | 3 |
| `TLS/SSL` | `bx_net_tls` | 🔴 Stub | 3 |
| `HTTP client` | `bx_http_*` | 🔴 Stub | 3 |

---

## PARTE 3: Filesystem (NTFS-inspired)

### 3.1 BMO-FS (NTFS-inspired)

| NTFS Feature | BMO-FS | Estado | Ring |
|--------------|--------|--------|------|
| MFT (Master File Table) | `bmofs::Mft` | 🔴 Stub | 0 |
| Clusters & extents | `bmofs::Extents` | 🔴 Stub | 0 |
| Journaling (NTFS Log) | `bmofs::Journal` | 🔴 Stub | 0 |
| ACLs & permissions | `bmofs::Acl` | 🔴 Stub | 0 |
| Alternate Data Streams | `bmofs::Ads` | 🔴 Stub | 0 |
| Hard links | `bmofs::Hardlink` | 🔴 Stub | 0 |
| Symbolic links | `bmofs::Symlink` | 🔴 Stub | 0 |
| Compression | `bmofs::Compress` | 🔴 Stub | 0 |
| Encryption (EFS) | `bmofs::Encrypt` | 🔴 Stub | 0 |
| Sparse files | `bmofs::Sparse` | 🔴 Stub | 0 |

### 3.2 VFS Layer

| Feature | BMO VFS | Estado | Ring |
|---------|---------|--------|------|
| Mount points | `vfs::mount` | 🟡 Structure | 0 |
| Inode table | `vfs::inode` | 🟡 Structure | 0 |
| File descriptors | `vfs::fd` | 🟡 Structure | 0 |
| Page cache | `vfs::cache` | 🔴 Stub | 0 |
| Async I/O (io_uring) | `vfs::aio` | 🔴 Stub | 0 |

### 3.3 Storage Drivers

| Driver | Estado | Ring |
|--------|--------|------|
| NVMe | 🟡 Read-only | 0 |
| AHCI (SATA) | 🟡 Read/write | 0 |
| USB Mass Storage | 🟡 SCSI layer | 0 |
| VirtIO-Block | 🔴 Stub | 0 |
| AHCI trim | 🔴 Stub | 0 |

---

## PARTE 4: Ring 0 / Ring 3 Separación

### 4.1 Ring 0 (Kernel Mode)

```
kernel/
├── arch/           # CPU, interrupts, GDT, IDT, TSS, SYSCALL
├── memory/         # VMM, page allocator, heap
├── sched/          # Process, thread, scheduler, futex
├── fs/             # VFS, FAT32, BMO-FS, storage drivers
├── drivers/        # GPU, USB, NIC, audio, input
├── security/       # ByteDefender, Restaurer, sandbox
├── ipc/            # Shared memory, pipes, message queues
├── net/            # TCP/IP stack, sockets
└── diag/           # Telemetry, diagnostics
```

**Responsabilidades Ring 0**:
- Gestión de memoria virtual
- Planificación de procesos/hilos
- Manejo de interrupciones
- Drivers de hardware
- Sistema de archivos
- Seguridad (antivirus, sandbox)

### 4.2 Ring 3 (User Mode)

```
user/
├── bmo-api/        # Win32-equivalent API library
│   ├── process/    # Process/thread management
│   ├── memory/     # Virtual memory, heap
│   ├── fs/         # File operations
│   ├── sync/       # Mutex, events, semaphores
│   ├── string/     # String operations, encoding
│   ├── win/        # Window management
│   ├── input/      # Keyboard, mouse, gamepad
│   ├── audio/      # Audio playback
│   └── net/        # Networking (TCP, UDP, HTTP)
├── barex-api/      # DirectX-equivalent API library
│   ├── graphics/   # D3D9-12 equivalent
│   ├── audio/      # XAudio2 equivalent
│   ├── input/      # DirectInput/XInput equivalent
│   └── net/        # Winsock equivalent
├── runtime/        # C runtime, startup code
└── apps/           # User applications
```

**Responsabilidades Ring 3**:
- Aplicaciones de usuario
- Bibliotecas de sistema (BMO API, BareX API)
- Drivers de usuario (GPU user-mode driver)
- Compiladores, shells, editores

### 4.3 Syscall Interface

```rust
// Ring 3 → Ring 0 transition
// via SYSCALL/SYSRET (AMD64)

// Números de syscall (planificados)
pub const SYS_PROCESS_CREATE: u64 = 0x00;
pub const SYS_PROCESS_EXIT: u64 = 0x01;
pub const SYS_THREAD_CREATE: u64 = 0x02;
pub const SYS_THREAD_EXIT: u64 = 0x03;
pub const SYS_MEM_ALLOC: u64 = 0x04;
pub const SYS_MEM_FREE: u64 = 0x05;
pub const SYS_FS_OPEN: u64 = 0x06;
pub const SYS_FS_READ: u64 = 0x07;
pub const SYS_FS_WRITE: u64 = 0x08;
pub const SYS_FS_CLOSE: u64 = 0x09;
pub const SYS_NET_SOCKET: u64 = 0x0A;
pub const SYS_NET_SEND: u64 = 0x0B;
pub const SYS_NET_RECV: u64 = 0x0C;
pub const SYS_INPUT_GET: u64 = 0x0D;
pub const SYS_AUDIO_INIT: u64 = 0x0E;
pub const SYS_TIME_WAIT: u64 = 0x0F;
// ... más syscalls
```

---

## PARTE 5: Plan de Implementación

### Fase 1: Core del Kernel (Prioridad ALTA)

| # | Tarea | Esfuerzo | Estado |
|---|-------|----------|--------|
| 1.1 | Context switch (save/restore regs) | Medio | 🔴 |
| 1.2 | APIC timer → scheduler tick | Bajo | 🔴 |
| 1.3 | Syscall dispatcher (22 syscalls) | Medio | 🔴 |
| 1.4 | Heap allocator real (free-list) | Medio | 🔴 |
| 1.5 | mmap/munmap/mprotect syscalls | Medio | 🔴 |
| 1.6 | USB HID polling (xHCI) | Alto | 🔴 |
| 1.7 | TCP/IP stack | Alto | 🔴 |
| 1.8 | FAT32 write support | Medio | 🔴 |
| 1.9 | Basic window system (compositor) | Alto | 🔴 |
| 1.10 | PE loader + BEF format | Alto | 🔴 |

### Fase 2: BMO API (Prioridad MEDIA)

| # | Tarea | Esfuerzo | Estado |
|---|-------|----------|--------|
| 2.1 | Process management API | Bajo | 🔴 |
| 2.2 | Memory management API | Bajo | 🔴 |
| 2.3 | File system API | Medio | 🔴 |
| 2.4 | Synchronization API | Bajo | 🔴 |
| 2.5 | String/encoding API | Bajo | 🔴 |
| 2.6 | Window management API | Alto | 🔴 |
| 2.7 | Input API | Medio | 🔴 |
| 2.8 | Audio API | Alto | 🔴 |
| 2.9 | Networking API | Medio | 🔴 |

### Fase 3: BareX API (Prioridad BAJA — esperar GPU)

| # | Tarea | Esfuerzo | Estado |
|---|-------|----------|--------|
| 3.1 | Graphics core (D3D12-style) | Muy Alto | 🟡 API only |
| 3.2 | Shader pipeline (DXBC→SPIR-V→Native) | Muy Alto | 🔴 |
| 3.3 | Audio engine (XAudio2-style) | Alto | 🔴 |
| 3.4 | Input system (DirectInput-style) | Medio | 🟡 Structure |
| 3.5 | Networking (Winsock-style) | Medio | 🟡 UDP only |
| 3.6 | GPU driver (cuando tengas GPU) | Muy Alto | 🔴 |

### Fase 4: NTFS-inspired Filesystem

| # | Tarea | Esfuerzo | Estado |
|---|-------|----------|--------|
| 4.1 | BMO-FS MFT structure | Alto | 🔴 |
| 4.2 | Journaling | Alto | 🔴 |
| 4.3 | ACLs & permissions | Medio | 🔴 |
| 4.4 | Compression | Medio | 🔴 |
| 4.5 | Encryption (EFS) | Medio | 🔴 |
| 4.6 | Hard/symbolic links | Bajo | 🔴 |

### Fase 5: Compatibility Layer

| # | Tarea | Esfuerzo | Estado |
|---|-------|----------|--------|
| 5.1 | PE loader (COFF/PE32+) | Alto | 🔴 |
| 5.2 | Win32 API thunks | Muy Alto | 🔴 |
| 5.3 | D3D → BareX translation | Muy Alto | 🔴 |
| 5.4 | XInput emulation | Medio | 🔴 |
| 5.5 | XAudio2 emulation | Alto | 🔴 |
| 5.6 | Winsock emulation | Medio | 🔴 |

---

## PARTE 6: Documentación Requerida

### 6.1 Para BMO API (Win32-equivalent)

| Documento | Fuente | Uso |
|-----------|--------|-----|
| Windows API documentation | Microsoft Docs | Referencia de cada API |
| Wine source code | winehq.org | Implementaciones open-source |
| ReactOS | reactos.org | Reimplementación de Windows |
| NTOSKRNL internals | Various | Estructuras de kernel |

### 6.2 Para BareX API (DirectX-equivalent)

| Documento | Fuente | Uso |
|-----------|--------|-----|
| DirectX documentation | Microsoft Docs | API reference |
| vkd3d-proton | Valve/Vulkan | D3D12 → Vulkan translation |
| DXVK | doitsujin | D3D9/11 → Vulkan translation |
| Vulkan specification | Khronos | GPU API estándar |
| Naga | gfx-rs | Shader translation |

### 6.3 Para Filesystem (NTFS-inspired)

| Documento | Fuente | Uso |
|-----------|--------|-----|
| NTFS documentation | Microsoft Docs | Estructuras MFT, etc. |
| Linux NTFS driver | ntfs-3g | Implementación open-source |
| ext4 documentation | kernel.org | Diseño de filesystem |

### 6.4 Para Networking

| Documento | Fuente | Uso |
|-----------|--------|-----|
| TCP/IP Illustrated | W. Stevens | Fundamentos |
| smoltcp | smoltcp.rs | TCP/IP stack en Rust |
| lwIP | nongnu.org | TCP/IP stack ligero |

---

## PARTE 7: Comparativa Final

### Windows vs BMO/FastOS

| Componente | Windows | BMO/FastOS | Estado |
|------------|---------|------------|--------|
| Kernel | NTOSKRNL | BMO Core | 🟡 Boot OK |
| API | Win32 API | BMO API | 🔴 Planning |
| Graphics | DirectX 9-12 | BareX | 🟡 API defined |
| Audio | XAudio2/WASAPI | BareX Audio | 🟡 Structure |
| Input | DirectInput/XInput | BareX Input | 🟡 Structure |
| Filesystem | NTFS | BMO-FS | 🟡 FAT32 only |
| Networking | Winsock | BareX Net | 🟡 UDP only |
| Shell | Explorer.exe | BMO Shell | 🔴 Not started |
| Compositor | DWM | BMO Compositor | 🔴 Not started |
| Security | Defender | ByteDefender | ✅ Ring 0 |
| Recovery | System Restore | Restaurer | ✅ Ring 0 |

### Lo que BMO/FastOS tiene que Windows NO tiene

1. **ByteDefender** — Antivirus Ring 0 nativo
2. **Restaurer** — Snapshots del kernel en tiempo real
3. **EDF Scheduler** — Prioridades de tiempo real para games
4. **Kernel bypass networking** — DPDK-style para competitive gaming
5. **Sandbox nativo** — Aislamiento de procesos sin hypervisor
6. **SMP completo** — Multi-core desde el arranque
7. **FPU/SSE/AVX lazy** — Context switching optimizado

---

## Resumen

```
ESTADO ACTUAL:
✅ Boot UEFI completo
✅ GOP framebuffer 1920x1080
✅ 2D rendering
✅ SMP multi-core
✅ FPU/SSE/AVX
✅ UDP networking
✅ DHCP
✅ ICMP (ping)
✅ Demand paging + CoW
✅ EDF scheduler
✅ ByteDefender
✅ Restaurer
✅ ÑEXO language
✅ C frontend

FALTA PARA SER "GAMING OS":
❌ Context switch real
❌ Timer → scheduler
❌ Heap allocator real
❌ TCP/IP
❌ Audio output
❌ USB HID polling
❌ FAT32 write
❌ Window system
❌ GPU driver (cuando tengas)
❌ Win32 API equivalent
❌ DirectX API equivalent
❌ NTFS-like filesystem
```

---

**Última actualización**: 2026-06-16
**Versión del plan**: v1.0
**Autor**: Plan generado por opencode
