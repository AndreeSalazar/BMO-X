# CPU API (`ring0::cpu`)

> Primitivas de bajo nivel para el CPU: lectura/escritura de
> control registers, MSRs, MTRR, PAT, FPU, performance counters, TSC.

## Estructura

```
cpu/
├── mod.rs       — Declara los submódulos y expone la API pública
├── features.rs  — CPUID (vendor, family, model, features, TSC)
├── cr.rs        — CR0/CR2/CR3/CR4/CR8 read/write helpers
├── xcr.rs       — XCR0 (extended control register, AVX/SSE)
├── msrs.rs      — Model-specific registers genéricos
├── mtrr.rs      — Memory Type Range Registers (cache control)
├── pat.rs       — Page Attribute Table
├── fpu.rs       — Lazy FPU switching + xsave/xrestore
├── perf.rs      — Performance counters
├── tsc.rs       — Time Stamp Counter
└── info.rs      — Display de info de CPU en serial log
```

## API pública

### `features::init()`
Hace CPUID y guarda:
- Vendor string (12 bytes: "GenuineIntel", "AuthenticAMD", etc)
- Family, Model, Stepping
- Features bitmap: SSE, SSE2, SSE3, SSSE3, SSE4.1, SSE4.2, AVX,
  AVX2, AVX512F, AES-NI, RDRAND, RDSEED, etc.
- Cache sizes (L1d, L1i, L2, L3)
- TSC: rate, constant, reliable?

### `features::has(feature: Feature) -> bool`
Devuelve true si el feature está presente. Ej:
`features::has(Feature::AVX2)`.

### `cr::read(register) -> u64`
Lee CR0, CR2, CR3 o CR4. Cada uno retorna u64.

### `cr::write(register, value)`
Escribe CR0/CR3/CR4. CR2 y CR8 son read-only (sólo lectura).

### `xcr::read(index) -> u64`
Lee XCR0 (vector de features SSE/AVX/AVX-512/AMX enabled).

### `xcr::write(index, value)`
Escribe XCR0. Validar feature support antes.

### `msrs::read(msr: u32) -> u64`
Lee un MSR genérico. `rdmsr` requiere 2 instrucciones asm.

### `msrs::write(msr: u32, value: u64)`
Escribe un MSR. `wrmsr`.

### `mtrr::init()`
Programa los MTRRs con la config de cache:
- DRAM = Write-Back (default)
- MMIO = Uncacheable (UC)
- ROM = Write-Protect (WP)

### `mtrr::set_range(base: u64, size: u64, mtype: MemType)`
Programa un MTRR range register (variable MTRR).
- `base` y `size` deben estar alineados a 4 KB.
- `mtype`: `Uncacheable`, `WriteCombining`, `WriteThrough`,
  `WriteProtect`, `WriteBack`.

### `pat::init()`
Programa la PAT (Page Attribute Table) con entries:
- 0: Uncacheable
- 1: WriteCombining
- 2: Reserved
- 3: WriteThrough
- 4: WriteProtect
- 5: WriteBack
- 6: UncachedMinus
- 7: Reserved

### `fpu::init()`
Configura CR0 y CR4 para usar xsave/xrestore:
- CR0.EM = 0 (FPU presente)
- CR0.MP = 1 (monitor coprocessor)
- CR0.NE = 1 (native exceptions)
- CR4.OSFXSR = 1 (enable FXSAVE/FXRSTOR)
- CR4.OSXMMEXCPT = 1 (SSE exceptions)
- CR4.OSXSAVE = 1 (xsave enabled)
- XCR0 = 0b11 (SSE + x87)

### `fpu::save(out: &mut [u8; 4096])`
Hace `xsave` a un buffer de 4 KB.

### `fpu::restore(in: &[u8; 4096])`
Hace `xrstor`.

### `perf::init()`
Inicializa un performance counter:
- MSR 0x38D (IA32_FIXED_CTR_CTRL): programa para contar retired instructions.
- MSR 0x38F (IA32_PERF_GLOBAL_CTRL): habilita el counter.

### `perf::read_instructions() -> u64`
Lee el número de instrucciones retired.

### `tsc::init()`
Calibra el TSC:
- `tsc_rate = (T_end - T_start) / delta_time`
- `tsc_constant` = `tsc_rate * 1000` (µs)

### `tsc::now() -> u64`
Lee el TSC con `rdtsc` (no serializa).

### `tsc::ns_to_ticks(ns: u64) -> u64`
Convierte nanosegundos a ticks TSC.

### `tsc::ticks_to_ns(ticks: u64) -> u64`
Convierte ticks a nanosegundos.

### `info::print()`
Imprime toda la info de CPU por serial:
- Vendor / Family / Model
- Features bitmap (SSE, AVX, etc)
- Cache sizes
- TSC rate
- APIC ID

## Cómo añadir un nuevo MSR helper

1. Agregar la constante en `msrs.rs`:
   ```rust
   pub const IA32_TEMPERATURE_TARGET: u32 = 0x1A2;
   ```
2. Agregar la función wrapper:
   ```rust
   pub fn read_temp_target() -> u64 {
       unsafe { read(IA32_TEMPERATURE_TARGET) }
   }
   ```

## Reglas

- `unsafe` en TODO módulo que toque CRx, MSR, o `asm!`.
- **NO** deshabilitar paginación (CR0.PG = 0) en runtime.
- **NO** deshabilitar caching globalmente.
- **SÍ** validar con CPUID antes de usar AVX/AVX-512/AMX.
- **SÍ** usar `core::sync::atomic` o un spinlock simple antes
  de leer/escribir MSRs que otros cores puedan tocar.
