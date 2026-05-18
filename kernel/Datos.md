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

## 📌 SNAPSHOT EJECUTIVO (post-Sesión 16) — léeme antes que nada

### Estado en una línea
`cargo build` **Finished ✅** · fastgpu intacto · 16 sesiones cerradas · ~259 archivos `.rs` nuevos · 17 sub-sistemas modularizados.

### Sub-sistemas del kernel — estado actual de cada uno

| Carpeta `src/` | Sub-carpetas | Archivos `.rs` | Estado | Sesión |
|---|---|---|---|---|
| `barex/abi/` | **19** | ~50 | ⭐ BMO ABI completo + 7 multi-lenguaje | 3,4,5,7 |
| `barex/abi/runtime.rs` | (top) | 1 | ⭐ BmoRuntime agregador | 8 |
| `barex/graphics/` | **13** | 17 | Modularizado, 1 carpeta por objeto | 13 |
| `barex/audio/` | **10** | 39 | Modularizado zero-bloat | 11 |
| `barex/input/` | **10** | 39 | Modularizado zero-bloat | 12 |
| `barex/net/` | **9** | 32 | Modularizado zero-bloat | 10 |
| `barex/shader/` | **9** | 9 | Fachada (delega a naga/vkd3d/dxvk) | 14 |
| `barex/compat/` | 1 | 1 | ⏳ pendiente modularización | — |
| `barex/bmoasm/` ⭐ | **7** | 15 | BMO Simple v2 — lexer DFA real | 15,16 |
| `bef/` | 1 + `loader/9` | 18 | BEF format + 5 loaders + BLAKE3 | 5,6,9 |
| `drivers/usb/` | 1 | 5 | xHCI + HID + Audio Class | 3 |
| `drivers/gpu/fastgpu/` | **10** | — | ⛔ INTOCABLE (bridge BMO/GSP usuario) | — |

### Estructura completa actual del kernel/src

```
kernel/src/
├── main.rs · boot_info.rs · console.rs · fb.rs · font.rs · panic.rs · shell.rs · allocator.rs
├── arch/ · agent/ · export/ · fs/
├── sched/ · syscall/ · sandbox/        ← stubs activos
│
├── drivers/
│   ├── mod.rs · pci.rs · serial.rs · nvme.rs · ahci.rs
│   ├── gpu/fastgpu/                    ⛔ NO TOCAR (bridge BMO/GSP del usuario)
│   └── usb/                            ✅ {mod, descriptors, xhci, hid, audio_class}
│
├── barex/                              ✅ API moderna
│   ├── mod.rs · _LAYERS.md · _WORK_LOG.md
│   ├── abi/                            ⭐ BMO ABI (19 sub-carpetas, runtime.rs + _README.md)
│   │   ├── primitives/  memory/  string/  handle/  status/  calling/
│   │   ├── async_io/    time/    compat/  sync/    option/   result/
│   │   ├── type_system/ vtable/  closure/ exception/ reflect/
│   │   ├── lang_bridge/ marshal/
│   │   └── runtime.rs                  ← BmoRuntime (agregador único)
│   │
│   ├── graphics/                       (13 sub: types, device, queue, cmdlist, pso, rootsig,
│   │                                    heap, fence, swapchain, buffer, texture, sampler, queryheap)
│   ├── audio/                          (10 sub: capabilities, format, engine, voice, mixer,
│   │                                    codec, spatial, effects, route, backend, ring)
│   ├── input/                          (10 sub: capabilities, system, device, keyboard, mouse,
│   │                                    headset, gamepad, wheel, hid_raw, keymap, event, ring)
│   ├── net/                            (9 sub: capabilities, types, socket, quic, tls, http,
│   │                                    dns, ring, driver, bypass)
│   ├── shader/                         (9 sub: stage, ir, sass, spirv, dxil, dxbc, loader, cache)
│   ├── compat/                         (PE detection — pendiente modularización)
│   └── bmoasm/                         ⭐ BMO Simple v2 (lexer, parser, sema, emit,
│                                        runtime, builtin, sample)
│
└── bef/                                ✅ Formato ejecutable BEF
    ├── header.rs · sections.rs (20 SectionKind) · imports.rs · exports.rs
    ├── relocations.rs (3 tipos) · symbols.rs · manifest.rs · signing.rs · tls.rs
    ├── blake3.rs                       ⭐ BLAKE3 256 nativo no_std
    └── loader/                         (native, pe, pe_imports, pe_thunks 75 funcs,
                                         elf, elf_dynamic, elf_thunks 60 funcs,
                                         meta_sections ← parser 5 secciones meta-genéricas)
```

### Cómo pensar en cada sub-sistema (mapa mental rápido)

