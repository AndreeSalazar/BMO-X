# Rutas.md — Mapa Arquitectónico de FastOS / BMO

> Documento vivo. Define la responsabilidad de cada carpeta y la dirección
> de dependencia entre módulos. Si tocas algo, actualiza este archivo.

---

## 1. Filosofía de capas

FastOS se organiza como **capas con dependencias unidireccionales**.
Una capa inferior NUNCA importa a una capa superior. Esto permite:

- Sustituir un módulo sin romper el resto
- Aislar fallos
- Razonar sobre seguridad (capas bajas = más privilegiadas)

```
                    ┌──────────────────────┐
                    │      RING 3          │  ← apps de usuario
                    │   (userland)         │
                    └──────────┬───────────┘
                               │ iretq / syscall
                    ┌──────────▼───────────┐
                    │     BMO CORE         │  ← servicios del sistema
                    │  (windowing, FS,     │
                    │   desktop, comp.)    │
                    └────┬────────────┬────┘
                         │            │
              ┌──────────▼──┐    ┌────▼──────────┐
              │  BMO ABI    │    │   BMO GPU     │  ← lenguajes
              │ (contrato   │    │  (driver GPU, │
              │  lenguajes) │    │  compositor)  │
              └──────┬──────┘    └───────┬───────┘
                     │                   │
                     └─────────┬─────────┘
                               │ syscall 0x00..0xF0
                    ┌──────────▼───────────┐
                    │      RING 0          │  ← kernel / HAL
                    │   (CPU, RAM, HW)     │
                    └──────────────────────┘
```

**Regla de oro:** una flecha solo puede ir hacia ABAJO.
RING 0 no sabe que existe BMO CORE. BMO CORE no sabe que existe RING 3.

---

## 2. RING 0 — Kernel / HAL

**Único trabajo:** administrar la **máquina física** (CPU, RAM, buses, interrupciones).
No sabe nada de ventanas, ni de juegos, ni de GPUs como concepto de alto nivel.

**Ruta:** `kernel/src/ring0/`

```
ring0/
├── mod.rs              ← entry point (_start, kernel_main_real)
├── coordinator.rs      ← orquesta las fases de boot
├── panic.rs            ← triple-fault handler visual
├── log.rs              ← logging temprano
├── result.rs           ← tipo Result del kernel
├── sync.rs             ← spinlocks, atomics
│
├── platform/           ← info de plataforma (CPUID, firmware)
│   ├── cpu.rs
│   └── mod.rs
│
├── arch/               ← específico x86-64
│   ├── gdt.rs          ← GDT + TSS
│   ├── idt.rs          ← IDT + ISR
│   ├── syscall.rs      ← dispatcher 0x00..0xF0
│   ├── apic.rs         ← Local APIC + timer
│   ├── smp.rs          ← multi-core (diferido)
│   ├── ctx.rs          ← context switch
│   └── topology.rs     ← info de cores
│
├── mem/                ← gestión de memoria física y virtual
│   ├── phys.rs         ← frame allocator (bitmap)
│   ├── virt.rs         ← page tables, VMA, demand paging
│   ├── heap.rs         ← kernel heap 16 MB
│   ├── space.rs        ← address space por proceso
│   └── mmio.rs         ← mapeo de dispositivos
│
├── dev/                ← drivers de hardware básicos
│   ├── console.rs      ← COM1 serial
│   ├── pcie.rs         ← PCI Express (ECAM + IO-port)
│   ├── framebuffer.rs  ← GOP driver
│   ├── watchdog.rs     ← watchdog (no armado todavía)
│   ├── audio.rs        ← PC speaker stub
│   └── acpi.rs         ← ACPI tables (STUB — prioridad alta)
│
├── proc/               ← procesos, threads, scheduler
│   ├── process.rs      ← Process struct
│   ├── task.rs         ← Task / Thread struct
│   ├── rt.rs           ← realtime scheduler
│   ├── mod.rs          ← scheduler RR con 5 prioridades
│   └── user_init.rs    ← salto a Ring 3 (iretq)
│
├── cpu/                ← primitivas de CPU
│   ├── features.rs     ← CPUID, XCR0
│   ├── regs.rs         ← lectura de registros
│   ├── msr.rs          ← Model-Specific Registers
│   ├── cache.rs        ← MTRR, PAT, cache control
│   ├── fpu.rs          ← FPU/SSE/AVX init + lazy save
│   ├── perf.rs         ← performance counters
│   ├── tsc.rs          ← Time Stamp Counter calibration
│   ├── delay.rs        ← busy-wait y nsleep
│   └── info.rs         ← info de CPU
│
└── boot/               ← secuencia de arranque
    ├── info.rs         ← almacena BootInfo
    ├── context.rs      ← contexto inicial
    ├── log.rs          ← log de boot
    ├── visual.rs       ← splash visual
    ├── serial.rs       ← log serial temprano
    └── phases/         ← fases 0..4
        ├── p0_arch.rs  ← GDT, IDT, FPU, CPUID
        ├── p1_mem.rs   ← frame alloc, heap, paging
        ├── p2_dev.rs   ← ACPI, PCI, devices
        ├── p3_proc.rs  ← display (GOP)
        └── p4_bmo.rs   ← scheduler, APIC timer
```

