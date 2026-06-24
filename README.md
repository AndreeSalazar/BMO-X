# FastOS / BMO — Bare Metal Orchestrator

Sistema operativo bare metal escrito en Rust para AMD Ryzen 5 5600X. GPU por UEFI GOP (framebuffer). Sin dependencias de drivers propietarios.

**Versión**: 1.8.14 | **Bootloader**: v0.2.0 | **Estado**: Boot funcional + desktop Ring 0

---

## Arquitectura

```
┌────────────────────────────────────────────────────────────┐
│                    Ring 3 — Apps (próximo)                 │
│  ┌──────────┐ ┌──────────┐ ┌──────────┐ ┌──────────┐       │
│  │ BMO CLI  │ │ ByteDef  │ │ Restaur  │ │ BareX    │       │
│  └────┬─────┘ └────┬─────┘ └────┬─────┘ └────┬─────┘       │
│       │            │            │             │            │
│       └────────────┴───────┬────┴─────────────┘            │
│                            │ SYSCALL/SYSRET                │
├────────────────────────────┼───────────────────────────────┤
│                    Ring 0 — Kernel                         │
│  ┌──────────┐ ┌──────────┐ ┌──────────┐ ┌──────────┐       │
│  │ ByteDef  │ │ Restaur  │ │ Scheduler│ │ Memory   │       │
│  │ Antivirus│ │ Snapshots│ │ RoundRobin│ │ DemandPg │      │
│  └──────────┘ └──────────┘ └──────────┘ └──────────┘       │
│  ┌──────────┐ ┌──────────┐ ┌──────────┐ ┌──────────┐       │
│  │ Desktop  │ │ Filesys  │ │ Cabina   │ │ BMO Lang │       │
│  │ Welcome  │ │ Ramdisk  │ │ Diag HUD │ │ Compiler │       │
│  └──────────┘ └──────────┘ └──────────┘ └──────────┘       │
│  ┌──────────┐ ┌──────────┐ ┌──────────┐ ┌──────────┐       │
│  │ APIC     │ │ ACPI/PCI │ │ MTRR/PAT │ │ BEF Load │       │
│  └──────────┘ └──────────┘ └──────────┘ └──────────┘       │
├────────────────────────────────────────────────────────────┤
│                    Hardware                                │
│  AMD Ryzen 5 5600X (Zen 3) │ UEFI GOP │ COM1 Serial        │
└────────────────────────────────────────────────────────────┘
```

---

## Estado actual

### Funciona (real en hardware)
- **Boot UEFI** → BOOTX64.EFI → kernel.elf → GOP framebuffer 1920x1080
- **GDT + TSS** con Ring 0 / Ring 3, IST1 para excepciones
- **IDT** con 256 entradas, ISR stubs naked, handlers para #GP/#PF/#UD/#NM/#MF/#XM/#DE/#DF
- **SYSCALL/SYSRET** (IA32_LSTAR/STAR/FMASK) con dispatcher ~25 syscalls
- **Page allocator** bitmap-based (16 MB – 4 GB) desde UEFI memory map
- **Heap** free-list 32 MB con coalescing
- **VMM** 4 niveles (PML4/PDPT/PD/PT), demand paging + CoW
- **Local APIC** con calibración PIT (timer periódico, deshabilitado temporalmente)
- **MTRR + PAT** para framebuffer Write-Combining
- **Performance counters** (3 fixed counters)
- **ACPI** RSDP/XSDT/MCFG/FADT parsing
- **PCI** enumeración por I/O ports (ECAM deshabilitado)
- **GOP Framebuffer** con backbuffer, primitivas gráficas completas
- **Serial COM1** 115200 baud con timeout guard
- **Round-robin scheduler** (cooperativo, sin preempción)
- **Ring 0 → Ring 3** transición via iretq con user page tables
- **BMO Language** compilador AOT x86-64 (lexer, parser, sema, codegen)
- **BEF Loader** nativo + PE + ELF con relocations/imports/exports/TLS
- **Cabina** sistema de diagnóstico con eventos, overlay HUD, telemetría
- **Desktop** welcome screen + render + wallpaper + input
- **BMO API v2.0** 256 syscalls + Window Manager + Paint Compositor
- **AMD Zen 3** detección CPUID, errata workarounds, TSC calibration

