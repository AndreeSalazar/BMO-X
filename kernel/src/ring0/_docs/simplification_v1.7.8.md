# Simplificación del CPU (v1.7.8)

> El kernel es **específico** del Ryzen 5 5600X. No exponemos un
> bitmap de 83 features. Exponemos **constantes booleanas** que son
> siempre `true` en el 5600X.

## Antes (v1.7.7): FeatureBitmap de 83 campos

```rust
pub struct FeatureBitmap {
    pub fpu: bool, pub vme: bool, pub de: bool, pub pse: bool,
    pub sse3: bool, pub pclmulqdq: bool, ...  // 83 campos
    pub monitorx: bool, pub addr_mask_ext: bool,
    pub invtsc: bool,
}
```

El driver tenía que hacer:
```rust
if features.has_avx2 { /* siempre true */ }
if !features.has_avx512f { /* siempre true */ }
```

## Ahora (v1.7.8): constantes simples

```rust
// cpu/features.rs
pub const HAS_SSE: bool = true;
pub const HAS_SSE2: bool = true;
pub const HAS_SSE3: bool = true;
pub const HAS_SSSE3: bool = true;
pub const HAS_SSE4_1: bool = true;
pub const HAS_SSE4_2: bool = true;
pub const HAS_AVX: bool = true;
pub const HAS_AVX2: bool = true;
// ... etc, todo true en el 5600X
pub const HAS_AVX512F: bool = false;
pub const HAS_5LEVEL_PAGES: bool = false;
```

Y un struct compacto `CpuFeatures` para los que aún lo necesitan:
```rust
pub struct CpuFeatures {
    pub has_sse: bool, has_sse2: bool, has_avx: bool, has_avx2: bool,
    pub has_xsave: bool, has_osxsave: bool, has_fs_gs_base: bool,
    pub has_smep: bool, has_smap: bool, has_umip: bool,
    pub has_mtrr: bool, has_perfctr_core: bool,
}

impl CpuFeatures {
    pub const fn for_5600x() -> Self {
        // Todas las features son true en el 5600X
        Self { has_sse: true, /* ... */ }
    }
}
```

## Por qué funciona

El kernel se compila **específicamente** para el 5600X. Si el
usuario tiene otro CPU, el kernel panic con un mensaje claro al
boot:

```
ring0: CPU no es Ryzen 5 5600X
```

No hay `if` para vendor/family/model. El kernel se niega a
arrancar en otro hardware.

## ¿Qué cambió?

| Archivo | Antes | Ahora |
|---|---|---|
| `cpu/features.rs` | 194 LOC, 83 features | 100 LOC, 12 features + 28 constantes |
| `cpu/info.rs` | 60 LOC, `print(&features)` | 80 LOC, `print()` sin args |
| `platform/cpu.rs` | 700 LOC, `FeatureBitmap` + topology + cache | 320 LOC, solo CPUID de identificación |

## Si en el futuro tienes otro CPU

1. Edita `cpu/features.rs` — cambia las constantes
2. Edita `cpu/mod.rs::init` — ajusta los pasos de init
3. Edita `platform/cpu.rs` — el `detect()` debe verificar el CPU

Eso es todo. No hay `if` en el resto del kernel que se rompa.

## Estructura final de Ring 0 (v1.7.8)

