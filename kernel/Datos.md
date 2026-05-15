# 📋 Datos.md — Estado completo de FastOS para el siguiente chat

> **Lee este archivo PRIMERO al iniciar una nueva sesión.**
> Contiene todo el contexto necesario para continuar sin perder hilo.

---

## 🎯 Identidad del proyecto

- **Nombre del SO:** **FastOS** (también referido como **BMO** — Bare Metal Orchestrator).
- **Hardware target oficial:**
  - CPU: AMD **Ryzen 5 5600X** (Zen 3, 6 cores / 12 threads, 1 CCD, 32 MB L3)
  - GPU: NVIDIA **RTX 3060** (GA106, Ampere SM 8.6, 12 GB GDDR6, RT Cores Gen2, Tensor Cores Gen3)
  - Boot: **UEFI** puro (sin legacy BIOS)
- **Hardware local del usuario (para devs):**
  - Teclado USB
  - Ratón USB
  - **Headset Redragon USB** (VID `0x0C45`, USB Audio Class 2.0 + HID buttons)
- **Filosofía:** Cero legacy. Sin Win32, POSIX, DOS, COM, HRESULT, errno, wchar_t. Rust-first.
- **Lenguaje del kernel:** Rust nightly, `#![no_std]`, `#![no_main]`, `extern crate alloc`.
- **Repo:** `https://github.com/andreesalazar/fastos`

---

## 🛡️ REGLAS INMUTABLES (no romper nunca)

1. **NO TOCAR `kernel/src/drivers/gpu/fastgpu/`** — el usuario está construyendo el bridge **BMO/GSP** ahí. Cualquier modificación rompe su trabajo. La integración con BareX se hará cuando él lo indique.
2. **`cargo build` debe terminar `Finished`** antes de cerrar cualquier sesión.
3. Cada módulo nuevo lleva `#![allow(dead_code)]` mientras esté en stub.
4. Funciones no implementadas devuelven `Err(BxError::NotImplemented)` — **nunca panic**.
5. Ningún módulo nuevo se llama desde `kernel_main` hasta que esté completo.
6. Especs en `combo_Window_Extractor/MAPA de Window/` se mantienen sincronizadas en ambos paths (FastOS y SigDead).
7. **C ABI está prohibido en código nuevo.** Usar **BMO ABI** (ver §5).

---

## 📁 Estructura completa del workspace