### Parcial / Stub
- **ByteDefender**: Solo validación de headers BEF (sin análisis heurístico)
- **Restaurer/TimeBack**: API existe pero capture retorna zeros, rollback no hace nada
- **FPU lazy switching**: `init_fpu()` funciona, pero #NM mata el proceso (sin save/restore per-task)
- **PCI**: Código completo pero deshabilitado (I/O ports bloquea CPU, ECAM causa #PF)
- **SMP**: Detección de topology funciona, AP startup removido en v1.8.7
- **BMO GPU**: RDNA4 skeleton sin driver real
- **Shader BSF**: Loader/validator existe, BLAKE3 es placeholder

### No existe (removido o nunca implementado)
- ~~NVMe driver~~ — Solo check de PCI class
- ~~AHCI driver~~ — Solo check de PCI class
- ~~RTL8168 NIC~~ — Sin código en todo el codebase
- ~~USB/xHCI~~ — Solo check de PCI class
- ~~I/O APIC~~ — Solo mención en comentarios
- ~~SMP AP startup~~ — Removido en v1.8.7
- ~~EDF scheduler~~ — Removido en v1.8.7
- ~~FAT32~~ — Removido en v1.8.8
- ~~BMOasm assembler~~ — No existe
- ~~nexo-sh-tool~~ — No existe
- ~~BareX network stack~~ — Sin código
- ~~BareX audio hardware~~ — Solo beep()

---

## Estructura del proyecto

```
FastOS/
├── bootloader/              # UEFI bootloader (Rust, x86_64-unknown-uefi)
│   └── src/main.rs          # ELF loader, GOP, RSDP, memory map, jump to kernel
├── boot_protocol/           # BootInfo struct compartido bootloader ↔ kernel
├── kernel/                  # Kernel principal (Rust, no_std, x86_64-unknown-none)
│   └── src/
│       ├── ring0/           # Hardware abstraction layer
│       │   ├── mod.rs       # _start (entry point), BSS zero, kernel_main_real
│       │   ├── coordinator.rs   # Boot orchestration (phases 0-4, init, welcome)
│       │   ├── arch/        # GDT, IDT, TSS, APIC, SYSCALL, context switch
│       │   │   ├── gdt.rs       # 7-entry GDT + TSS + LGDT/LTR assembly
│       │   │   ├── idt.rs       # 256-entry IDT + naked ISR stubs + exception handlers
│       │   │   ├── apic.rs      # Local APIC + PIT calibration + periodic timer
│       │   │   └── syscall.rs   # SYSCALL MSRs + naked entry + ~25 syscall dispatcher
│       │   ├── mem/         # Memory management
│       │   │   ├── phys.rs      # Bitmap page frame allocator (16 MB – 4 GB)
│       │   │   ├── heap.rs      # 32 MB free-list heap with coalescing
│       │   │   ├── virt.rs      # 4-level page tables, demand paging, CoW, user mapping
│       │   │   └── space.rs     # AddressSpace + VMA tracking
│       │   ├── dev/         # Device drivers
│       │   │   ├── framebuffer.rs   # GOP + backbuffer + graphics primitives
│       │   │   ├── console.rs       # COM1 serial 115200 baud
│       │   │   ├── pcie.rs          # PCI enumeration (IO ports + ECAM)
│       │   │   └── acpi.rs          # ACPI table parsing
│       │   ├── cpu/         # CPU features
│       │   │   ├── mod.rs      # CPUID, FPU, MSR, TSC, perf counters
│       │   │   └── perf.rs     # Fixed performance counters
│       │   ├── proc/        # Process management
│       │   │   ├── task.rs     # Task table (256), round-robin pick_next
│       │   │   ├── process.rs  # Process table (64), PID, capabilities
│       │   │   ├── mod.rs      # Scheduler with CR3 switching + IBPB
│       │   │   └── user_init.rs # Ring 3 process creation + jump via iretq
│       │   ├── boot/        # Boot phases
│       │   │   └── phases/
│       │   │       ├── p0_arch.rs   # GDT + IDT + SYSCALL + CPU init
│       │   │       ├── p1_mem.rs    # Page allocator + heap
│       │   │       ├── p2_dev.rs    # ACPI + PCI
│       │   │       ├── p3_display.rs # GOP framebuffer
│       │   │       └── p4_bmo.rs    # Process tables (cooperative mode)
│       │   ├── vendor/amd/  # AMD Zen 3 specific
│       │   │   └── cpu/zen3/  # CPUID, MTRR/PAT, TSC, errata, topology
│       │   ├── bus/         # Bus abstraction
│       │   ├── gpu/         # GPU abstraction
│       │   ├── syscall/     # Syscall numbers + dispatch
│       │   ├── profile/     # Profiling
│       │   ├── security/    # Execution hooks (stubs)
│       │   ├── snapshot/    # Snapshot marks (stubs)
│       │   └── diag_min/    # Minimal diagnostics (blackbox, serial)
│       ├── bmo_core/        # Core OS services
│       │   ├── bmo_core.rs  # coord::init() + coord::enter() — boot final
│       │   ├── desktop/     # Desktop environment
│       │   │   ├── welcome.rs    # Welcome screen (currently ultra-minimal)
│       │   │   ├── render.rs     # Full frame rendering (wallpaper, windows, dock)
│       │   │   ├── compositor.rs # Ring 3 compositor payload builder
│       │   │   ├── wallpaper.rs  # Procedural wallpaper
│       │   │   ├── input.rs      # PS/2 keyboard + mouse
│       │   │   ├── commands.rs   # Command dispatch
│       │   │   ├── windows.rs    # Window management
│       │   │   ├── state.rs      # Desktop state
│       │   │   ├── theme.rs      # Color themes
│       │   │   ├── sound.rs      # Beep()
│       │   │   └── display.rs    # Display abstraction
│       │   ├── desktop3/    # Ring 3 → Ring 0 gateway
│       │   ├── ui/          # Console, font (8x16 bitmap), framebuffer abstraction
│       │   ├── fs/          # Filesystem (ramdisk only, FAT32 removed)
│       │   ├── bef/         # BEF binary format + loaders (native/PE/ELF)
│       │   ├── bmo_api/     # BMO API v2.0: 256 syscalls + WM + paint
│       │   └── bmo_gpu_api/ # GPU API bridge
│       ├── cabina/          # Diagnostics cockpit (26 archivos)
│       │   ├── events/      # Event system (5 niveles, buffer circular 256)
│       │   ├── telemetry/   # Atomic telemetry (30+ counters)
│       │   ├── panels/      # HUD panels (boot, CPU, GPU, I/O, mem, sched, etc.)
│       │   └── paint/       # Overlay primitives
│       ├── defense/         # ByteDefender (framework, no scanning real)
│       ├── timeback/        # Restaurer (framework, no capture real)
│       ├── lang/            # BMO Language compiler
│       │   ├── bmo/         # Lexer, parser, sema, AOT x86-64 backend
│       │   ├── c/           # C frontend (preprocessor, lexer, parser, translator)
│       │   ├── backends/    # AOT x86_64 codegen
│       │   └── bef/         # BEF format (header, sections, relocations, imports)
│       ├── bmo_gpu/         # GPU bridge + BSF shader loader
│       └── userland/        # Ring 3 process stub (v1.8.8)
├── bmo_abi/                 # ABI definitions (syscalls, types, BEF format)
├── bmo_audio/               # Audio crate (stub)
├── build_uefi.ps1           # Build + flash script (PowerShell)
└── linker.ld                # Kernel linker script
```

---

## Build

```powershell
# Build completo + flash USB
powershell -ExecutionPolicy Bypass -File .\build_uefi.ps1 -Flash

# Solo compilar (sin flash)
.\build_uefi.ps1 -BuildOnly

# Solo flashear (ya compilado)
.\build_uefi.ps1 -FlashOnly

# Limpiar artefactos
.\build_uefi.ps1 -Clean
```

Requisitos:
- Rust **stable** (kernel) + **nightly** (bootloader)
- UEFI con Secure Boot desactivado
- USB conectado

El script build_uefi.ps1 ejecuta:
1. Build bootloader (nightly, `x86_64-unknown-uefi`) → `fastos-bootloader.efi`
2. Build kernel (stable, `x86_64-unknown-none`) → `fastos-kernel` ELF (~706 KB)
3. Crea staging `EFI/BOOT/` con BOOTX64.EFI + kernel.elf
4. Copia a USB + verifica SHA256

---

## Boot path

```
UEFI Firmware
  → BOOTX64.EFI (bootloader)
    1. Leer kernel.elf del ESP (SimpleFileSystem)
    2. Parsear ELF64, cargar segments (PT_LOAD)
    3. Query GOP (1920x1080 BGR preferido)
    4. Buscar RSDP (ACPI 2.0/1.0)
    5. Allocate stack (4 MiB) + BootInfo
    6. Exit boot services
    7. Jump to kernel (RSP=stack_top, RDI=boot_info_ptr)
  → _start (kernel entry)
    1. Save RDI → R12 (antes de BSS zero)
    2. Zero-init BSS
    3. Restore RDI, call kernel_main_real()
  → coordinator::main()
    Phase 0 (p0_arch):  GDT + IDT + SYSCALL MSRs + CPU init (FPU/MTRR/PAT/TSC)
    Phase 1 (p1_mem):   Page allocator + 32 MB heap + smoke test
    Phase 2 (p2_dev):   ACPI MCFG + PCI (IO ports, ECAM deshabilitado)
    Phase 3 (p3_display): GOP framebuffer init
    Phase 4 (p4_bmo):   Process tables (cooperativo, sin APIC/interrupts)
    → init_fastos_cpu():  AMD Zen 3: CPUID, cache, TSC, errata
    → init_acpi():        ACPI tables
    → bmo_core::coord::init(): Cabina + Defense + TimeBack + FS + GPU + BEF + API + Desktop
    → bmo_core::coord::enter():
      1. Clear splash
      2. Init bmo_audio
      3. Play logon chime
      4. welcome::run() ← no retorna
```

---

## Syscalls disponibles

### Kernel syscalls (Ring 0 → Ring 0)

| Número | Nombre | Descripción |
|--------|--------|-------------|
| 0x00 | ProcessExit | Matar proceso actual |
| 0x03 | ThreadYield | Yield al scheduler |
| 0x20 | FileOpen | Abrir archivo |
| 0x21 | FileRead | Leer archivo |
| 0x23 | FileClose | Cerrar archivo |
| 0x25 | FileStat | Tamaño de archivo |
| 0x50 | ClockGetTime | Leer TSC |
| 0x51 | NanoSleep | Dormir (busy-wait) |
| 0x60 | FbInfo | Info del framebuffer |
| 0x61 | FbFill | Rellenar rectángulo |
| 0x62 | FbText | Dibujar texto |
| 0x63 | FbPresent | Present (noop) |
| 0x64 | FbBlit | Blit de imagen |
| 0x65 | DesktopFrame | Render frame completo |
| 0x70 | KeyPoll | Poll teclado PS/2 |
| 0x71 | MousePoll | Poll ratón PS/2 |
| 0x80 | Beep | Beep por frecuencia |
| 0xA0 | BD_Scan | Escanear archivo (ByteDefender) |
| 0xA1 | BD_Status | Estado de ByteDefender |
| 0xA2 | SnapshotCreate | Crear snapshot |
| 0xA3 | SnapshotRollback | Retroceder a snapshot |
| 0xA4 | SnapshotList | Listar snapshots |
| 0xF0 | DebugPrint | Imprimir por serial |

### BMO API v2.0 (Ring 3 → Ring 0, 0x100..0x1FF)

Dispatcher con validación + Cabina audit + ByteDefender check. Window Manager, Paint Compositor, Surface management, Timer wheel, Input events.

---

## Hardware

- **CPU**: AMD Ryzen 5 5600X (Zen 3, Family 19h, 6C/12T)
- **RAM**: Cualquier tamaño (detectado por UEFI memory map)
- **GPU**: UEFI GOP framebuffer (1920x1080 BGR, backbuffer)
- **Serial**: COM1 115200 baud (debug output)
- **PCI**: Enumeración por I/O ports (0xCF8/0xCFC)
- **ACPI**: RSDP/XSDT/MCFG/FADT

---

## Próximos pasos

1. **Fix boot #GP**: Conectar serial cable para diagnosticar crash exacto
2. **Restaurar welcome completo**: Rehabilitar render/input/commands paso a paso
3. **Re-habilitar APIC timer**: Preemptive scheduling (causaba #GP, necesita fix)
4. **NVMe/AHCI**: Drivers de almacenamiento
5. **RTL8168**: Driver de red
6. **USB/xHCI**: Drivers USB
7. **SMP**: Multi-core (INIT-SIPI-SIPI restaurado)
8. **I/O APIC**: IRQ routing
9. **FPU per-task**: Save/restore con XSAVE/XRSTOR
10. **PML4 propio**: Separación kernel/usuario real (sin UEFI identity map)

---

## Principios

- **GOP primero**: Todo visual por framebuffer, sin drivers GPU propietarios
- **Ring 0 + Ring 3**: Sin Rings 1/2 (x86-64 moderno)
- **Serial debug**: COM1 como canal de diagnóstico primario
- **Cooperativo**: Scheduler sin preempción (temporal, hasta fix del #GP)
- **Modular**: Cada subsistema es independiente (ring0, bmo_core, cabina, defense, etc.)
- **Depurable**: Diag integrado desde el primer byte