```
                          ╭───────────────────╮
                          │   BMO ABI (abi/)  │ ←─ cimiento absoluto
                          │   BmoRuntime      │    cualquier código nuevo
                          ╰─────────┬─────────╯    consume estos tipos
                                    │
       ┌──────────────┬─────────────┼─────────────┬──────────────┐
       ▼              ▼             ▼             ▼              ▼
   graphics/      audio/        input/         net/         shader/
   (12 obj)      (engine+      (HID +        (TCP/UDP/    (delegate
   →fastgpu      mixer→        keymap→       QUIC/TLS13/  naga, vkd3d,
                 USB AC2)      device)       HTTP3, ring) dxvk)
                                                              │
                                                              ▼
                                                          drivers/gpu/fastgpu ⛔

                          ╭───────────────────╮
                          │   bef/  (formato) │ ←─ unifica BEF nativo,
                          │   loader/         │    PE Windows, ELF Linux
                          ╰─────────┬─────────╯
                                    │
                                    ▼
                          ╭───────────────────╮
                          │  bmoasm/ (BMO     │ ←─ lenguaje propio,
                          │  Simple)          │    keywords español,
                          ╰───────────────────╯    bytes precisos x86-64
```

### BMO ABI — 19 sub-carpetas en `barex/abi/`

**Cimiento (12):** `primitives memory string handle status calling async_io
time compat sync option result`.

**Multi-lenguaje genérico (7):** `type_system vtable closure exception reflect
lang_bridge marshal`. Permite que cualquier lenguaje futuro (Rust, C++, Java,
Swift, Python, Go, OCaml, Lua, Haskell, BEAM, Mojo, Carbon, Vale…) se integre
nativamente sin C ABI: registra `LangDescriptor` + opcional marshaller → corre.

**Agregador:** `runtime.rs` con `BmoRuntime` (reemplaza el patrón C de globals
dispersos `__cxa_*`/`__libc_*`/`KeServiceDescriptorTable`).

### BEF — 20 `SectionKind` (15 base + 5 meta-genéricas Sesión 8)

`Code RoData Data Bss Imports Exports Relocs Symbols Manifest Shaders
Resources Tls Unwind Debug Signature` **`TypeMap VTables LangBridge Reflect
Closures`**. El parser `bef/loader/meta_sections.rs` ya los materializa en un
`BmoRuntime` real.

### BMO Simple (`bmoasm/`) — estado actual

- **Lexer DFA real** (sesión 16): reconoce identifiers, decimal/hex/binary
  literals con `_` separators, comentarios `// ... \n`, lookahead 2-char `->`,
  delimitadores. Tabla `KEYWORDS` con **89 entries** greedy.
- **95 keywords** en `TokenKind`. Categorías: base (10), tipos (5), ops (16),
  control de flujo (15), OOP fase 2 (8), UI fase 3 (3), intrínsecos CPU (12),
  memoria/consistencia (8), vectorización (3), directivas (6), CPU flags (6),
  léxico (13).
- **12 intrínsecos** con **bytes exactos** (`pausa`→`F3 90`, `cpuid`→`0F A2`,
  `mfence`→`0F AE F0`, `syscall`→`0F 05`, etc). `align_nops(N)` con multi-byte
  NOPs recomendados Intel.
- **5 programas `.bmo`** de muestra en `sample/`: EXIT_ZERO, SPIN_LOCK
  (atomico+pausa+cuando zf), MEDIR_CICLOS (rdtsc), TABLA_SALTO (match+caso),
  ALIGN_FUNCION (paralelo+para+cerca).
- **Emit encoder operativo**: `mov_reg_imm64` (REX.W/REX.B), `ret`, `syscall`,
  `nop`, `emit_raw`. Calling convention BMO: 7 GPR (`RDI RSI RDX R10 R8 R9
  RAX_extra`) en `BMO_ARG_REGS`.
- **Pendiente**: parser real (S17, ~300 líneas), sema real (S18), codegen
  full AST→x86 (S19), OOP (`tipo`/`impl`/`nuevo`) (S20), UI (`ventana`/
  `evento`/`dibuja`) (S21).

### Reglas críticas (recordatorio rápido)

1. ⛔ **NO TOCAR `drivers/gpu/fastgpu/`** (bridge BMO/GSP del usuario).
2. ✅ `cargo build` debe terminar `Finished`.
3. ✅ Funciones no impl → `Err(BxError::NotImplemented)` (nunca panic).
4. ✅ Cada módulo nuevo: `#![allow(dead_code)]` mientras esté en stub.
5. ✅ Nuevo código → **BMO ABI**, nunca C ABI.
6. ✅ Specs en `combo_Window_Extractor/MAPA de Window/` sincronizadas en FastOS y SigDead.

### Top-5 pendientes prioritarios (post-S16)

