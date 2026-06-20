# Interrupt API (`ring0::interrupt`)

> API para registrar handlers de interrupción, configurar el
> Advanced Programmable Interrupt Controller (APIC), y gestionar
> context switches.

## Estructura

```
interrupt/
├── mod.rs      — Declara los submódulos y expone la API pública
├── gdt.rs      — Global Descriptor Table + Task State Segment
├── idt.rs      — Interrupt Descriptor Table (256 entries)
├── apic.rs     — Local + I/O APIC
├── smp.rs      — Symmetric Multi-Processing (start other cores)
├── context.rs  — Save/restore de los 15 GPRs en syscall/interrupt
└── syscall.rs  — Syscall dispatcher (legacy 0x00..0xFF)
```

## API pública

### `gdt::init()`
Inicializa la GDT con:
- GDT 0: null
- GDT 1: kernel code (ring 0, 64-bit)
- GDT 2: kernel data (ring 0)
- GDT 3: user code (ring 3, 64-bit)
- GDT 4: user data (ring 3)
- GDT 5: TSS (16 KB, con kernel stack pointer)

Carga `GDTR` con `lgdt`, luego `CS`/`SS` vía `ltr`/`retf`.

### `idt::init()` + `idt::register(vector, handler)`
Inicializa la IDT con 256 entries (0..255), con `sti; hlt;` para
los vectores 0-31 que no tienen handler explícito (deben generar
panic en el primer hit).

`register(vector: u8, handler: fn)` registra un handler para
`vector` (debe estar en 32..255). El handler debe tener la firma:

```rust
pub extern "x86-interrupt" fn handler(frame: &mut InterruptFrame) { ... }
```

### `apic::init()`
Configura:
- Local APIC: enable, map virtual a `0xFEE0_0000`, set spurious
  interrupt vector 0xFF, set LINT0 a ExtINT legacy.
- I/O APIC: lo mismo + redirige IRQ 0..15 a vectores 32..47.
- APIC timer: 1 ms tick, vector 32, periodic.

### `apic::register_irq(irq, vector)`
Redirige una IRQ legacy (0..15) a un vector (32..255) del Local APIC.

### `apic::send_ipi(target, vector)`
Envía un IPI a otro core (usado por `smp::start`).

### `smp::init()`
Para cada CPU presente en ACPI MADT, envía INIT + SIPI + SIPI
al core AP, con vector = 0x08 (entry trampoline en low memory).

### `context::save(out: &mut [u64; 15])`
Guarda los 15 GPRs (sin RSP) en el slice. Usado por syscall
para pasarlos a BMO Core.

### `context::restore(in: &[u64; 15])`
Restaura los 15 GPRs (sin RSP). Usado por `syscall_return`.

### `syscall::init()`
Configura:
- `IA32_STAR`: segmento ring 0/3.
- `IA32_LSTAR`: entry point del syscall (en `syscall_entry`).
- `IA32_FMASK`: flags a limpiar en syscall.
- `IA32_EFER` bit 0: `SCE` (syscall enable).

### `syscall::dispatch(regs: &mut SyscallRegs)`
Tabla estática 0x00..0xFF. Cada entry es un handler Rust de 6
argumentos (rdi, rsi, rdx, r10, r8, r9). Para 0x100..0x1FF, hace
`crate::bmo_api::dispatch_syscall(nr, ...)` (ver `bmo_core/_docs/`).

## Cómo añadir un handler nuevo

1. Crear handler en `interrupt/<nombre>.rs`:
   ```rust
   pub extern "x86-interrupt" fn keyboard_handler(_frame: &mut InterruptFrame) {
       // leer teclado de 0x60
   }
   ```
2. Registrar en `coordinator::init()` después de `apic::init()`:
   ```rust
   interrupt::idt::register(33, interrupt::keyboard::keyboard_handler);
   interrupt::apic::register_irq(1, 33);
   ```

## Reglas de los handlers

- **NO** hacer alocaciones (lock, mutex, alloc).
- **NO** hacer I/O bloqueante.
- **NO** dormir (`hlt`).
- **NO** llamar a funciones que esperen > 100 µs.
- Si necesitan hacer trabajo pesado, encolan un evento y el
  scheduler lo despacha después.

## Tests

Los tests de la API viven en `interrupt/tests/` (no incluidos
en v1.7.4; vendrán en v1.8.0 con QEMU).
