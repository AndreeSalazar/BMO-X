# Ring 0 — Architecture & Contracts

> Detalle de las capas de Ring 0, dependencias entre módulos, y el orden
> de inicialización que sigue `coordinator::init()`.

## Mapa de capas

```
                    ┌──────────────────────────────┐
                    │       coordinator.rs         │
                    │   init() + main() + loops    │
                    └──────────────┬───────────────┘
                                   │
        ┌──────────────┬───────────┴──────────┬──────────────┐
        ▼              ▼                      ▼              ▼
   ┌────────┐    ┌──────────┐          ┌──────────┐   ┌──────────┐
   │ cpu::* │    │interrupts│          │ memory   │   │  device  │
   │  CPU   │    │  API     │          │   API    │   │   API    │
   │feats.  │    │          │          │          │   │          │
   └────────┘    └──────────┘          └──────────┘   └──────────┘
                       │                     │               │
                       └──────────┬──────────┘               │
                                  ▼                          ▼
                            ┌──────────┐              ┌──────────┐
                            │ sched::* │              │  panic   │
                            │scheduler │              │ handler  │
                            └──────────┘              └──────────┘
```

## Reglas de dependencia

Las dependencias siguen un grafo **estrictamente acíclico**:

- `cpu::*` no depende de nadie (leaf).
- `memory::*` depende de `cpu::cr` (para configurar CR3/CR4).
- `interrupt::*` depende de `cpu::*` (cli/sti, rdtsc) y de `memory::stack_alloc` (kernel stack para CPUs).
- `device::*` depende de `memory::*` (MMIO), `cpu::*` (rdtsc) y `interrupt::apic` (MSI).
- `sched::*` depende de `interrupt::context` (para context switch).
- `coordinator.rs` orquesta todo: es el único módulo que conoce
  simultáneamente las 4 APIs.

## Orden de inicialización (coordinator::init)

```
1. cpu::features::init()         — CPUID, microarchitecture, vendor string
2. cpu::cr::init()                — CR0/CR4 sane bits (cache enable, etc)
3. cpu::fpu::init()               — xsave/xrestore setup
4. memory::init()                 — heap + page allocator (antes de allocar nada)
5. interrupt::gdt::init()         — GDT + TSS (necesario para ring transitions)
6. interrupt::idt::init()         — IDT con 256 entries (handlers 0-31 y 32-255)
7. interrupt::apic::init()        — Local APIC + I/O APIC + timer tick
8. interrupt::smp::init()         — INIT-SIPI-SIPI a otros cores
9. interrupt::syscall::init()     — STAR/LSTAR MSR + EFER.SCE
10. device::serial::init()        — COM1 a 115200 baud (log)
11. device::acpi::init()          — RSDP/MCFG discovery
12. device::pci::init()           — ECAM si MCFG presente, sino legacy scan
13. device::gop::init_gop()       — UEFI framebuffer (si estaba mapeado)
14. device::watchdog::init()      — Hardware watchdog 30s timeout
15. device::audio::init()         — AC97/HDA stub (no real HW en v1.7)
16. sched::init()                 — Scheduler + 1 idle thread
17. syscall::init()               — Tabla 0x00..0xFF legacy
18. bmo_core::init()              — BMO Core carga sus módulos
```

## Contratos de inicialización

- Cada `init()` debe ser **idempotente**: llamarlo 2 veces no rompe el
  sistema.
- Si un `init()` falla, debe **panic inmediatamente** con un mensaje
  claro. No retornar `Result` para errores fatales.
- Los `init()` no pueden llamarse entre sí (excepto panic si fallan).
- Cualquier alocación debe esperar a `memory::init()` (paso 4).

## Memory layout esperado

```
0x0000_0000 ──────────────  (legacy low memory)
0x0010_0000 ──┐
              │  kernel .text
0x0020_0000 ──┘
...
0x0100_0000 ──────────────  (BASE: por encima de esto = usable)
...
0x1000_0000 ──────────────  (16 MB) heap crece desde aquí
0x1000_0000 + heap_size ──
0x2000_0000 ──────────────  (32 MB) page allocator frames
0x2000_0000 + frame_size ─
0xFFFF_8000_0000_0000 ────  (kernel space) PML4
```

## Interfaz con el bootloader

El bootloader UEFI entrega:

- BootInfo con magic `0xF455F455` (ver `boot_info.rs`).
- Puntero a GOP framebuffer (si lo configuró).
- ACPI RSDP pointer (vía UEFI config table).
- Mapeo de UEFI memory map (pre-staged para que el kernel no haga
  page faults durante `init()`).

## Hooks opcionales

- `#[no_mangle] pub extern "C" fn _start()` en `mod.rs` — entry point
  dado por el bootloader.
- `#[panic_handler] fn panic(info: &PanicInfo)` en `panic.rs`.
- `#[lang = "eh_personality"]` (vacío, sin unwinding).

Ver `interrupt.md`, `device.md`, `memory.md`, `cpu.md` para detalles de cada API.