**Syscalls que expone (0x00..0xF0):**
- 0x00..0x05: procesos y threads
- 0x20..0x25: ramdisk (open/read/write/close/seek/size)
- 0x50..0x51: clock (rdtsc, nsleep busy)
- 0x60..0x65: framebuffer (info/fill/text/present/blit/frame)
- 0x70..0x71: input polling
- 0x80: beep (PC speaker)
- 0xF0: DebugPrint

**Lo que RING 0 NO debe hacer NUNCA:**
- Pintar ventanas
- Conocer el sistema de archivos
- Compilar código
- Hablar con una GPU a nivel de "draw triangle"

---

## 3. BMO CORE — Servicios del sistema

**Único trabajo:** ser la "personalidad" de FastOS. Windowing, FS, desktop, composición.
Corre **en Ring 0** pero es lógicamente una capa encima del HAL.
Pasa el control a Ring 3 cuando hay userland.

**Ruta:** `kernel/src/bmo_core/`

```
bmo_core/
├── mod.rs              ← módulo entry
├── bmo_core.rs         ← init() + enter() — punto de entrada
├── coord.rs            ← coordinación interna
│
├── bmo_api/            ← BMO API v2.0 (windowing Win32-like)
│   ├── mod.rs          ← BmoState global
│   ├── syscall.rs      ← dispatcher 0x100..0x1FF (256 syscalls)
│   ├── wm.rs           ← Window Manager (Z-order, focus, drag)
│   ├── class.rs        ← window classes + wnd_proc
│   ├── dc.rs           ← Device Context
│   ├── draw.rs         ← primitivas de dibujo (rect, text, blit)
│   ├── surface.rs      ← surfaces fuera de pantalla
│   ├── cursor.rs       ← 16 cursores built-in
│   ├── paint.rs        ← paint compositor (dirty regions)
│   ├── timer.rs        ← timer wheel global
│   ├── msg.rs          ← mensajes SPSC per-thread
│   ├── handle.rs       ← handles con generation counter
│   ├── event.rs        ← eventos (mouse, key, paint, timer)
│   ├── input.rs        ← input queue
│   ├── menu.rs         ← menús y popups
│   ├── dialog.rs       ← dialogs modales
│   ├── taskbar.rs      ← taskbar (syscalls 0x1A0..0x1A3)
│   └── err.rs          ← códigos de error
│
├── bmo_abi/            ← contratos de bajo nivel para BMO CORE
│   ├── mod.rs
│   ├── handles.rs      ← tipos de handle (HWND, HDC, etc.)
│   ├── layout.rs       ← memory layout de structs compartidos user↔kernel
│   ├── conv.rs         ← conversiones safe
│   ├── ptr.rs          ← user pointer validation
│   └── ...             ← primitivas ABI
│
├── desktop/            ← shell gráfico
│   ├── mod.rs
│   ├── welcome.rs      ← welcome screen (613 líneas, interactiva)
│   ├── commands.rs     ← Run, Hello, Reboot, Nexo
│   ├── compositor.rs   ← compone ventanas al framebuffer
│   ├── background.rs   ← fondo animado
│   ├── taskbar.rs      ← barra de tareas
│   ├── startmenu.rs    ← menú inicio
│   ├── icons.rs        ← iconos del escritorio
│   └── sound.rs        ← PC speaker beep
│
├── diag/               ← diagnóstico y telemetría
│   ├── mod.rs
│   ├── telemetry.rs    ← 30+ contadores atómicos
│   ├── overlay.rs      ← overlay de debug en pantalla
│   ├── fault.rs        ← captura de fallos
│   ├── profile.rs      ← profiling
│   ├── log_ring.rs     ← ring buffer de logs
│   └── version.rs      ← versión del sistema
│
├── gustos/             ← audio / música
│   ├── mod.rs
│   ├── synth.rs        ← FM synth
│   ├── tracks/         ← tracks procedurales
│   │   ├── logon.rs    ← windows logon chime
│   │   └── procedural.rs
│   └── mixer.rs        ← mezcla de canales
│
├── lang/               ← Compilador BMO (ver §5)
│   ├── mod.rs
│   └── bmo/            ← lenguaje BMO + toolchain
│
├── bef/                ← BEF (Binary EXchange Format)
│   ├── mod.rs
│   ├── loader/         ← loaders: BEF nativo, PE, ELF
│   ├── manifest.rs     ← manifest del binario
│   ├── reloc.rs        ← relocations
│   ├── sign.rs         ← BLAKE3 signing
│   └── tls.rs          ← Thread Local Storage
│
├── fs/                 ← sistema de archivos
│   ├── mod.rs
│   ├── manager.rs      ← VFS manager
│   ├── mount.rs        ← puntos de montaje
│   ├── inode.rs        ← inodes
│   ├── fat32.rs        ← FAT32 (stub)
│   ├── exfat.rs        ← exFAT (stub)
│   ├── ramdisk.rs      ← ramdisk funcional
│   └── ramdisk_device.rs
│
├── ui/                 ← render 2D de bajo nivel para BMO CORE
│   ├── mod.rs
│   ├── font.rs         ← bitmap font
│   ├── primitives.rs   ← líneas, rects, círculos
│   └── palette.rs      ← paleta de colores
│
└── runtime/            ← runtime de BMO (carga binarios compilados)
    ├── mod.rs
    └── loader.rs
```