```
c:/Users/andre/OneDrive/Documentos/FastOS/
├── boot_protocol/                ← protocolo bootloader↔kernel
├── bootloader/                   ← UEFI bootloader
├── kernel/                       ← ⭐ EL KERNEL (donde trabajamos)
│   ├── Cargo.toml                (deps: ntfs, nt-hive, binrw, volatile, bitflags, byteorder)
│   ├── linker.ld
│   ├── rust-toolchain.toml       (nightly)
│   ├── Datos.md                  ← ESTE ARCHIVO
│   └── src/
│       ├── main.rs               (kernel_main, no_std)
│       ├── boot_info.rs · console.rs · fb.rs · font.rs · panic.rs · shell.rs · allocator.rs
│       ├── arch/                 (x86_64 specifics)
│       ├── agent/
│       ├── export/
│       ├── fs/                   (NTFS via crate `ntfs`, GPT, walker)
│       ├── drivers/
│       │   ├── mod.rs
│       │   ├── pci.rs · serial.rs · nvme.rs · ahci.rs
│       │   ├── gpu/fastgpu/      ⛔ NO TOCAR (bridge BMO/GSP del usuario)
│       │   │   ├── debug/ engines/ falcon/ fw/ hw/ intelligence/
│       │   │   ├── mmio/ runtime/ sequences/ wddm/
│       │   │   └── mod.rs
│       │   └── usb/              ✅ NUESTRO (sesión 3)
│       │       ├── mod.rs        (UsbDeviceInfo, REDRAGON_VID = 0x0C45)
│       │       ├── descriptors.rs (Device/Config/Interface/Endpoint/SetupPacket)
│       │       ├── xhci.rs       (XhciController + XhciMmio + Trb/TrbType)
│       │       ├── hid.rs        (KeyboardBootReport + MouseHighResReport + HidEvent)
│       │       └── audio_class.rs (UAC2 isoch OUT, StreamFormat::REDRAGON_DEFAULT)
│       │
│       ├── barex/                ✅ API moderna (sesiones 2-6)
│       │   ├── mod.rs            (BxError, BxResult, BAREX_VERSION, HW_TARGET)
│       │   ├── _LAYERS.md        (mapa de capas)
│       │   ├── _WORK_LOG.md      (tracker de sesiones)
│       │   ├── abi/              ⭐ BMO ABI (12 sub-carpetas, ver §5)
│       │   ├── graphics/         (12 objetos núcleo BareX)
│       │   ├── audio/            (BxAudioEngine, USB AC2 + HDMI vía GSP)
│       │   ├── input/            (Key, MouseReading, HeadsetButton, BxInputSystem)
│       │   ├── net/              (BxTcpSocket, BxUdpSocket, BxQuicEndpoint)
│       │   ├── shader/           (ShaderBlob loader: SASS/SPIR-V/DXIL/DXBC)
│       │   └── compat/           (PE detection, FAKE_DLLS list)
│       │
│       ├── bef/                  ✅ Formato ejecutable (sesiones 5-6)
│       │   ├── mod.rs            (re-exports)
│       │   ├── _README.md
│       │   ├── header.rs         (BefHeader 48B, BefMagic, BefFlags, BefArch)
│       │   ├── sections.rs       (15 SectionKind, SectionTable parser)
│       │   ├── imports.rs        (ImportTable + ImportFlags)
│       │   ├── exports.rs        (ExportTable + búsqueda por hash 32-bit)
│       │   ├── relocations.rs    (3 tipos: Abs64/Rel32/Got64 + apply())
│       │   ├── symbols.rs        (Symbol + binding + visibility)
│       │   ├── manifest.rs       (Manifest + Provenance: Native/PeDevoured/ElfDevoured)
│       │   ├── signing.rs        (SectionHash + verify, usa blake3.rs)
│       │   ├── tls.rs            (TlsTemplate único, vs .tdata+.tbss de ELF)
│       │   ├── blake3.rs         ⭐ BLAKE3 real, no_std, ~250 líneas (sesión 6)
│       │   └── loader/
│       │       ├── mod.rs        (Image, LoadError, dispatcher universal)
│       │       ├── native.rs     (BEF nativo)
│       │       ├── pe.rs         ⭐ DEVOUR PE completo (sección parser)
│       │       ├── pe_imports.rs (ImageImportDescriptor, ResolvedImport)
│       │       ├── pe_thunks.rs  (75 funciones Win32 fake-DLL → BMO)
│       │       ├── elf.rs        ⭐ DEVOUR ELF completo (Phdr iter)
│       │       ├── elf_dynamic.rs (30 DT_*, parse PT_DYNAMIC)
│       │       └── elf_thunks.rs (60 funciones libc/libm/libpthread → BMO)
│       │
│       ├── sched/                ✅ Scheduler stub (Priority, CoreAffinity 5600X)
│       ├── syscall/              ✅ Syscall table stub (28 syscalls iniciales)
│       └── sandbox/              ✅ Capability bitflags (FS/NET/GFX/AUDIO/INPUT/SYS)
│
├── target/                       (build output)
├── combo_Window_Extractor/       ← MAPA de Window y herramientas (mirror en SigDead)
│   └── MAPA de Window/
│       ├── 00_INDEX.md
│       ├── 01_Windows_DNA/       (Anatomía Win11, ntoskrnl, etc.)
│       ├── 02_BEF_Format/        ⭐ specs BareX + BMO ABI + BEF
│       │   ├── BMO_ABI_Spec.md
│       │   ├── BEF_Executable_Format_Spec.md
│       │   ├── BMO_Graphics_Layer_Spec.md (L1)
│       │   ├── BareX_Shader_Pipeline.md (L2)
│       │   ├── BareX_API_Spec.md (L3 graphics)
│       │   ├── BareX_Audio_Spec.md
│       │   ├── BareX_Input_Spec.md
│       │   ├── BareX_Network_Spec.md
│       │   ├── BareX_Compat_Shim_Spec.md (L4)
│       │   ├── DX12_to_BareX_Mapping.md
│       │   └── NVK_Shader_Pipeline_Analysis.md
│       ├── 03_Kernel_Specs/      (Syscall, MM, Timers, Scheduler, Locks)
│       ├── 04_Storage/           (VFS, NVMe, Native FS)
│       ├── 05_UserSpace/         (StdLib, Rust Runtime BEF, Compositor)
│       ├── 06_Ecosystem/         (PkgMgr, Security, Sandbox)
│       └── 07_Audit/
└── (otros: build_uefi.ps1, README.md, etc.)
```

Mirror del MAPA también en: `c:/Users/andre/OneDrive/Documentos/SigDead/combo_Window_Extractor/MAPA de Window/`

---

## 📜 Historial de sesiones (orden cronológico)

