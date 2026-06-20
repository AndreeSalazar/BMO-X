# BMO Core — Windowing + Language + FS

> BMO Core es el **núcleo lógico** de FastOS: windowing system, sistema
> de archivos (BFS), intérprete BMO, BMOASM JIT, scheduler, y la
> API de windowing 0x100..0x1FF que se expone a Ring 3.

## Estructura

```
bmo_core/
├── mod.rs                  — Init de BMO Core (lanzado desde coordinator)
├── _docs/                  — Esta documentación
│   ├── README.md           — Éste archivo (overview)
│   ├── ARCHITECTURE.md     — Capas, BFS layout, BMOASM model
│   ├── window.md           — Windowing API
│   ├── bmoasm.md           — BMOASM spec
│   ├── bfs.md              — BFS filesystem layout
│   ├── nexolang.md         — Nexolang spec
│   └── schedule.md         — Scheduler
│
├── api/                    — API expuesta a Ring 3 (256 syscalls)
├── window/                 — Windowing system
├── fs/                     — BFS filesystem
├── bmo/                    — BMO interpreter
├── bmoasm/                 — BMOASM emitter + JIT
├── nexolang/               — Nexolang compiler
├── schedule/               — Scheduler
├── diag/                   — Diag / debug HUD
├── task/                   — Tasks/threads
├── event/                  — Event loop
├── ipc/                    — IPC (message passing)
├── graph/                  — 2D graphics primitives
├── text/                   — Text rendering
├── audio/                  — Audio mixer
├── input/                  — Input devices
├── time/                   — Time API
├── heap/                   — User heap
├── stack/                  — User stack
└── stdio/                  — STDIN/STDOUT/STDERR
```

## Concepto

BMO Core es el **único lugar** donde vive la lógica de FastOS. Ring 0
sólo expone las 4 APIs HAL. Toda decisión de "qué hacer" se hace en
BMO Core.

Por ejemplo: cuando un usuario presiona una tecla:

```
hardware (ring 0)
  ↓
interrupt::keyboard_handler (ring 0)
  ↓
sched::enqueue_event (ring 0)
  ↓
[scheduler switch]
  ↓
bmo_core::event::process_event (BMO Core)
  ↓
bmo_core::window::dispatch_key (BMO Core)
  ↓
bmo_core::api::keyboard_press (BMO Core)
  ↓
[syscall return]
  ↓
app Ring 3
```

## Capas dentro de BMO Core

```
         ┌─────────────────────────────┐
         │    api/ (256 syscalls)      │  ← User-facing
         ├─────────────────────────────┤
         │  window/ bmo/ fs/ bmoasm/   │  ← Lógica
         ├─────────────────────────────┤
         │  schedule/ event/ task/     │  ← Concurrencia
         ├─────────────────────────────┤
         │  graph/ text/ audio/        │  ← Rendering
         ├─────────────────────────────┤
         │  input/ time/ heap/         │  ← Primitivas
         └─────────────────────────────┘
```

Las capas inferiores pueden llamarse entre sí; las superiores no
pueden saltarse capas.

## API expuesta a Ring 3

256 syscalls en 0x100..0x1FF (ver `window.md`):

- 0x100..0x11F: Window create/destroy/show/hide/move/resize
- 0x120..0x13F: Draw pixel/rect/line/text/blit
- 0x140..0x15F: Input poll (keyboard/mouse)
- 0x160..0x17F: File operations (open/read/write/close/seek)
- 0x180..0x19F: Process (spawn/exit/yield/kill)
- 0x1A0..0x1BF: Memory (mmap/munmap/brk)
- 0x1C0..0x1DF: IPC (send/recv)
- 0x1E0..0x1FF: Time, audio, misc

## Cómo añadir una syscall nueva

1. Agregar la entrada en `api/<categoría>.rs`:
   ```rust
   pub fn my_new_syscall(arg0: u64, arg1: u64) -> u64 {
       // implementación
   }
   ```
2. Registrar en `api/mod.rs`:
   ```rust
   pub fn dispatch(nr: u32, args: &[u64; 6]) -> u64 {
       match nr {
           0x100 => { /* window_create */ }
           0x1F0 => window::my_new_syscall(args[0], args[1]),
           _ => 0xFFFF_FFFF,
       }
   }
   ```
3. Documentar en `window.md` (o el .md de la categoría).

## Convenciones

- Toda función de BMO Core es **safe** (no `unsafe` público).
- Si necesita `unsafe`, encapsular en un módulo y exponer API safe.
- **NO** tocar hardware directamente; usar las APIs de Ring 0.
- **NO** hacer I/O bloqueante > 1 ms en event loop.
- **SÍ** loguear errores con `diag::log`.

## Compilación

- BMO Core es una carpeta `#[path]`-included desde `ring0/mod.rs`.
- NO compila independiente: es parte del binario `fastos-kernel`.
- Compila cuando el kernel compila (`cargo build --release
  --target x86_64-unknown-none`).
