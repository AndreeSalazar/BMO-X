# Análisis: BMO CORE como Kernel de Ring 3

> Análisis exhaustivo de `bmo_abi/` y `lang/` para determinar qué falta
> para que BMO CORE pueda actuar como kernel de Ring 3 (entregando
> control a userland apps).
>
> **v1.8.8** — generado tras la reorganización Opus que separó estas
> capas como independientes.

---

## Resumen ejecutivo

FastOS v1.8.8 está **arquitectónicamente bien diseñado** y
**sustancialmente implementado**:

- **108 archivos** en `bmo_abi/` y `lang/` (contratos + compiladores)
- **76 archivos** en `bmo_core/` (windowing, FS, desktop, BEF loader)
- **~30 syscalls** implementadas en `bmo_api`
- **AOT x86-64** de 604 líneas funcional
- **BEF loader** de 124 KB que come BEF + PE + ELF
- **`ring0::proc::user_init`** tiene `allocate_user_process` y
  `jump_to_ring3` operativos

**Lo que falta para handoff Ring 3 es muy poco:**

1. Un módulo `ring3::init()` real (~100 líneas)
2. Un linker BMO AST → BEF bytes (~300 líneas)
3. Resolver 1 conflicto de syscall numbers
4. Completar 6-8 syscalls faltantes en `bmo_api` (~200 líneas)
5. `bmo_core::coord::enter` → `ring3::init` en vez de `welcome::run`
6. argv/envp/auxv setup (~50 líneas)

**Total: ~600 líneas de Rust** para tener un userland funcional
ejecutando un BMO nativo compilado. Eso es **menos del 1%** del código
existente.

---

## 1. Estructura de `bmo_abi/` (42 archivos .rs)

Capa independiente de **contratos para lenguajes**. No depende de
nada en el proyecto. Provee:

### Tipos primitivos
- `bx_u8..u128`, `bx_i8..i128`, `bx_f32/f64/f16`, `bx_bool`
- `BmoStatus` (16B), `BmoError`, `BmoResult<T>`, `BmoOption<T>`
- 18 `ErrorCode` (OK, OUT_OF_MEMORY, INVALID_ARGUMENT, ..., ADDR_IN_USE)

### Handles y recursos
- `BmoHandle` (64-bit): tag 1b + kind 7b + generation 16b + index 40b
- 33 `HandleKind` (Device, Queue, CmdList, Pso, Buffer, Texture, Fence, Swapchain, Process, Thread, Mutex, Futex, Port, ...)
- `BmoFileHandle`, `BmoPipe`, `BmoSeekMode`
- `BmoSocketAddr` (IPv4/IPv6)
- `BmoSpinLock` (TTAS)

### Strings, time, memory
- `BmoStr`/`BmoString` UTF-8 (ptr+len, sin nul-terminator)
- `BmoInstant` (TSC-backed, Q32.32), `BmoDuration`
- `BmoSlice<T>`, `BmoRange`, `BmoAligned<T,N>`, `BmoPageAligned<T>`
- `BmoFormatter` (stack-allocated 1KB, sin heap)

### Atomic y sync
- `BmoAtomicU32/U64/Bool` + `MemOrder`
- `BmoSpinLock` (TTAS; **no hay futex/mutex bloqueante**)

### Runtime
- `TypeRegistry` (256 slots de TypeDescriptor)
- `VTableStore` (64 slots × 16 métodos)
- `LangBridge` (8 languages metadata)

### Lo que falta
- ❌ Contracts para copy_in/copy_out user↔kernel
- ❌ `BmoFutex` (hay tipo en HandleKind pero no API)
- ❌ `BmoMutex` bloqueante (no solo spinlock)
- ❌ `BmoIpcPort` (declarado en HandleKind, sin API)
- ❌ `BmoProcess`/`BmoThread` (tipos FFI)

---

## 2. Estructura de `lang/` (66 archivos .rs)

Capa independiente de **compiladores**. Solo depende de `bmo_abi/`.

### Compilador BMO (nativo)