### Sesión 1 — Specs maestras
Creadas 7 specs en `MAPA de Window/02_BEF_Format/`: BareX_API_Spec, BareX_Shader_Pipeline, BareX_Compat_Shim_Spec, DX12_to_BareX_Mapping, BareX_Audio_Spec, BareX_Input_Spec, BareX_Network_Spec.

### Sesión 2 — Esqueletos kernel
Creadas carpetas `barex/{graphics,audio,input,net,shader,compat}` + `bef/`, `sched/`, `syscall/`, `sandbox/`. Build verde sin tocar fastgpu.

### Sesión 3 — BMO ABI (cimiento) + USB local
- Creada spec `BMO_ABI_Spec.md` ⭐
- Creado `barex/abi.rs` (single file inicial)
- Stack USB: `drivers/usb/{mod, descriptors, xhci, hid, audio_class}.rs`
- Renombradas todas las menciones "C ABI" → "BMO ABI" en specs

### Sesión 4 — BMO ABI multi-carpeta
`barex/abi/` reorganizado en **9 sub-carpetas** (primitives, memory, string, handle, status, calling, async_io, time, compat).

### Sesión 5 — BEF devour PE/ELF + BMO ABI extensión
- BEF expandido a **9 archivos** + carpeta `loader/` con 4 archivos
- BMO ABI ampliado a **12 carpetas** (añadidas: sync, option, result)

### Sesión 6 — BLAKE3 real + thunks completos
- BLAKE3 256-bit nativo no_std (~250 líneas) en `bef/blake3.rs`
- `bef/loader/pe.rs` itera secciones reales y mapea a `MappedSection` BEF
- `bef/loader/elf.rs` itera Phdr y procesa `PT_DYNAMIC`
- 75 funciones Win32 fake-DLL en `pe_thunks.rs` → BMO
- 60 funciones POSIX/glibc en `elf_thunks.rs` → BMO

---

## 🧠 BMO ABI — referencia rápida (12 sub-carpetas en `barex/abi/`)

| Carpeta | Reemplaza C ABI |
|---|---|
| `primitives/` | `<stdint.h>`, `<stddef.h>`, `<stdbool.h>` |
| `memory/` | `void*`, `size_t`, alignment helpers |
| `string/` | `char*`, `wchar_t*`, `<string.h>` |
| `handle/` | `HANDLE`, `fd`, `IUnknown*` |
| `status/` | `HRESULT`, `errno`, `GetLastError` |
| `calling/` | convención de llamada (registros + stack) |
| `async_io/` | `OVERLAPPED`, IOCP, callbacks |
| `time/` | `time_t`, `timespec`, `GetTickCount` |
| `compat/` | thunks Win64 / SysV ↔ BMO ABI |
| `sync/` | `<stdatomic.h>`, `<threads.h>`, Interlocked*, pthread_mutex |
| `option/` | layout C-FFI estable para `Option<T>` |
| `result/` | layout C-FFI estable para `Result<T,E>` |

### Características distintivas vs C ABI
- **7 GPRs** para args int (vs 4 MS x64 / 6 SysV): RDI RSI RDX R10 R8 R9 RAX_extra
- **64 B** stack alignment (cache line Zen 3, vs 16 B C)
- **0 B** shadow space (vs 32 B MS x64)
- **256 B** red zone (vs 0 / 128 C)
- **`BmoStatus` 16 B en RAX:RDX** como retorno universal (sin HRESULT/errno globals)
- **`BmoHandle` 64-bit con generación 16-bit** (UAF detectado por construcción)
- **UTF-8 universal** (sin wchar_t / UTF-16)
- **SQ/CQ rings** io_uring-style para async (sin OVERLAPPED/IOCP/callbacks)

### Tipos clave a recordar
```rust
BmoStatus { code: u32, flags: u32, value: u64 }           // 16 B en RAX:RDX
BmoHandle(u64)  // tag(1) | kind(7) | generation(16) | index(40)
BmoSlice { ptr: *const u8, len: u64 }                     // 16 B en 2 GPRs
BmoStr<'a>, BmoString                                     // UTF-8 + len explícita
BmoInstant { ns_since_boot: u64 }                         // monotónico
BmoDuration { ns: u64 }
BmoAtomicU32/U64/Bool, BmoMutex, BmoFutex, MemOrder
BmoOption<T>, BmoResult<T>                                // FFI-safe
```

---

## 🎮 BareX — capas

