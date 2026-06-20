# Device API (`ring0::device`)

> API para drivers de hardware. Cada módulo aquí dentro representa
> un dispositivo: serial, PCI, framebuffer, watchdog, audio, ACPI.

## Estructura

```
device/
├── mod.rs      — Declara los drivers y expone la API pública
├── serial.rs   — COM1 115200 baud (log + early printk)
├── pci.rs      — PCI bus scan (ECAM si MCFG, sino legacy port 0xCF8)
├── gop/        — UEFI GOP framebuffer
│   ├── mod.rs
│   ├── pixel.rs
│   └── font.rs
├── watchdog.rs — Hardware watchdog (vía PCI o PNP)
├── audio/      — Audio DSP (AC97/HDA stub)
│   ├── mod.rs
│   ├── dsp.rs
│   ├── effects.rs
│   └── synth.rs
└── acpi.rs     — RSDP/MCFG parser
```

## API pública

### `serial::init()`
Configura COM1:
- Divisor = 1
- Baud = 115200
- Data = 8N1
- FIFO enabled, threshold 14 bytes
- DTR + RTS + OUT2

Tras init, `serial_write` y `serial_write_byte` están disponibles.

### `serial::serial_write(s: &str)`
Escribe un string a COM1. Útil para logs desde BMO Core (ver
`boot/serial.rs` para helpers de formato como `hex`, `u32_dec`).

### `pci::init_ecam(base: u64, end_bus: u8)`
Inicializa PCI con acceso MMIO ECAM (a partir de v1.6.6:
NO se mapea realmente porque causa #PF en Ryzen 5 5600X, pero
la función está aquí para usarla cuando arreglemos el PML4).

### `pci::scan_legacy()`
Alternativa a ECAM: scan via ports 0xCF8/0xCFC (limitado, pero
funciona en casi todo HW).

### `gop::init_gop(fb: *mut u32, w: u32, h: u32, pitch: u32)`
Inicializa el framebuffer con los parámetros entregados por UEFI
GOP. Tras init, `gop::put_pixel(x, y, color)` y
`gop::blit(&[u32])` están disponibles.

### `gop::get_mode() -> (w, h, pitch)`
Devuelve las dimensiones del framebuffer actual.

### `watchdog::init()`
Configura el watchdog hardware (si hay). Por defecto, 30s timeout.
En v1.7.4, detecta tipo via PCI (no usa ICH LPC PNP, sólo PCI).

### `watchdog::kick()`
Patea el watchdog. Llamar desde `coordinator::main` cada 10s.

### `audio::init()`
Stub. En v1.7.4 no hay audio HW. La estructura del módulo está
lista para que en v1.8.0 (con HDA driver real) sólo se llene
`audio::dsp`.

### `acpi::init()`
Encuentra el RSDP en las UEFI config tables, valida checksum,
parsea RSDP→XSDT→MCFG y guarda la dirección base ECAM.

### `acpi::mcfg_snapshot() -> Option<McfgHeader>`
Devuelve un snapshot del MCFG encontrado (o None si no hay MCFG).

## Cómo añadir un driver nuevo

1. Crear `device/<nombre>.rs`:
   ```rust
   pub fn init() {
       // inicialización
   }
   pub fn read() -> Result<[u8; 4], DriverError> {
       unsafe { ... }
   }
   pub fn write(data: &[u8; 4]) -> Result<(), DriverError> {
       unsafe { ... }
   }
   pub fn probe() -> bool {
       // true si el driver detecta su HW
   }
   ```
2. Agregar `pub mod <nombre>;` en `device/mod.rs`.
3. Agregar la llamada a `<nombre>::init()` en `coordinator::init()`,
   después de `device::pci::init()` (porque muchos drivers usan PCI).
4. Si expone syscall, agregar entry en `interrupt/syscall.rs`.

## Reglas de drivers

- **NO** hacer alocaciones en `read`/`write` (rutas de I/O rápidas).
- **NO** bloquear > 1 ms. Si necesitas esperar, usa `cpu::busy_wait_ms`
  o agenda un poll en `sched`.
- **NO** asumir que el hardware responde a la primera. Reintentar
  hasta 3 veces antes de devolver error.
- **SÍ** loguear a serial cada `init()` exitoso con su nombre.

## Tests

En v1.7.4 no hay tests (no tenemos QEMU configurado en CI).
En v1.8.0: tests de driver en `device/tests/` con mocks.