**Syscalls que expone (0x100..0x1FF):** las 256 de la BMO API v2.0.

**Lo que BMO CORE NO debe hacer:**
- Tocar hardware directamente (va por RING 0)
- Implementar el driver de GPU (eso es BMO GPU)
- Compilar BMO a x86-64 directamente desde aquí (usa BMO ABI como contrato)

---

## 4. BMO GPU — Subsistema gráfico

**Único trabajo:** hablar con la **GPU física** (AMD, Intel, NVIDIA futura).
Aísla todo el conocimiento de hardware gráfico en un solo lugar.
Sirve a RING 0 (consola) Y a RING 3 (juegos) Y a BMO CORE (windowing).

**Ruta:** `kernel/src/bmo_gpu/`

```
bmo_gpu/
├── mod.rs              ← entry point + init del subsistema
│
├── driver/             ← drivers de GPU (uno por vendor)
│   ├── mod.rs
│   ├── amd/            ← AMDGPU / RDNA4 (futuro)
│   │   ├── mod.rs
│   │   ├── init.rs     ← init del hardware
│   │   ├── ring.rs     ← GFX/compute ring buffers
│   │   ├── sh.rs       ← shadow/visible MMIO
│   │   ├── power.rs    ← power management
│   │   └── interrupt.rs
│   ├── intel/          ← Intel (futuro)
│   └── generic/        ← VGA fallback (mínimo)
│
├── commands/           ← command buffers (Vulkan-like)
│   ├── mod.rs
│   ├── buffer.rs       ← command buffer + recording
│   ├── queue.rs        ← submission queues
│   ├── fence.rs        ← fences
│   └── semaphore.rs    ← semaphores
│
├── compositor/         ← composition final al framebuffer
│   ├── mod.rs
│   ├── layer.rs        ← cada ventana = una layer
│   ├── present.rs      ← presenta al scanout
│   └── vsync.rs        ← sincronización vertical
│
├── shader/             ← compilador de shaders
│   ├── mod.rs
│   ├── bsf.rs          ← BSF (BareX Shader Format) — 4 bytes magic
│   ├── frontend/       ← high-level IR
│   ├── middle/         ← optimización
│   └── backend/        ← RDNA4 ISA, future x86 fallback
│
├── memory/             ← VRAM management
│   ├── mod.rs
│   ├── heap.rs         ← VRAM allocator
│   ├── swap.rs         ← swap to RAM
│   └── residency.rs    ← paginación de recursos
│
├── pipeline/           ← graphics/compute pipelines
│   ├── mod.rs
│   ├── graphics.rs     ← render pass + subpasses
│   ├── compute.rs      ← compute shaders
│   └── rt.rs           ← ray tracing
│
├── shims/              ← adaptadores
│   ├── pe_imports/     ← imports desde PE (Windows .exe)
│   └── pe_thunks/      ← thunks para que PE llame a BMO
│
└── api/                ← API expuesta a BMO CORE y Ring 3
    ├── mod.rs
    ├── surface.rs      ← crear superficie
    ├── device.rs       ← device lógico
    ├── swapchain.rs    ← swap chain
    └── sync.rs         ← sync primitives
```