Pipeline completo:
- **Lexer** (424 líneas) — keywords en español (fn/let/mut/si/sino/...)
- **Parser** (688 líneas) → AST con Stmt/Expr/BinOp/TypeAnnotation
- **Semantic** (455 líneas) → VarInfo, FnInfo, StructInfo, Scope, ImportEntry
- **AOT x86-64** (604 líneas) — emite bytes máquina reales
  - `mov rax, <syscall_nr>; syscall` para llamadas BMO ABI
  - prologue/epilogue con stack frame patching
- **Runtime** (7 archivos): proc, io, fs, mem, time + 13 módulos de stdlib

### Frontends multi-lenguaje

| Lenguaje | Estado |
|----------|--------|
| **C** | Funcional: lexer + parser + ast + translator (382 ln) → AST BMO → AOT |
| **C++** | Stub v0.1.0: solo classes → struct |
| **Java** | Stub v0.1.0: solo class lowering |
| **Python** | Stub v0.1.0: `translate()` retorna `Ast::empty` |
| **Rust/Go** | **No existen** (solo en docs) |

### Stdlib BMO (13 módulos)

| Módulo | Estado |
|--------|--------|
| io, mem, str, math, fs, proc, time, gfx, path, collections, sys | ✅ Funcionales |
| **net** | ❌ Stubs puros (tcp_connect retorna None) |
| **env** | ❌ Stubs puros (args() retorna &[]) |

### Package manager (4 archivos)
- `manifest.rs`: parser TOML-subset
- `registry.rs`: registry local con 9 paquetes builtin
- `resolver.rs`: cycle detection (DFS), topological sort
- `build.rs`: ⚠️ solo cuenta archivos, no invoca el AOT

### Plugins (22 archivos)
- 4 traits base (LanguageAdapter, MemoryModel, GcStrategy, Language enum)
- Bridge ABI/Ffi
- ✅ BmoAdapter funcional
- ⚠️ CppAdapter, JavaAdapter, PythonAdapter parciales

### Lo que falta

- ❌ **GC** (mark_sweep, copying, generational, refcount, concurrent, region)
- ❌ **Linker** (AOT emite solo bytes de función, sin header BEF)
- ❌ **Runtime BMO bytecode** (la arquitectura v2.0 dice "No VM. No bytecode.")
- ❌ **Debugger/inspector**
- ❌ **Profiler** (solo `telemetry.rs` a nivel kernel)
- ❌ **Cap system en runtime BMO** (Process::caps existe en ring0 pero no se valida en bmo_api::dispatch_syscall)

---

## 3. Estructura de `bmo_core/` (76 archivos)

Es el kernel de Ring 3. Tiene:

### Windowing API (`bmo_api/`, 16 módulos)
- **70+ syscalls** implementadas en `syscall.rs` (883 líneas)
- Window manager, Z-order, focus, parent/child, drag/resize
- DC (Device Context) con clip
- Surfaces offscreen, paint compositor con dirty regions
- 36 `BmoMsgKind`, SPSC queue 64 msgs/thread
- Timers con wheel 1ms
- 16 cursores built-in
- Clipboard (stubs que retornan OK)

### Filesystem (`fs/`, 6 archivos)
- `ramdisk.rs` (168 líneas) — **FUNCIONAL** con 16 FDs estáticos
- `fat32.rs` (769 líneas, 27 KB) — driver completo, **no conectado**
- `exfat.rs` (880 líneas, 29 KB) — driver completo, **no conectado**
- `manager.rs`, `mount.rs`, `inode.rs` — VFS infrastructure

### BEF Loader (`bef/`, 19 archivos, 124 KB)
- Header 48B con `BEF_MAGIC = "BEF1"`
- 16 `SectionKind` (Code, RoData, Data, Imports, Exports, etc.)
- BLAKE3 hash, Ed25519 firma
- ASLR + capabilities + provenance
- **Devoradores PE/ELF** que traducen a BEF interno
- `loader/native.rs` (330 líneas) — carga BEF, mapea, aplica relocations, resuelve imports, setup TLS

### Audio (`gustos/`)
- FM synth, PCM, tracks procedurales, logon chime de Windows
- Syscalls 0x170..0x173 declaradas pero **no implementadas** en dispatcher