1. **BMO Simple S17** — Parser recursive-descent operativo (~300 líneas).
2. **`barex/compat/`** — única carpeta `barex/*` aún monolítica.
3. **`arch::x86_64::tsc::read_ns()`** — hace vivir a `BmoInstant::now()`.
4. **Pipeline loader BEF nativo** — wirear relocs+imports+TLS+sandbox tras meta_sections.
5. **`xhci::probe()`** real — para que poll HID rellene `barex::input::ring`.

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
│       │   ├── abi/              ⭐ BMO ABI (19 sub-carpetas, ver §5)
│       │   ├── graphics/         (modularizado S13: 13 sub-carpetas, 1 por objeto núcleo — types, device, queue, cmdlist, pso, rootsig, heap, fence, swapchain, buffer, texture, sampler, queryheap)
│       │   ├── audio/            (modularizado S11: 10 sub-carpetas / 39 archivos — format, engine, voice, mixer, codec, spatial, effects, route, backend, ring)
│       │   ├── input/            (modularizado S12: 10 sub-carpetas / 39 archivos — device, keyboard, mouse, headset, gamepad, wheel, hid_raw, keymap, event, ring)
│       │   ├── net/              (BxTcpSocket, BxUdpSocket, BxQuicEndpoint)
│       │   ├── shader/           (modularizado S14: 9 sub-carpetas — stage, ir, sass, spirv, dxil, dxbc, loader, cache; delega a naga / vkd3d-shader-rs / dxvk-spirv-rs)
│       │   ├── compat/           (PE detection, FAKE_DLLS list)
│       │   └── bmoasm/           ⭐ S15+S16: lenguaje propio (lexer DFA real + parser + sema + emit + runtime + builtin + sample) — 95 keywords semánticos, 12 intrínsecos con bytes exactos
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
│       │       ├── elf_thunks.rs (60 funciones libc/libm/libpthread → BMO)
│       │       └── meta_sections.rs ⭐ (parser TypeMap/VTables/LangBridge/Reflect/Closures + build_runtime)
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

### Sesión 7 — BMO ABI genérico multi-lenguaje ⭐
Añadidas **7 nuevas sub-carpetas** a `barex/abi/` (ahora **19 total**) para
que cualquier lenguaje (Rust, C++, Java, Swift, Python, Go, JS, OCaml,
Haskell, Erlang, Lua, futuros) se integre nativamente sin C ABI:

- `type_system/` (5 archivos): `TypeDescriptor`, `TypeKind` (21 variantes),
  `TypeLayout` + `LayoutFlags` (8 flags POD/SEND/SYNC/REPR_C/...),
  `TypeRegistry`, `TypeId` (BLAKE3 truncado 64-bit). Reemplaza RTTI / `Type` / `class`.
- `vtable/` (4 archivos): `BmoVTable` con magic `b"BVT1"`, `VTableEntry`
  (5 kinds), `BmoFatPtr` (16 B en RAX:RDX), `query_interface`. Reemplaza
  vtables C++ / COM / `dyn Trait` / Java iface / Go itab.
- `closure/` (3 archivos): `BmoClosure` 32 B, `ClosureEnv`, `ClosureSig`
  (Pure/Mut/Once). Algo que C ABI **no tiene**.
- `exception/` (4 archivos): `BmoPanic` + `PanicKind` (6), `UnwindContext` +
  `UnwindReason` (5), `UnwindAction` (4), `ResumeToken` (resumable
  exceptions, estilo Common Lisp / OCaml 5), `UnwindTable` compacta.
  Reemplaza Itanium EH / SEH / managed exceptions.
- `reflect/` (2 archivos): `Mirror` / `MirrorOf` estilo Strongtalk,
  `ReflectQuery`. Reemplaza Java reflection / .NET / Go reflect / `inspect`.
- `lang_bridge/` (4 archivos): `LangDescriptor` con versión y features,
  `LangRegistry`, **26 IDs de lenguaje** (Rust, C, C++, Zig, Swift, JVM,
  CLR, Python, JS, Go, OCaml, Lua, Haskell, BEAM, Nim, Crystal, Dart,
  Kotlin, Ruby, PHP, Fortran, Ada, Racket, Scheme, Clojure + slot
  `LANG_FUTURE_*`), `LangFeatures` (14 flags: GC, exceptions, closures,
  effects, ownership, lazy, ...).
- `marshal/` (4 archivos): `Marshaller` trait, `MarshalError`, helpers de
  boxing/unboxing, estimadores UTF-8↔UTF-16, bool-marshal Win32↔BMO.

`cargo build` Finished ✅ (solo warnings de unused imports — todos los
módulos son stubs aún sin consumidor llamándolos).

### Sesión 8 — Wiring BEF ↔ BMO ABI genérico ⭐
Conectado el cimiento de la sesión 7 al formato ejecutable y al kernel:

- **`bef/sections.rs`**: `SectionKind` extendido de 15 → **20 variantes**.
  Nuevas: `TypeMap` (0x10), `VTables` (0x11), `LangBridge` (0x12),
  `Reflect` (0x13), `Closures` (0x14). Cualquier compilador BEF puede ya
  emitir metadatos para los módulos de sesión 7.
- **`barex/abi/runtime.rs`** ⭐: nuevo `BmoRuntime` que agrega
  `TypeRegistry`, `LangRegistry`, slice de `BmoVTable` y `UnwindTable`
  en una sola estructura `repr(C)`. Reemplaza el patrón C de "globals
  dispersos" (`__cxa_*`, `__libc_*`, `KeServiceDescriptorTable`).
  Provee `stats()`, `type_of()`, `lang_of()`, `reflect()`.
- **`barex/abi/_README.md`** ⭐: documentación maestra de las 19
  sub-carpetas con mapa visual y guía "cómo añadir un nuevo lenguaje".