```
ring0/
├── mod.rs                  — Entry point + re-exports
├── coordinator.rs          — Init orchestrator
├── panic.rs
│
├── platform/               — Identificación del hardware
│   ├── cpu.rs              — CpuIdentity (vendor/family/model/brand)
│   ├── chipset.rs          — ACPI tables (RSDP, MCFG)
│   ├── firmware.rs         — UEFI stub
│   └── topology.rs         — Re-export desde arch
│
├── arch/                   — CPU mode (Ring 0 ↔ Ring 3)
│   ├── gdt.rs              — GDT + TSS
│   ├── idt.rs              — IDT
│   ├── apic.rs             — Local + I/O APIC
│   ├── smp.rs              — INIT-SIPI-SIPI
│   ├── syscall.rs          — Syscall entry + dispatcher
│   ├── ctx.rs              — 15-GPR save/restore
│   └── topology.rs         — Topology + PerCpu
│
├── cpu/                    — CPU primitives
│   ├── mod.rs              — Re-exports + `init()` entry point
│   ├── features.rs         — Constantes del 5600X (HAS_SSE, HAS_AVX2, etc.)
│   ├── msr.rs              — MSR read/write + constants
│   ├── regs.rs             — CR0/CR2/CR3/CR4/XCR0
│   ├── cache.rs            — MTRR + PAT
│   ├── fpu.rs              — XSAVE/FXSAVE
│   ├── perf.rs             — Performance counters
│   ├── tsc.rs              — TSC + calibration
│   ├── info.rs             — Print CPU info to serial
│   └── delay.rs            — udelay/mdelay
│
├── mem/                    — Memory management
│   ├── heap.rs             — Bump heap
│   ├── phys.rs             — Frame allocator
│   ├── virt.rs             — Page tables
│   └── space.rs            — VMM / address spaces
│
├── dev/                    — Devices
│   ├── console.rs          — COM1
│   ├── framebuffer.rs      — UEFI GOP
│   ├── pcie.rs             — PCIe scan
│   ├── watchdog.rs
│   ├── audio.rs            — DSP math
│   └── acpi.rs             — ACPI control
│
├── proc/                   — Scheduler
│   ├── mod.rs              — Round-robin scheduler
│   ├── task.rs             — Task struct
│   ├── process.rs          — Process struct
│   ├── rt.rs               — Real-time (EDF)
│   └── user_init.rs        — Ring 3 init
│
├── boot/                   — Boot sequence
│   ├── mod.rs
│   ├── context.rs          — BootContext (DI)
│   ├── log.rs              — Legacy log shim
│   ├── serial.rs           — Hex/dec formatters
│   ├── visual.rs           — Splash screen
│   └── phases/
│       ├── p0_arch.rs      — Phase 0: arch init
│       ├── p1_mem.rs       — Phase 1: mem init
│       ├── p2_dev.rs       — Phase 2: dev init
│       ├── p3_proc.rs      — Phase 3: proc init
│       ├── p4_bmo.rs       — Phase 4: BMO init
│       ├── p5_user.rs      — Phase 5: Ring 3 init
│       └── trait_def.rs
│
├── hal/                    — API limpia para BMO Core (futuro)
│   ├── info.rs             — BootInfo (re-export)
│   ├── log.rs              — Logger per-subsystem
│   ├── result.rs           — KError + KResult
│   ├── sync.rs             — SpinLock, OnceCell
│   ├── delay.rs            — Re-export de cpu::delay
│   └── mmio.rs             — MmioRegion
│
├── result.rs               — (futuro: mover a hal/)
├── sync.rs                 — (futuro: mover a hal/)
└── log.rs                  — (futuro: mover a hal/)
```

## Lo que NO cambió (a propósito)

- **arch/**: GDT, IDT, APIC, SMP, syscall, ctx, topology — son del 5600X
  (registros, formatos). El SMP topology sí depende del CPU pero lo
  soportamos en `arch/topology.rs` con `Zen 3 APIC ID decoding`.
- **mem/**: page tables, VMM, heap — son genéricos a x86-64 pero no a
  otros CPUs. Para un ARM o RISC-V, hay que reescribir.
- **dev/**: drivers específicos. PCIe scan funciona en cualquier PCIe
  device, GOP funciona en cualquier UEFI, COM1 es x86.
- **proc/**: scheduler round-robin es CPU-agnostic.
- **boot/**: secuencia de fases. Cada phase llama a `arch::init`,
  `cpu::init`, `mem::init`, `dev::init`, etc.

## Resumen

- **Ring 0 = driver del 5600X**, no genérico.
- **Sin `if has_feature { ... }`** en el resto del kernel.
- **Sin traits, sin dyn, sin genéricos**.
- **Constantes** que son siempre `true` (o siempre `false`).
- **Si cambias CPU**, editas 1-2 archivos.
- **Drivers adicionales** (AMDGPU, NIC, etc.) se cargan de USB vía
  instalador, no viven aquí.