### Diag, UI, Desktop
- Diag con 30+ contadores atómicos
- Framebuffer + font 8x16
- Welcome screen 22 KB con splash, comandos (Run/Hello/Reboot)

---

## 4. Estructura de `ring3/` (2 archivos + 4 docs)

Stub. Lo que hay:
- `mod.rs` (27 líneas) — re-export
- `ring_3.rs` (40 líneas):
  - `init()` — **vacía**. Comentario: "Los procesos se crean bajo demanda via allocate_user_process(). No hay loader dinámico todavía — las apps son 64 bytes de x86-64 hardcoded en user_init.rs."
  - `enter_wnd_proc(hwnd, msg, wparam, lparam) → Option<u64>` — retorna None siempre
  - `is_ring3_wnd_proc(wnd_proc: u64) → bool` — chequea wnd_proc != 0

Las docs describen un sistema completo de loader, pero **ningún código está implementado**.

---

## 5. Gaps críticos para handoff Ring 3 funcional

| # | Item | Severidad | Esfuerzo | Bloquea userland? |
|---|------|-----------|----------|-------------------|
| 1 | Handoff `bmo_core::coord::enter` → `proc::user_init::allocate_user_process` + `jump_to_ring3` | **Alta** | 1-2 días | **SÍ** |
| 2 | Linker BMO AST → bytes BEF (header 48B + section table) | **Alta** | 1 semana | **SÍ** |
| 3 | Resolver conflicto `0x180` (`PROC_SPAWN` vs `MAP_SURFACE`) | **Alta** | 1 día | **SÍ** |
| 4 | Implementar dispatcher para PROC_SPAWN, PROC_EXIT, PROC_GET_PID, PROC_YIELD, THREAD_*, MEM_ALLOC, MEM_FREE, MEM_MAP, MEM_UNMAP en `bmo_api` | **Alta** | 3-5 días | **SÍ** |
| 5 | `ring3::init()` real: cargar BEF desde ramdisk + alloc_process + jump_to_ring3 | **Alta** | 2-3 días | **SÍ** |
| 6 | argv/envp/auxv setup en `user_init::allocate_user_process` | Alta | 1 día | Sí (funcional) |
| 7 | Implementar dispatcher FS BMO ABI (0x140..0x149) en `bmo_api` (hoy solo legacy 0x20..0x25) | Alta | 2-3 días | Sí |
| 8 | Validación robusta de user pointers (límite `< 0x0000_8000_0000_0000`, copy_in/out) | Media | 2-3 días | Sí (seguridad) |
| 9 | TLS setup para procesos userland (llamar `bef::tls::setup_for_thread`) | Media | 1-2 días | Sí (C/C++/Rust) |
| 10 | Capabilities enforcement en `bmo_api::dispatch_syscall` | Media | 2-3 días | No (init caps=ALL) |
| 11 | Dispatcher IPC (0x1A0..0x1A3) — ports o pipes | Media | 3-5 días | No (single-process) |
| 12 | Dispatcher de audio (0x170..0x173) | Baja | 2-3 días | No |
| 13 | GC mark_sweep/copying/generational (para Java/Python) | Baja | 2-3 semanas | No (BMO nativo) |
| 14 | Compilador C++ completo (classes, vtables, templates) | Media | 2-3 semanas | No (BMO nativo basta) |
| 15 | Compilador Java completo (exceptions, generics) | Media | 2-3 semanas | No |
| 16 | Compilador Python (AST → BMO AST) | Media | 1-2 semanas | No |
| 17 | Compilador Rust (borrow checker + AOT) | Baja | 4-6 semanas | No |
| 18 | Compilador Go | Baja | 4-6 semanas | No |
| 19 | Signals (kill, signal, sigaction) | Baja | 1 semana | No |
| 20 | Debugger/inspector userland | Baja | 1-2 semanas | No |
| 21 | Profiler userland | Baja | 1 semana | No |
| 22 | Net stack (TCP/IP, sockets) | Baja | 2-3 semanas | No |
| 23 | Disk driver (NVMe/AHCI) para que FAT32/exFAT sean usables | Alta (post-demo) | 2-4 semanas | No (ramdisk basta) |
| 24 | Driver RDNA4 GPU | Alta (post-Opus 3) | 2-3 meses | No (CPU drawing) |
| 25 | BSF shader compiler | Media | 1 mes | No |
| 26 | Hacer que `std::net` y `std::env` de BMO stdlib funcionen (ahora stubs) | Baja | 1 semana | No |
| 27 | Terminar `bmo_core::coord::enter` para que llame a `ring3::init` (en vez de `desktop::welcome::run` que es Ring 0) | **Alta** | 0.5 días | **SÍ** |
| 28 | Implementar 6-10 syscalls en `lang/bmo/abi.rs` que están en la tabla pero `bmo_api` cae a `err::INVALID` (audio, signals, etc.) | Media | 1-2 días | Parcial |
| 29 | FS `Manager::open` conectar a ramdisk path lookup (hoy solo ruta literal `bmo:readme`) | Baja | 1 día | No (legacy funciona) |
| 30 | Test: compilar un BMO programa, linkearlo a BEF, cargarlo, ejecutarlo en Ring 3, hacer una syscall a `win_create` | **Alta** | 2-3 días (post items 1-5) | **SÍ** (acceptance test) |