- **Re-exports planos en `barex/abi/mod.rs`**: ahora `use crate::barex::abi::*;`
  trae `BmoRuntime`, `TypeDescriptor`, `BmoVTable`, `BmoFatPtr`,
  `BmoClosure`, `BmoPanic`, `LangDescriptor`, `Marshaller`, etc. directo.

`cargo build` Finished ✅.

### Sesión 9 — Parser meta-secciones BEF + materialización ⭐
Cierra el ciclo: las 5 nuevas `SectionKind` ahora se leen del archivo:

- **`bef/loader/meta_sections.rs`** ⭐ (~180 líneas): localiza las 5
  secciones (`TypeMap`/`VTables`/`LangBridge`/`Reflect`/`Closures`) +
  `Unwind` en una `SectionTable`, valida offsets, expone `MetaSectionViews`
  zero-copy. Provee `type_descriptors_from()` y `lang_descriptors_from()`
  (reinterpretación segura `&[u8]` → `&[TypeDescriptor]`/`&[LangDescriptor]`
  con chequeo de tamaño y alineación). `build_runtime()` construye un
  `BmoRuntime` real desde un `MetaSectionViews`. `meta_stats()` para shell/debug.
- **`bef/loader/native.rs`**: pipeline de carga BEF nativo ahora invoca
  `parse_meta_sections()` después del section table. Si las secciones
  meta están ausentes, sigue funcionando (BEF "puramente código" válido);
  si están malformadas, devuelve `LoadError::InvalidHeader`.

`cargo build` Finished ✅. **Total módulos `bef/loader/`: 9 archivos.**

### Sesión 10 — `barex/net` modularizado zero-bloat ⭐
`net/` pasó de **1 archivo (56 líneas)** → **9 sub-carpetas + 32 archivos**
con stack agresivo, eliminando todo el bloat típico de Windows/Linux:

- `mod.rs` — re-exports + `BxNetService` + `Protocol` (TCP/UDP/QUIC/TLS13/HTTP2/HTTP3/WS/WT/Raw).
- `capabilities.rs` — `NetCapabilities` con 8 flags (OUTBOUND, INBOUND,
  RAW_KERNEL_BYPASS, QUIC, MULTICAST, RAW_SOCKETS, PRIVILEGED_PORTS,
  CUSTOM_DNS).
- `types/` (5 archivos): `IpAddr`/`IpV4`/`IpV6`, `Endpoint` (24 B vs 128 B
  de `sockaddr_storage`), `MacAddr`, `Cidr`, `Port` (typed con constantes
  HTTP/HTTPS/DNS/DOH/DOT/QUIC).
- `socket/` (3 archivos): `BxTcpSocket`, `BxUdpSocket`, `SocketState`
  (FSM TCP completa + Bound UDP).
- `quic/` (2 archivos): `BxQuicEndpoint` (0-RTT/1-RTT), `BxQuicStream` +
  `QuicStreamId` (62-bit).
- `tls/` (2 archivos): `TlsContext` cliente/servidor, `TlsCipherSuite`
  con las **únicas 5** del RFC 8446 (TLS 1.3 only — sin SSLv3/1.0/1.1/1.2).
- `http/` (3 archivos): `Http3Client`, `Http3Server`, `HttpVersion` con
  `alpn()`. HTTP/1.x **prohibido** por design.
- `dns/` (2 archivos): `DnsResolver` (DoH **o** DoT), `DnsAnswer`.
  Sin `getaddrinfo`, sin /etc/hosts, sin UDP/53 plano.
- `ring/` (3 archivos): `NetSqe` 64 B (10 ops), `NetCqe` 32 B,
  `NetSubmissionQueue`/`NetCompletionQueue` SPSC lock-free.
  Reemplaza IOCP / epoll / kqueue / `WSAOVERLAPPED`.
- `driver/` (2 archivos): `NicDriver` trait, `NicCapabilities` con
  `NicOffloads` (12 flags: TX_CKSUM_*, TSO_V4/V6, LRO, RSS, SR_IOV,
  ZERO_COPY, QUIC_OFFLOAD).
- `bypass/` (1 archivo): `BypassRing` DPDK/AF_XDP-style para HFT/gaming.

**Bloat eliminado:** Winsock + WSAStartup + `sockaddr_in/in6` zoo + `int fd` +
`errno`/`WSAGetLastError` + OpenSSL + SChannel + NetBIOS + WPAD + SMB +
`getaddrinfo` + epoll + kqueue + IOCP + NDIS + AF_PACKET + libcurl + WinHTTP.

`cargo build` Finished ✅.

### Sesión 11 — `barex/audio` modularizado zero-bloat ⭐
`audio/` pasó de **1 archivo (158 líneas)** → **10 sub-carpetas + 39 archivos**.
Cada concern en su carpeta dedicada — **sin monolitos**.

- `mod.rs` — re-exports + versión + `REDRAGON_DEFAULT_SR`.
- `capabilities.rs` — `AudioCapabilities` (8 flags: PLAYBACK, CAPTURE,
  EXCLUSIVE_MODE, SPATIAL, HEAVY_DSP, REALTIME, MIDI, LOOPBACK).
- `format/` (3 archivos): `SampleFormat` (I16/I24/I32/F32/F64),
  `ChannelLayout` (Mono..Surround916), `LatencyTier` (Realtime 0.7ms..Power 10.7ms).
