# Ring 0 — Hardware Abstraction Layer

> **v1.7.4** — El kernel privileged de FastOS corre aquí. Cualquier código
> que toque hardware directamente (GDT, IDT, drivers, memoria) vive en
> este anillo. El resto del kernel nunca toca hardware: pasa por las APIs
> que aquí se exponen.

## Estructura

```
ring0/
├── mod.rs              — Entry point del binario (declara los 4 APIs + entry point)
├── _docs/              — Esta documentación
│   ├── README.md       — Éste archivo (overview)
│   ├── ARCHITECTURE.md — Contratos, capas, dependencias
│   ├── interrupt.md    — API de interrupciones (GDT, IDT, APIC, SMP, syscall)
│   ├── device.md       — API de drivers (GOP, serial, PCI, watchdog, audio, ACPI)
│   ├── memory.md       — API de memoria (heap, page_alloc, paging, VMM)
│   └── cpu.md          — Primitivas CPU (CR, MSRs, MTRR, FPU, features)
│
├── interrupt/          ← Interrupt API (Ring 0 HAL)
├── device/             ← Device API (Ring 0 HAL)
├── memory/             ← Memory API (Ring 0 HAL)
├── cpu/                ← CPU primitives
│
├── boot/               — Fases 0-5 del boot sequence
├── sched/              — Scheduler, process, thread
├── syscall/            — Driver API: tabla de syscalls 0x00..0xFF (legacy)
│
├── boot_info.rs        — BootInfo global (del bootloader)
├── coordinator.rs      — init() + main(): orquesta todo Ring 0
├── panic.rs            — panic_handler
```

## Concepto: las 4 APIs

Ring 0 expone **cuatro APIs claras** que cualquier otro módulo del kernel
puede consumir:

| API | Módulo | Para qué |
|---|---|---|
| **Interrupt** | `ring0::interrupt::*` | Registrar handlers, enviar IPIs, configurar IDT/GDT/APIC |
| **Device** | `ring0::device::*` | Inicializar y usar drivers (GOP, serial, PCI, watchdog, audio, ACPI) |
| **Memory** | `ring0::memory::*` | Asignar páginas, mapear memoria virtual, crear procesos |
| **CPU** | `ring0::cpu::*` | rdtsc, busy_wait_ms, leer MSRs, info de CPU |

Las 4 APIs son **independientes entre sí** (no se llaman entre ellas en
loop). El `coordinator::init()` las inicializa en orden seguro (ver
`ARCHITECTURE.md`).

## Cómo extender

### Añadir un nuevo driver de hardware

1. Crear `device/<nombre>.rs` con la API:
   ```rust
   pub fn init() { /* hardware init */ }
   pub fn read() -> Result<Data, Error> { ... }
   pub fn write(data: &Data) -> Result<(), Error> { ... }
   ```
2. Agregar `pub mod <nombre>;` en `device/mod.rs`.
3. Si expone syscall nuevo, agregar el case en `interrupt/syscall.rs`.
4. Si necesita init en una fase específica, agregar en `coordinator.rs::init()`.

Ver `device.md` para más detalles.

### Añadir un nuevo handler de interrupción

1. Definir el handler en `interrupt/<nombre>.rs`:
   ```rust
   pub extern "x86-interrupt" fn handler(frame: &mut InterruptFrame) { ... }
   ```
2. Registrar en la IDT con `interrupt::idt::register(vector, handler)`.
3. Si el handler es per-IRQ, registrar también en el IO-APIC con
   `interrupt::apic::register_irq(irq, vector)`.

Ver `interrupt.md` para más detalles.

## Convenciones del código

- **No `unsafe` global** — usar `unsafe { ... }` localizado en funciones
  que exponen API safe.
- **No alocar en handlers de interrupción** — usar tablas estáticas.
- **No bloquear con `hlt`/`sti` salvo en initialization** — los
  handlers deben retornar rápido.
- **Naming**:
  - `init()` — función pública de inicialización (llamada desde
    `coordinator::init()`).
  - `poll()` — para drivers que se consultan periódicamente.
  - `read()` / `write()` — para drivers bloqueantes.
  - `register_*()` — para APIs que registran algo (handlers, IRQs, etc).

## Contratos con BMO Core (ver `bmo_core/_docs/`)

- Ring 0 expone syscalls a BMO Core (0x00..0xFF, en `interrupt/syscall.rs`).
- BMO Core expone la windowing API 0x100..0x1FF a Ring 3.
- Ring 0 **nunca** debe ser alcanzado por código de Ring 3. La única
  vía es vía syscall, que valida origen y destino antes de ejecutar.
- Ring 0 → BMO Core: helpers como `rdtsc`, `busy_wait_ms` y los
  drivers de `device/` están disponibles para BMO Core vía paths normales.

## Compilación y debug

- `cargo build --release --target x86_64-unknown-none` desde `kernel/`.
- Logs serial: `crate::device::serial::serial_write(...)`.
- Debug HUD: `Ctrl+Alt` activa el overlay de `bmo_core::diag`.

## Próximos pasos (v1.7.x → v2.0)

- **v1.7.5**: Conectar `cpu::ring3_test` real (trampoline + per-thread kernel stack).
- **v1.8.0**: Re-introducir USB/ATA/AHCI cuando se reescriban los drivers con device tree.
- **v2.0.0**: Spec del BMO API v2.1 con driver dinámico de apps Ring 3.

Ver `docs/BMO_API_V2_SPEC.md` para la spec completa.