---

## 6. Roadmap sugerido

### Fase 0 (días 1-2): Decisiones arquitectónicas
1. Resolver conflicto 0x180 (mover MAP_SURFACE a 0x1C0..0x1CF).
2. Decidir primer proceso: hardcoded o cargado de BEF.
3. Fijar rango de direcciones user (actual: code 0x40_0000, stack 0x80_0000 — bien).

### Fase 1 (días 3-7): Mínimo userland ejecutable
1. **Linker BMO** (`lang/bmo/linker.rs`): toma bytes del AOT + metadata y emite BEF.
2. **Conectar `bmo_core::coord::enter` con `ring3::init`**.
3. **Implementar syscalls faltantes** en `bmo_api::dispatch_syscall`.
4. **argv/envp/auxv setup** en `user_init::allocate_user_process`.
5. **Test:** compilar `fn main() { win_create("Test", 0, 0, 100, 100); loop {} }`, linkear a BEF, boot, ver ventana.

### Fase 2 (semana 2): Seguridad y robustez
1. `validate_user_ptr` robusto (límite sup + copy_in/out).
2. TLS setup en `user_init`.
3. Capabilities enforcement.
4. Errores claros en lugar de `err::OK` para syscalls no implementadas.

### Fase 3 (semana 3-4): Userland apps reales
1. Compilador C++ completo.
2. Compilador Java (try/catch, interfaces).
3. Compilador Python.
4. Net stack mínimo.
5. GC mark-sweep.

### Fase 4 (semana 5+): Pulido
1. Signals.
2. Debugger GDB-stub.
3. Profiler.
4. Disk driver (NVMe/AHCI).
5. BMO GPU (BSF shader, RDNA4).

---

## 7. Conclusión

**Lo que falta para handoff Ring 3 es muy poco comparado con lo que
ya existe.** El proyecto tiene:

- **~30 syscalls** de windowing implementadas
- **AOT x86-64** de 604 líneas funcional
- **BEF loader** de 124 KB que come BEF+PE+ELF
- **`proc::user_init`** con `allocate_user_process` y `jump_to_ring3`
- **ABI** con 33 HandleKind, TypeRegistry, VTableStore
- **Stdlib BMO** con 11 módulos funcionales

**Para que un BMO programa corra en Ring 3 solo faltan ~600 líneas
de Rust:**

1. `ring3::init()` real (~100 líneas)
2. Linker BMO AST → BEF (~300 líneas)
3. Resolver 0x180 + 6-8 syscalls (~200 líneas)
4. Conectar `coord::enter` con `ring3::init` (~10 líneas)

Eso es **menos del 1%** del código existente. La arquitectura Opus está
**lista para soportar un userland funcional**; solo falta el último
paso de integración.

---

_Generado como parte del análisis Opus Phase 3 (v1.8.8)._