- `engine/` (3 archivos): `BxAudioEngine`, `EngineMode` (ExclusiveOrShared/Shared/Exclusive),
  `AudioBackend` (None/UsbAc2/HdmiGsp/RealtekHda).
- `voice/` (1 archivo): `BxVoice` (volume/pitch/pan/loop, play/stop/pause/resume).
- `mixer/` (1 archivo): `BxMixer` software (master_volume, active_voices).
- `codec/` (4 archivos): `CodecKind`, `PcmDecoder`, `OpusDecoder`, `VorbisDecoder`.
- `spatial/` (2 archivos): `BxSpatializer` HRTF, `ListenerPose` (pos/forward/up).
- `effects/` (5 archivos): `EffectKind` (8 tipos), `BxEq` 10 bandas, `BxReverb`,
  `BxCompressor`, `BxLimiter` brick-wall.
- `route/` (2 archivos): `Endpoint` + `EndpointKind` (7), `Router` enumeración.
- `backend/` (4 archivos): `Backend` trait, `UsbAc2Backend`, `HdmiGspBackend`
  (depende de bridge BMO/GSP), `RealtekHdaBackend`.
- `ring/` (3 archivos): `AudioSqe` 64 B (6 ops), `AudioCqe` 32 B,
  `AudioSubmissionQueue`/`AudioCompletionQueue` SPSC.

**Bloat eliminado:** WASAPI + IAudioClient COM + DirectSound + XAudio2 + ASIO
driver-by-driver + CoreAudio HAL + ALSA + PulseAudio + JACK + MMDevice COM +
KMixer + APO chain + `WAVEFORMATEX` zoo + Media Foundation Transforms +
DirectShow filters + GStreamer plugins.

`cargo build` Finished ✅.

### Sesión 12 — `barex/input` modularizado zero-bloat ⭐
`input/` pasó de **1 archivo (203 líneas)** → **10 sub-carpetas + 39 archivos**.

- `mod.rs` + `capabilities.rs` (9 flags) + `system.rs` (BxInputSystem singleton).
- `device/` (2): `DeviceKind` (14 tipos), `DeviceInfo` (VID/PID/poll_rate/bus).
- `keyboard/` (3): `Key` (USB HID Usage Page 0x07, ~80 keycodes), `Modifiers`
  con helpers `any_ctrl/shift/alt/gui`, `KeyboardReading` (6-key rollover).
- `mouse/` (3): `MouseButtons` (8 botones), `MouseReading` raw deltas
  (sin aceleración del SO), `CursorMode` (Visible/Hidden/Captured/Confined).
- `headset/` (2): `HeadsetButton` (7 botones del Redragon), `HeadsetButtonEvent`.
- `gamepad/` (6): `GamepadFamily`, `GamepadButtons` (18 bits canónicos
  S/E/W/N/...), `GamepadReading` con accel+gyro, mapeos `xbox` (Microsoft VID
  0x045E), `playstation` (Sony 0x054C: DS4/DualSense/Edge), `switch` (Nintendo
  0x057E: Pro/Joy-Con L/R).
- `wheel/` (1): `WheelReading` con steer/throttle/brake/clutch/handbrake/
  rudder/throttle_lever para HOTAS y volantes.
- `hid_raw/` (2): `HidUsagePage` (12 páginas IANA), `HidReportItem` parser.
- `keymap/` (2): `Layout` (9 layouts: US/ES/UK/DE/FR/JP/Dvorak/Colemak),
  `KeymapEntry` con plain/shift/altgr.
- `event/` (2): `InputReading` snapshot por frame, `InputEvent`+`InputEventKind`
  (12 tipos discretos: KeyDown/Up, MouseMove, GamepadAxis, DevicePlugged...).
- `ring/` (3): `InputSqe` 64B con 5 ops (Subscribe/Unsubscribe/Inject/Rumble/
  SetCursorMode), `InputCqe` 32B, queues SPSC.

**Bloat eliminado:** RawInput `WM_INPUT` + DirectInput 8 COM + XInput
4-gamepad cap + Windows.Gaming.Input WinRT + `LoadKeyboardLayout` HKL +
`GetAsyncKeyState` + WndProc message loop + aceleración mouse del kernel +
Win32 IME/TSF + GLFW callbacks + SDL2 polling + X11/Wayland event queues.

`cargo build` Finished ✅.

### Sesión 13 — `barex/graphics` modularizado (una carpeta por objeto) ⭐
`graphics/` pasó de **1 archivo (183 líneas)** → **13 sub-carpetas + 17 archivos**.
Solo firmas BMO ABI — NO duplica trabajo de NAGA / fastgpu. Minimalista.