**Lo que BMO GPU NO debe hacer:**
- Pintar ventanas de alto nivel (lo hace BMO CORE sobre las APIs de BMO GPU)
- Manejar procesos (RING 0)
- Conocer el sistema de archivos (RING 0 / BMO CORE)

**Relación con BMO CORE:**
```
BMO CORE (windowing)
     │  usa
     ▼
BMO GPU API (surface, device, swapchain, draw)
     │  traduce a
     ▼
BMO GPU driver (AMD, Intel, ...)
     │  escribe en
     ▼
MMIO / BAR / ring buffer
```

---

## 5. BMO ABI — Contrato de lenguajes

**Único trabajo:** definir **cómo un lenguaje de programación habla con FastOS**.
NO contiene lógica del OS, solo **contratos** (tipos, convenciones, números de syscall,
layout de structs user↔kernel).

**Ruta:** `kernel/src/bmo_abi/`

```
bmo_abi/
├── mod.rs              ← entry: define versión del ABI
│
├── syscalls/           ← números y signaturas de syscall
│   ├── mod.rs
│   ├── ring0.rs        ← syscalls 0x00..0xF0 (expone RING 0)
│   ├── core.rs         ← syscalls 0x100..0x1FF (expone BMO CORE)
│   └── gpu.rs          ← syscalls BMO GPU (futuro rango)
│
├── calling/            ← convenciones de llamada
│   ├── mod.rs
│   ├── x86_64.rs       ← registros, stack alignment,影子空间
│   └── shadow.rs       ← shadow space para syscalls
│
├── types/              ← tipos compartidos user↔kernel
│   ├── mod.rs
│   ├── handle.rs       ← HANDLE, HWND, HDC, etc.
│   ├── result.rs       ← tipo Result estándar
│   ├── string.rs       ← strings (UTF-8, UTF-16, length-prefixed)
│   └── memory.rs       ← memory layout
│
├── memory/             ← layout de memoria user↔kernel
│   ├── mod.rs
│   ├── user_ptr.rs     ← validación de punteros user
│   ├── shared.rs       ← structs compartidos
│   └── copy.rs         ← copy in/out
│
├── proc/               ← convenciones de proceso
│   ├── mod.rs
│   ├── startup.rs      ← argumentos de inicio
│   ├── tls.rs          ← Thread Local Storage layout
│   └── stack.rs        ← stack layout
│
├── version.rs          ← versión del ABI (para compat)
└── stable.rs           ← qué partes son ABI-stable
```

**Lo que BMO ABI NO debe hacer:**
- Implementar nada (solo declarar)
- Depender de BMO CORE, BMO GPU, ni RING 0 (es independiente)
- Cambiar con cada versión (rompe todos los lenguajes)

**Relación:**
```
Compilador BMO (bmo_core/lang/bmo/)     Compilador C (futuro)
              │                                   │
              └──────────┬────────────────────────┘
                         ▼
                    BMO ABI  ← contrato estable
                         │
              ┌──────────┼──────────┐
              ▼          ▼          ▼
          RING 0     BMO CORE    BMO GPU
```