```
L4   barex/compat        →  PE loader + thunks DX/COM/Win32 (apps Windows)
L3   barex/graphics      →  API gráfica (12 objetos núcleo, hereda DX12 Ultimate)
     barex/audio         →  bx_audio (USB Redragon + HDMI vía GSP)
     barex/input         →  bx_input (USB HID directo < 0.5 ms)
     barex/net           →  bx_net (TCP/UDP/QUIC propio, sin Winsock)
L2   barex/shader        →  HLSL/DXIL/DXBC/SPIR-V → SASS GA106
     barex/abi           →  BMO ABI (cimiento de TODO arriba)
L1   drivers/gpu/fastgpu →  ⛔ Bridge BMO/GSP del usuario (no tocar)
     drivers/usb/*       →  xHCI + HID + USB Audio Class
     drivers/{nvme,ahci,pci,serial}
```

### 12 objetos núcleo de `barex::graphics` (= DX12 destilado)
`BxDevice`, `BxQueue`, `BxCmdList`, `BxPso`, `BxRootSig`, `BxGlobalHeap`, `BxFence`, `BxSwapchain`, `BxBuffer`, `BxTexture`, `BxSampler`, `BxQueryHeap`.

Hereda DX12 Ultimate + Agility 1.614: DXR 1.2, Mesh Shaders, VRS Tier 2, Sampler Feedback, Work Graphs 1.0, GPU Upload Heaps (ReBAR), Enhanced Barriers (único modo permitido), Bindless puro (modelo SM 6.6 ResourceDescriptorHeap).

---

## 📦 BEF — formato ejecutable que devora PE y ELF

### Detector universal
```
bef::loader::load(bytes) → BefMagic::detect() → 
   "BEF1"      → loader::native (BEF nativo)
   "MZ"        → loader::pe     (PE Windows .exe/.dll)
   0x7F"ELF"   → loader::elf    (ELF Linux/Unix)
```
Todos producen el mismo `Image { format, manifest, entry_point, base_address, sections }`.

### 15 SectionKind BEF
Code, RoData, Data, Bss, Imports, Exports, Relocs, Symbols, Manifest, Shaders, Resources, Tls, Unwind, Debug, Signature.

### 3 Relocation types BEF (vs 38 ELF / 16 PE)
`Abs64`, `Rel32`, `Got64`. La función `bef::relocations::apply()` ya las aplica.

### Devour Win32 (75 funciones mapeadas en `pe_thunks::THUNK_TABLE`)
- `kernel32.dll` (32): ExitProcess, VirtualAlloc, CreateFileW, GetTickCount, GetModuleHandle, etc.
- `user32.dll` (12): MessageBoxW, CreateWindowExW, GetMessageW...
- `ntdll.dll` (11): NtAllocateVirtualMemory, NtReadFile, NtClose...
- `d3d12.dll`/`dxgi.dll` (4): → BareX graphics
- `xinput1_4.dll` (3): → BareX input
- `xaudio2_9.dll` (1): → BareX audio
- `ws2_32.dll` (10): → BareX net

### Devour Linux (60 funciones mapeadas en `elf_thunks::THUNK_TABLE`)
- `libc.so` (50): malloc/free/mmap, open/read/write, pthread_*, time, strings
- `libm.so` (9): sin/cos/sqrt/pow/log/exp/floor/ceil
- `libpthread.so` (4): create/join/self/exit
- `libdl.so` (3): dlopen/dlsym/dlclose

`normalize_lib_name()` convierte `libc.so.6`/`libc-2.31.so`/`/lib64/libc.so.6` → `libc.so`.

---

## 🎧 Hardware local del usuario

| Componente | Driver kernel | Capa BareX |
|---|---|---|
| Teclado USB | `drivers::usb::hid` (Boot Protocol) | `barex::input::Key` (HID Usage Page 0x07) |
| Ratón USB | `drivers::usb::hid` (Report HighRes 16-bit) | `barex::input::MouseReading` |
| Headset Redragon (VID 0x0C45) | `drivers::usb::audio_class` (UAC2 isoch) + `hid` | `barex::audio` + `barex::input::HeadsetButton` |
| GPU RTX 3060 | `drivers::gpu::fastgpu` ⛔ (usuario) | `barex::graphics` |
| NVMe SSD | `drivers::nvme` | `bef::load` + `barex::audio::stream_from_file` |

`StreamFormat::REDRAGON_DEFAULT` = 48 kHz / 16-bit / stereo / 192 B por isoch frame (1 ms HighSpeed).

---

## 🔜 Pendientes priorizados