- `mod.rs` — re-exports.
- `types/` (3): `Format`, `MemoryHint`, `BxBarrier`+`Sync`+`Access`+`Layout`.
- `device/` — (1) `BxDevice::primary()` (sin DXGI adapter enum).
- `queue/` — (2) `BxQueue` + `QueueKind` (Graphics/Compute/Copy/Video×2).
- `cmdlist/` — (3) `BxCmdList` (allocator interno, sin `ID3D12CommandAllocator`).
- `pso/` — (4) `BxPso` unificado (graphics/compute/RT/mesh/work-graph).
- `rootsig/` — (5) `BxRootSig` (default via reflexión SPIR-V/DXIL).
- `heap/` — (6) `BxGlobalHeap` bindless (SM 6.6 `ResourceDescriptorHeap`).
- `fence/` — (7) `BxFence` timeline-style.
- `swapchain/` — (8) `BxSwapchain` (compositor FastOS, sin DXGI).
- `buffer/` — (9) `BxBuffer`.
- `texture/` — (10) `BxTexture`.
- `sampler/` — (11) `BxSampler`.
- `queryheap/` — (12) `BxQueryHeap`.

Cada objeto en su carpeta para que cuando llegue la integración real con
`drivers::gpu::fastgpu` (cuando el usuario termine el bridge BMO/GSP),
se modifique sólo la carpeta del objeto correspondiente sin afectar al resto.

`cargo build` Finished ✅.

### Sesión 14 — `barex/shader` modularizado (delega a NAGA / vkd3d) ⭐
`shader/` pasó de **1 archivo (51 líneas)** → **9 sub-carpetas + 9 archivos**.
Pura **fachada BMO** — toda traducción real se delega a crates Rust existentes
(naga, vkd3d-shader-rs, dxvk-spirv-rs). Cero re-implementación.

- `mod.rs` — re-exports.
- `stage/` — `ShaderStage` (12: Vertex/Pixel/Compute/Mesh/Amp/RT×6/WorkGraph).
- `ir/` — `ShaderIr` (SassGa106/SpirV16/Dxil/Dxbc) + `ShaderBlob`.
- `sass/` — upload directo al GSP (sin traducción; bridge BMO/GSP del usuario).
- `spirv/` — `translate_to_sass()` delega a **naga + NAK**.
- `dxil/` — `translate_to_spirv()` delega a **vkd3d-shader-rs**.
- `dxbc/` — `translate_to_spirv()` delega a **dxvk-spirv-rs**.
- `loader/` — `load()` dispatcher: match por IR → llama al sub-módulo.
- `cache/` — `ShaderCache` LRU con key BLAKE3 (evita re-traducir mismo blob).

Pipeline:
```
SassGa106 → sass::upload
SpirV16   → spirv::translate_to_sass(naga) → sass::upload
Dxil      → dxil::translate_to_spirv(vkd3d) → spirv → sass
Dxbc      → dxbc::translate_to_spirv(dxvk) → spirv → sass
```

`cargo build` Finished ✅.

### Sesión 15 — **BMO Simple** (lenguaje propio, sintaxis español, emite bytes) ⭐
Nueva carpeta `barex/bmoasm/` — un mini-lenguaje semántico-puro con keywords
en español que **emite bytes precisos x86-64** sin depender de gcc/clang/LLVM.
Vive en kernel `no_std`. **Sí se puede.**

**Pipeline:**
```
fuente .bmo → lexer → [Token] → parser → Ast → sema → emit → bytes x86-64
```

**6 sub-carpetas / 15 archivos:**
- `mod.rs` + `_README.md` — versión, gramática, mapeo a BMO ABI.
- `lexer/` (2): `token.rs` con `TokenKind` (50+ variantes incluyendo todos los
  keywords solicitados); `scanner.rs` con tabla `KEYWORDS` greedy y `Scanner`
  esqueleto que ya reconoce delimitadores estructurales.
- `parser/` (2): `ast.rs` con `Stmt` (Def/Let/Retorna/Si/Mientras/Emit/RegAssign/
  Libre/Rompe/Continua/ExprStmt) + `Expr` (LitInt/LitByte/LitNulo/Ident/Bin/
  No/Reg/Aloc) + `BinOp` (9 ops) + `Type` (Byte/Num/Ptr/Arr/Ref/Void);
  `parse.rs` con `Parser` esqueleto.
- `sema/` (2): `Scope` con `ScopeEntry` (name/ty/frame_offset),
  `Sema` + `SemaError` (7 errores tipados).
- `emit/` (2): `Reg64` (16 regs x86-64) + `BMO_ARG_REGS` (7 GPR del BMO ABI:
  RDI RSI RDX R10 R8 R9 RAX_extra); `Emitter` con encoders ya operativos
  (`mov_reg_imm64` con REX.W/REX.B, `ret`, `syscall`, `nop`, `emit_raw`).
- `runtime/` (1): `aloc`/`libre` delegan a `barex::abi::memory`.

**Keywords implementados** (fase 1 base):
`def let si sino mientras retorna reg emit aloc libre byte num ptr arr ref
suma resta mult div y o no igual mayor menor rompe continua match nulo
tipo impl nuevo ventana evento dibuja`

**Reservados para fases siguientes:**
- Fase 2 OOP: `tipo`/`impl`/`nuevo` (tokens listos, AST por extender).
- Fase 3 UI: `ventana`/`evento`/`dibuja` (delegará a BareX graphics/input).

`cargo build` Finished ✅.