---

## 6. Lenguajes — Compilador BMO

**Único trabajo:** compilar el lenguaje BMO (y futuros C/C++/Java/Python)
a x86-64 nativo o a BEF, usando BMO ABI como contrato.

**Ruta:** `kernel/src/bmo_core/lang/bmo/`

```
lang/
├── mod.rs              ← entry del subsistema de lenguajes
│
└── bmo/                ← lenguaje BMO
    ├── mod.rs          ← entry del compilador
    │
    ├── lexer.rs        ← tokens, keywords, literales
    ├── parser/         ← AST + recursive-descent
    │   ├── mod.rs
    │   ├── ast.rs
    │   ├── expr.rs
    │   ├── stmt.rs
    │   └── decl.rs
    │
    ├── sema/           ← semantic analysis
    │   ├── mod.rs
    │   ├── types.rs    ← type system
    │   ├── scope.rs    ← scope resolution
    │   └── check.rs    ← type checking
    │
    ├── aot/            ← Ahead-Of-Time compiler
    │   ├── mod.rs
    │   ├── x86_64.rs   ← emite bytes x86-64 (604 líneas)
    │   ├── prologue.rs ← function prologue patching
    │   ├── epilogue.rs
    │   └── abi_emit.rs ← emite syscalls según BMO ABI
    │
    ├── bex/            ← BEX (BMO EXchange) intermediate
    │   ├── mod.rs
    │   ├── ir.rs
    │   └── lower.rs
    │
    ├── abi.rs          ← tabla de 50 syscalls 0x100..0x1FF
    ├── stdlib/         ← biblioteca estándar
    │   ├── mod.rs
    │   ├── sys.rs
    │   ├── io.rs
    │   ├── mem.rs
    │   ├── str.rs
    │   ├── fs.rs
    │   ├── math.rs
    │   ├── time.rs
    │   ├── gfx.rs
    │   ├── proc.rs
    │   ├── net.rs
    │   ├── path.rs
    │   ├── env.rs
    │   └── collections/
    │
    ├── pm/             ← package manager
    │   ├── mod.rs
    │   ├── manifest.rs
    │   ├── resolver.rs
    │   ├── registry.rs
    │   └── build.rs
    │
    └── plugins/        ← plugins del compilador
        ├── mod.rs
        ├── registry.rs
        ├── abi.rs
        ├── gc/         ← garbage collectors
        │   ├── mark_sweep.rs
        │   ├── copying.rs
        │   ├── generational.rs
        │   ├── refcount.rs
        │   ├── concurrent.rs
        │   └── region.rs
        ├── gil.rs
        └── languages/  ← frontends multi-lenguaje (FUTURO)
            ├── c/
            ├── cpp/
            ├── java/
            └── python/
```

**Lo que el Compilador BMO NO debe hacer:**
- Conocer detalles de drivers (usa BMO ABI)
- Depender de BMO CORE directamente (solo BMO ABI)
- Generar código que toque MMIO directamente

---

## 7. RING 3 — Userland (futuro)

**Único trabajo:** ejecutar aplicaciones de usuario.
Aislado, sin acceso directo a hardware.

**Ruta actual:** `kernel/src/ring3/` (stub, 2 archivos)

```
ring3/
├── mod.rs              ← entry stub
├── ring_3.rs           ← coord stub
│
├── loader/             ← (futuro) carga binarios userland
├── runtime/            ← (futuro) runtime de userland
├── lib/                ← (futuro) lib estándar userland
└── apps/               ← (futuro) apps de ejemplo
```

**Lo que RING 3 NO debe hacer:**
- Tocar Ring 0 (debe pedir vía syscall)
- Tocar hardware directamente
- Saltarse la validación de punteros

---

## 8. Mapa completo de carpetas (resumen)