1. **Bridge BMO/GSP** — lo lleva el usuario en `drivers/gpu/fastgpu/`. NO TOCAR.
2. Conectar `barex::graphics::BxDevice::primary()` al bridge cuando esté listo.
3. Implementar `arch::x86_64::tsc::read_ns()` para que `BmoInstant::now()` cobre vida.
4. **Pipeline completo del loader BEF nativo** (relocs + imports + tls + sandbox).
5. **Wirear thunks PE al IAT real** — escribir direcciones de `pe_thunks::THUNK_TABLE` en runtime.
6. **Wirear thunks ELF al GOT real** — usar `elf_dynamic::DynamicInfo` para mapear DT_NEEDED → resolver.
7. Localizar `IMAGE_DIRECTORY_ENTRY_IMPORT` real en PE (actualmente usa heurística).
8. `xhci::probe()` real — detectar host controller del chipset 500-series.
9. Loop de poll HID que rellene la cola de `barex::input`.
10. Stream isoch OUT al headset Redragon vía `usb::audio_class::submit_pcm`.
11. Dispatcher real en `syscall::dispatch` con BMO ABI extendido.
12. Implementar `tls::setup_for_thread()` con `WRMSR IA32_FS_BASE`.
13. Parser TOML real para `bef::manifest::Manifest`.
14. Ed25519 signature verification en `bef::signing` (BLAKE3 ya hecho).
15. Test del BLAKE3 contra el vector oficial: `blake3("abc")` debe dar `6437b3ac38465133ffb63b75273a8db548c558465d79db03fd359c6cd5bd9d85`.

---

## 📊 Tabla de archivos creados (por sesión)

| Sesión | Archivos nuevos |
|---|---|
| 1 | 7 specs en MAPA |
| 2 | 11 archivos kernel (barex/* + bef + sched + syscall + sandbox) |
| 3 | 5 USB drivers + abi.rs single file + BMO_ABI_Spec.md |
| 4 | 24 archivos en `barex/abi/` (9 sub-carpetas) + READMEs |
| 5 | 13 archivos BEF (9 + loader/4) + 7 archivos BMO ABI extension (sync/option/result) |
| 6 | 5 archivos BEF avanzados (blake3, pe_imports, pe_thunks, elf_dynamic, elf_thunks) + actualización pe.rs/elf.rs |

**Total acumulado:** ~75 archivos `.rs` nuevos + 4 `_README.md` + 1 `_WORK_LOG.md` + 11 specs en MAPA.

---

## 🔧 Comandos útiles (PowerShell desde `kernel/`)

```powershell
# Build
cargo build

# Lista archivos modificados recientemente
Get-ChildItem -Path src -Recurse -Filter "*.rs" | Where-Object { $_.LastWriteTime -gt (Get-Date).AddHours(-2) } | Select-Object FullName, Length

# Búsqueda de texto
Select-String -Path src/**/*.rs -Pattern "TODO" -Recurse
```

---

## ⚙️ Configuración del sistema operativo donde se desarrolla

- OS dev: Windows 10/11 (PowerShell — usar `Select-String`, no `grep`/`rg`)
- Working directory por defecto: `c:/Users/andre/OneDrive/Documentos/FastOS`
- Path absolutes con `c:/...` (forward slash funciona en PS para paths)
- Para `cargo`, usar `cwd: "c:/Users/andre/OneDrive/Documentos/FastOS/kernel"`
- Cargo.toml depende de: `fastos-boot-protocol`, `ntfs 0.4`, `nt-hive 0.3`, `binrw 0.11`, `volatile 0.4`, `bitflags 2`, `byteorder 1`

### Sintaxis bitflags 2.x (importante)
```rust
bitflags::bitflags! {
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]    // ← derives van AQUÍ DENTRO
    pub struct MyFlags: u32 {
        const FOO = 1 << 0;
    }
}
```

---

## 🎬 Cómo continuar en la próxima sesión

1. **Lee este `Datos.md` primero.**
2. Verifica el estado: `cargo build` desde `kernel/` debe terminar `Finished`.
3. Confirma que `drivers/gpu/fastgpu/` no tiene cambios sospechosos.
4. Pregunta al usuario qué pendiente abordar (lista en §"Pendientes priorizados").
5. Trabaja en la sesión y al final actualiza este `Datos.md` con la nueva sesión.

### Protocolo de actualización de Datos.md
Cuando termines una sesión:
- Añade la sesión nueva en el "Historial de sesiones".
- Actualiza la "Estructura completa del workspace" si creaste/borraste carpetas.
- Tacha o elimina pendientes completados.
- Añade nuevos pendientes que descubriste.
- Verifica que `cargo build` esté verde.

---

**Última actualización:** Sesión 6 (BLAKE3 real + thunks completos PE/ELF).
**Estado del kernel:** `cargo build` Finished ✅ — fastgpu intacto.