### Sesión 16 — BMO Simple: Lexer DFA real + 60+ keywords semánticos ⭐
Expansión masiva de BMO Simple para **diferenciarlo de ASM clásico**:
no es un mnemonic-mapper, es **semántica viviente** que emite bytes precisos.

**Cambios:**
- `lexer/token.rs` — `TokenKind` ampliado de 50 → **~95 variantes**.
  Nuevos: `OpMod/OpXor/OpShl/OpShr/OpRol/OpRor`, `KwCaso/KwDefecto/KwPara/
  KwBucle/KwDesde/KwHasta/KwPaso/KwSalto/KwEtiqueta/KwCuando/KwTabla`,
  `KwMio/KwPrest/KwMut/KwConst/KwPuro`, `KwNop/KwPausa/KwInt3/KwHlt/KwCli/
  KwSti/KwRdtsc/KwCpuid/KwLfence/KwMfence/KwSfence/KwSyscall`,
  `KwAtomico/KwVolatil/KwAcquire/KwRelease/KwRelax/KwBarr/KwCerca/KwMovnt`,
  `KwParalelo/KwSincro/KwIntrinseco`, `KwSeccion/KwAlign/KwRepetir/
  KwIncluye/KwComen/KwFin`, `FlagCf/FlagZf/FlagSf/FlagOf/FlagPf/FlagDf`,
  `LitHex/LitBin/Comment/Semicolon/Dot`.
- `lexer/scanner.rs` — **DFA real operativo** (~250 líneas): identifiers,
  decimal/hex/binary literals (con `_` separators), comments `// ... \n`,
  delimitadores estructurales, lookahead 2-char (`->`). Tabla `KEYWORDS`
  con **89 entries** ordenadas greedy.
- `builtin/` ⭐ NUEVA (3 archivos): `IntrinsicId` (12 intrínsecos
  mapeables), `bytes_for(id)` con **bytes exactos x86-64** (pausa→`F3 90`,
  cpuid→`0F A2`, mfence→`0F AE F0`, syscall→`0F 05`, etc), `emit_intrinsic()`
  operativo, `LOCK_PREFIX`/`REP_PREFIX`, `align_nops(N)` con multi-byte NOPs
  recomendados Intel (1-9 bytes optimizados), `CpuFlag` + `jcc_short_opcode()`.
- `sample/` ⭐ NUEVA: 5 programas BMO de muestra (`EXIT_ZERO`, `SPIN_LOCK`,
  `MEDIR_CICLOS`, `TABLA_SALTO`, `ALIGN_FUNCION`) que demuestran toda la
  sintaxis nueva.

**Diferencia clave vs ASM clásico:**
| ASM clásico (NASM/MASM)         | BMO Simple                          |
|---------------------------------|-------------------------------------|
| Recordar ~1500 mnemonics x86    | 95 keywords semánticos cross-CPU    |
| Cambiar de ISA = reescribir     | Misma sintaxis, distinto backend    |
| Sin tipos                       | `byte`/`num`/`ptr` + `mut`/`const`  |
| Sin ownership                   | `mio`/`prest` (move/borrow)         |
| Sin patrón match                | `match` + `caso` + `defecto`        |
| `lock` prefix manual            | `atomico { ... }`                   |
| `pause` opcode recordable       | `pausa` keyword → F3 90 garantizado |
| `align` con directiva del asm   | `align 64` portable                 |
| Inline asm en C requiere `__asm__` | `emit 0xF3 0x90` igual ergonomía |
| Sin conditional-flag exec       | `cuando cf { ... }` semántico       |
| Macros de assembler (M4)        | `repetir N { ... }` keyword         |

`cargo build` Finished ✅.

---

## 🧠 BMO ABI — referencia rápida (19 sub-carpetas en `barex/abi/`)

### Capa cimiento (sesiones 3-5)
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

### Capa genérica multi-lenguaje (sesión 7) ⭐
| Carpeta | Reemplaza |
|---|---|
| `type_system/` | RTTI C++, `Type` .NET, `Class` Java, `reflect.Type` Go, `PyTypeObject` |
| `vtable/` | vtable Itanium/MSVC, COM `IUnknown`, fat-ptr `dyn Trait`, Java iface, Go itab |
| `closure/` | (C ABI no tiene closures) — `Box<dyn FnMut>`, `std::function`, JS closures |
| `exception/` | DWARF `.eh_frame` C++, Win64 SEH, JVM/CLR exceptions, Python `raise` |
| `reflect/` | `java.lang.reflect`, `System.Reflection`, Go `reflect`, Python `inspect` |
| `lang_bridge/` | (no existía) — registro de 26 lenguajes + `LANG_FUTURE_*` |
| `marshal/` | manuales `WideCharToMultiByte`, JNI marshal, P/Invoke conversions |

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

### 20 SectionKind BEF (15 cimiento + 5 metadatos sesión 8)
Code, RoData, Data, Bss, Imports, Exports, Relocs, Symbols, Manifest,
Shaders, Resources, Tls, Unwind, Debug, Signature,
**TypeMap, VTables, LangBridge, Reflect, Closures**.

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