```
FastOS/
├── bootloader/                    ← UEFI bootloader (no parte del kernel)
├── boot_protocol/                 ← struct compartido bootloader↔kernel
│
├── kernel/                        ← kernel + servicios (todo en Rust)
│   ├── Cargo.toml
│   ├── linker.ld                  ← posiciones de memoria
│   └── src/
│       ├── ring0/                 ← RING 0: kernel/HAL
│       ├── bmo_core/              ← BMO CORE: servicios
│       │   ├── bmo_api/           ←   windowing 0x100..0x1FF
│       │   ├── bmo_abi/           ←   contrato lenguajes (en bmo_core/ ???)
│       │   ├── desktop/           ←   shell
│       │   ├── diag/              ←   diagnóstico
│       │   ├── gustos/            ←   audio
│       │   ├── lang/              ←   compilador BMO
│       │   ├── bef/               ←   binary loader
│       │   ├── fs/                ←   filesystem
│       │   ├── ui/                ←   render 2D
│       │   └── runtime/           ←   runtime
│       ├── bmo_gpu/               ← BMO GPU: subsistema GPU
│       └── ring3/                 ← RING 3: userland (stub)
│
├── docs/                          ← documentación
├── gustos/                        ← catálogo audio (markdown)
├── target_build/                  ← artefactos de build
├── build_uefi.ps1                 ← script de build
├── README.md
├── WORKSPACE_OVERVIEW.md
└── Rutas.md                       ← ESTE ARCHIVO
```

---

## 9. ⚠️ Inconsistencias detectadas vs. el código actual

Este documento describe la **arquitectura ideal**. El código actual tiene
varias desviaciones que hay que corregir:

| # | Desviación | Acción |
|---|---|---|
| 1 | `bmo_abi/` está DENTRO de `bmo_core/` (debería ser capa independiente) | Mover `bmo_abi/` a `kernel/src/bmo_abi/` |
| 2 | `lang/bmo/` está DENTRO de `bmo_core/` (debería depender solo de `bmo_abi`) | Mover `lang/bmo/` a `kernel/src/lang/bmo/` y reescribir imports |
| 3 | `runtime/` no existe aún | Crear cuando se necesite cargar BEF en userland |
| 4 | `desktop/` depende de `bmo_api/` Y de `gustos/` (bien) pero también tira del `lang::bmo` directamente | Refactorizar: desktop solo debe usar `bmo_api` + servicios |
| 5 | `bmofs/`, `bmo_usb/`, `nexo_ring3/`, `nexo-sh-tool/` mencionados en `WORKSPACE_OVERVIEW.md` NO existen | Borrar menciones o crearlos |
| 6 | `bmo_gpu/shader/` solo declara BSF, no hay compilador de shaders | Crear cuando llegue driver GPU real |
| 7 | `fs/fat32.rs`, `fs/exfat.rs`, `fs/manager.rs`, `fs/mount.rs`, `fs/inode.rs` no se han leído (probable stub) | Auditar y completar cuando llegue NVMe |

---

## 10. Reglas para contribuir

Antes de añadir un archivo, pregúntate:

1. **¿Esta lógica es de HW puro?** → `ring0/`
2. **¿Esta lógica es de servicio del sistema (ventanas, FS, audio)?** → `bmo_core/`
3. **¿Esta lógica es de GPU/compositor?** → `bmo_gpu/`
4. **¿Esto es solo un contrato/definición para lenguajes?** → `bmo_abi/`
5. **¿Esto es un compilador/intérprete de lenguaje?** → `bmo_core/lang/<lenguaje>/`
6. **¿Esto es una app?** → `ring3/`

**Regla de imports:** solo se puede importar de capas iguales o inferiores.
`bmo_core` puede usar `bmo_abi` y `ring0`. `ring0` no puede usar nada de arriba.

---

## 11. Próximos pasos concretos

1. **Mover `bmo_abi/` a `kernel/src/bmo_abi/`** (1-2 horas)
2. **Auditar dependencias de `bmo_core/lang/bmo/`** y migrar imports a `bmo_abi`
3. **Borrar referencias a carpetas inexistentes** en `WORKSPACE_OVERVIEW.md`
4. **Crear `bmo_gpu/driver/amd/` skeleton** con interfaces pero sin driver
5. **Implementar `bmo_abi/syscalls/gpu.rs`** con números reservados para GPU

---

_Última actualización: ver `git log Rutas.md`_