### Núcleo (legacy del roadmap original)
1. **Bridge BMO/GSP** — lo lleva el usuario en `drivers/gpu/fastgpu/`. NO TOCAR.
2. Conectar `barex::graphics::BxDevice::primary()` al bridge cuando esté listo.
3. Implementar `arch::x86_64::tsc::read_ns()` para que `BmoInstant::now()` cobre vida.
4. **Pipeline completo del loader BEF nativo** (relocs + imports + tls + sandbox tras `meta_sections`).
5. **Wirear thunks PE al IAT real** — escribir direcciones de `pe_thunks::THUNK_TABLE` en runtime.
6. **Wirear thunks ELF al GOT real** — usar `elf_dynamic::DynamicInfo` para mapear DT_NEEDED → resolver.
7. Localizar `IMAGE_DIRECTORY_ENTRY_IMPORT` real en PE (actualmente usa heurística).
8. `xhci::probe()` real — detectar host controller del chipset 500-series.
9. Loop de poll HID que rellene la cola de `barex::input::ring`.
10. Stream isoch OUT al headset Redragon vía `usb::audio_class::submit_pcm`.
11. Dispatcher real en `syscall::dispatch` con BMO ABI extendido.
12. Implementar `tls::setup_for_thread()` con `WRMSR IA32_FS_BASE`.
13. Parser TOML real para `bef::manifest::Manifest`.
14. Ed25519 signature verification en `bef::signing` (BLAKE3 ya hecho).
15. Test del BLAKE3 contra el vector oficial: `blake3("abc")` debe dar `6437b3ac38465133ffb63b75273a8db548c558465d79db03fd359c6cd5bd9d85`.

### BMO Simple (lenguaje propio — sesiones 17+)
16. **S17 Parser recursive-descent** operativo (~300 líneas) → parsea los 5 `sample/*` a `Ast` real.
17. **S18 Sema** completo (scopes, type-check, resolución de identificadores).
18. **S19 Codegen** AST→x86-64 (extender `Emitter` con jumps + call + ALU completas).
19. **S20 OOP** — implementar `tipo`/`impl`/`nuevo` (tokens listos, AST por extender).
20. **S21 UI** — `ventana`/`evento`/`dibuja` delegando a BareX graphics/input.
21. **S22+ Self-hosting** — traducir el lexer + emit Rust a BMO Simple y autohospedar.

### Modularizaciones restantes
22. **`barex/compat/`** — única carpeta `barex/*` aún monolítica. Modularizar como
    `detect/`, `fake_dll/`, `thunks/`, `lookup/` siguiendo el patrón fachada.

### Conexiones BMO Runtime ↔ Kernel
23. Llamar `meta_sections::build_runtime()` desde `native::load` y guardar el
    `BmoRuntime` resultante en la tabla de procesos (cuando exista).
24. Exponer `BmoRuntime::stats()` vía syscall para que el shell lo imprima.
25. Wirear `lang_bridge::LangRegistry::EMPTY` → registro real al boot con al menos
    `LANG_RUST` registrado por defecto.

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
| 7 | 26 archivos en 7 nuevas sub-carpetas BMO ABI (type_system/5, vtable/4, closure/3, exception/4, reflect/2, lang_bridge/4, marshal/4) |
| 8 | `runtime.rs` (BmoRuntime agregador) + `_README.md` abi/ + extensión SectionKind 15→20 + re-exports planos |
| 9 | `bef/loader/meta_sections.rs` (parser 5 secciones meta + builder BmoRuntime) + wiring en `native.rs` |
| 10 | `barex/net` modularizado: 9 sub-carpetas / 32 archivos (types, socket, quic, tls, http, dns, ring, driver, bypass) |
| 11 | `barex/audio` modularizado: 10 sub-carpetas / 39 archivos (format, engine, voice, mixer, codec, spatial, effects, route, backend, ring) |
| 12 | `barex/input` modularizado: 10 sub-carpetas / 39 archivos (device, keyboard, mouse, headset, gamepad, wheel, hid_raw, keymap, event, ring) |
| 13 | `barex/graphics` modularizado: 13 sub-carpetas (una por objeto núcleo) / 17 archivos — solo firmas BMO ABI (no duplica NAGA/fastgpu) |
| 14 | `barex/shader` modularizado: 9 sub-carpetas (stage, ir, sass, spirv, dxil, dxbc, loader, cache) — fachada que delega a naga + vkd3d + dxvk |
| 15 | `barex/bmoasm` ⭐ **BMO Simple** — lenguaje propio (lexer + parser + sema + emit + runtime), keywords en español, emite bytes x86-64 nativos sin LLVM |
| 16 | BMO Simple v2 — Lexer DFA real (250 líneas op) + 95 keywords semánticos + `builtin/` (12 intrínsecos con bytes exactos: pausa/cpuid/rdtsc/mfence/...) + `sample/` (5 programas .bmo) |

**Total acumulado:** ~259 archivos `.rs` nuevos + 6 `_README.md` + 1 `_WORK_LOG.md` + 11 specs en MAPA.

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

**Última actualización:** Sesión 16 (BMO Simple v2 — Lexer DFA real + 95 keywords semánticos + 12 intrínsecos bytes exactos).
**Estado del kernel:** `cargo build` Finished ✅ — fastgpu intacto.
