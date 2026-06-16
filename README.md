# FastOS / BMO — Bare Metal Orchestrator

Sistema operativo bare metal escrito en Rust para AMD Ryzen 5 5600X. GPU por UEFI GOP (framebuffer). Sin dependencias de drivers propietarios.

---

## Arquitectura

```
┌────────────────────────────────────────────────────────────┐
│                    Ring 3 — Apps / ÑEXO                    │
│  ┌──────────┐ ┌──────────┐ ┌──────────┐ ┌──────────┐       │
│  │ ÑEXO CLI │ │ ByteDef  │ │ Restaur  │ │ BareX    │       │
│  └────┬─────┘ └────┬─────┘ └────┬─────┘ └─────┬────┘       │
│       │            │            │             │            │
│       └────────────┴───────┬────┴─────────────┘            │
│                            │ SYSCALL/SYSRET                │
├────────────────────────────┼───────────────────────────────┤
│                    Ring 0 — Kernel                         │
│  ┌──────────┐ ┌──────────┐ ┌──────────┐ ┌──────────┐       │
│  │ ByteDef  │ │ Restaur  │ │ Scheduler│ │ Memory   │       │
│  │ Antivirus│ │ Snapshots│ │ EDF+RR   │ │ DemandPg │       │
│  └──────────┘ └──────────┘ └──────────┘ └──────────┘       │
│  ┌──────────┐ ┌──────────┐ ┌──────────┐ ┌──────────┐       │
│  │ BareX    │ │ Filesys  │ │ Network  │ │ Diag     │       │
│  │ Graphics │ │ FAT32+BMO│ │ RTL8168  │ │ Overlay  │       │
│  └──────────┘ └──────────┘ └──────────┘ └──────────┘       │
│  ┌──────────┐ ┌──────────┐ ┌──────────┐ ┌──────────┐       │
│  │ APIC     │ │ SMP      │ │ ACPI/PCI │ │ USB/xHCI │       │
│  └──────────┘ └──────────┘ └──────────┘ └──────────┘       │
├────────────────────────────────────────────────────────────┤
│                    Hardware                                │
│  AMD Ryzen 5 5600X (Zen 3) │ UEFI GOP │ RTL8168 │ USB      │
└────────────────────────────────────────────────────────────┘
```

---

## Lo que funciona

### Core del kernel
- Boot UEFI → Kernel ELF → GOP framebuffer
- GDT + TSS con Ring 0 / Ring 3
- IDT con IST1 para #PF/#GP/#UD/#NM
- SYSCALL/SYSRET (IA32_LSTAR/STAR/FMASK)
- Page allocator con demand paging
- APIC timer (100 Hz) + I/O APIC
- SMP: INIT-SIPI-SIPI para Application Processors
- FPU lazy switching (CR0.TS → #NM → clear)
- MTRR + PAT para framebuffer Write-Combining
- Performance counters (instructions, cycles, branches, cache)
- CPUID completo para Zen 3 (SSE/AVX/AES-NI/SHA/etc)

### Drivers
- **GOP**: Framebuffer 1920x1080 con backbuffer
- **Serial**: COM1 115200 baud
- **PCI**: Enumeración por ACPI MCFG/ECAM
- **NVMe**: Detección + read (write deshabilitado)
- **AHCI**: Detección + read (write deshabilitado)
- **RTL8168**: NIC con TX/RX ring buffers, DHCP, ARP, IP, ICMP, UDP
- **USB/xHCI**: Detección + HID + Audio Class 2.0

### Archivos de sistema
- **BMO-FS**: Filesystem nativo con CLI (`bmofs`)
- **FAT32**: Read-only BPB parser
- **Ramdisk**: Archivos embebidos

### Diagnóstico
- Eventos `[INFO] [WARN] [FAULT] [PANIC]` por módulo
- Salida COM1 inmediata
- Overlay visual GOP con 6 pestañas (Overview, CPU, Memory, I/O, Scheduler, Log)
- Caja negra circular en RAM (256 eventos)
- Telemetría atómica (30+ contadores)

### Seguridad
- **ByteDefender**: Antivirus Ring 0 con pre-execution scanning
  - Análisis heurístico (shellcode, packing, ROP, heap spray)
  - Firma de amenazas conocidas (EICAR, ransomware, Metasploit)
  - Caché de 64 escaneos
  - Hooks de ejecución
- **Restaurer**: Snapshots del kernel en tiempo real
  - Guarda estado completo (page tables, processes, LAPIC, FPU, network)
  - Rollback con verificación checksum
  - Auto-snapshot cada 60 segundos
  - Diff entre snapshots

### Gráficos (BareX)
- GOP software backend
- fill_rect, draw_line (Bresenham), draw_circle
- fill_gradient_h/v, fill_rounded_rect
- draw_line_aa (anti-aliased Wu's algorithm)
- fill_polygon (scanline)
- Double buffering con present()
- Color blending y lerp

### Audio (BareX)
- USB Audio Class 2.0 backend
- 96K PCM buffer por voz
- Mixer con equal-power pan law
- Efectos: compressor, limiter, EQ 10-band, reverb
- Spatial audio (distance + horizontal angle)
- DSP math (sin, cos, sqrt, etc) sin librerías

### Input (BareX)
- BxInputSystem con bitmap de 256 teclas
- Mouse state con cursor modes
- Event polling

### Red (BareX)
- TCP/UDP sockets
- SPSC ring buffers para SQE/CQE
- NIC link up/down

### Lenguajes
- **ÑEXO**: CLI + runtime para Ring 3
- **BMOasm v0.3.0**: Parser + codegen (x86_64, AArch64, RISC-V)
- **nexo-sh**: Compilador de shaders (WGSL/GLSL → SPIR-V → BSF)

---

## Estructura del proyecto

```
FastOS/
├── bootloader/           # UEFI bootloader (Rust, x86_64-unknown-uefi)
├── boot_protocol/        # BootInfo struct compartido
├── kernel/               # Kernel principal (Rust, no_std)
│   └── src/
│       ├── arch/         # CPU, GDT, IDT, TSS, paging, SMP, APIC, FPU
│       ├── drivers/      # GOP, serial, PCI, NVMe, AHCI, RTL8168, USB
│       ├── fs/           # FAT32, BMO-FS, ramdisk, VFS
│       ├── memory/       # Page allocator, VMM, demand paging
│       ├── sched/        # Round-robin, EDF, process, thread
│       ├── diag/         # Eventos, buffer, overlay, telemetría
│       ├── desktop/      # Welcome screen, compositor, render
│       ├── barex/        # Graphics, audio, input, net, shader
│       ├── bef/          # BEF loader (PE/ELF/native)
│       ├── lang/         # ÑEXO, BMOasm
│       ├── security/     # ByteDefender, Restaurer
│       ├── syscall/      # Syscall dispatch
│       └── ui/           # Console, font, framebuffer
├── bmofs/                # BMO-FS CLI tool
├── nexo/                 # ÑEXO runtime (no_std)
├── nexo-sh-tool/         # Shader compiler (Naga + BLAKE3)
├── USB_boot/             # Archivos para USB boot
├── build_uefi.ps1        # Build + flash script
├── build_uefi.cmd        # Wrapper para PowerShell
├── BOOTX64.EFI           # Bootloader compilado
├── kernel.elf            # Kernel compilado
└── bmofs.img             # Imagen BMO-FS
```

---

## Archivos legacy (no son parte del kernel)

Estos archivos quedan como referencia pero no son necesarios para el boot:

| Archivo | Propósito | Estado |
|---------|-----------|--------|
| `generate_payload_v2.py` | Generador de payload GPU NVIDIA | Legacy (NVIDIA) |
| `write_gsp.ps1` | Writer de firmware GSP a SATA | Legacy (NVIDIA) |
| `write_payload.ps1` | Writer de payload a SATA | Legacy (GPU) |
| `combo_Window_Extractor/` | Reverse engineering de drivers Windows | Investigación |
| `fastos_framebuffer_epic_animation.html` | Demo visual HTML | Demo |
| `target_build/` | Artefactos de build | .gitignore |

---

## Build

```powershell
# Build completo + flash USB
.\build_uefi.ps1

# Solo compilar (sin flash)
.\build_uefi.ps1 -BuildOnly

# Solo flashear (ya compilado)
.\build_uefi.ps1 -FlashOnly

# Limpiar artefactos
.\build_uefi.ps1 -Clean
```

Requisitos:
- Rust nightly
- Target bare metal (`.cargo/config.toml`)
- UEFI con Secure Boot desactivado
- USB conectado

---

## Boot path

```
UEFI → BOOTX64.EFI → kernel.elf
  → Phase 0: CPU init (FPU/SSE/AVX/MTRR/PAT/perf)
  → Phase 1: Memory (page allocator, heap)
  → Phase 2: Devices (ACPI/PCI/NVMe/AHCI/RTL8168)
  → Phase 3: Display (GOP framebuffer)
  → Phase 4: Scheduler (APIC timer, SMP, Security)
  → Phase 5: Desktop (welcome → "Run" → desktop Ring 0)
```

---

## Syscalls disponibles

| Número | Nombre | Descripción |
|--------|--------|-------------|
| 0x00 | ProcessExit | Matar proceso actual |
| 0x03 | ThreadYield | Yield al scheduler |
| 0x20 | FileOpen | Abrir archivo |
| 0x21 | FileRead | Leer archivo |
| 0x23 | FileClose | Cerrar archivo |
| 0x25 | FileStat | Tamaño de archivo |
| 0x50 | ClockGetTime | Leer TSC |
| 0x51 | NanoSleep | Dormir nanosegundos |
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
| 0xA2 | SnapshotCreate | Crear snapshot del kernel |
| 0xA3 | SnapshotRollback | Retroceder a snapshot |
| 0xA4 | SnapshotList | Listar snapshots |
| 0xF0 | DebugPrint | Imprimir por serial |

---

## Hardware soportado

- **CPU**: AMD Ryzen 5 5600X (Zen 3, Family 19h)
- **RAM**: Cualquier tamaño detectado por UEFI memory map
- **GPU**: UEFI GOP framebuffer (cualquier GPU con UEFI)
- **NIC**: Realtek RTL8168/8111
- **Storage**: NVMe, AHCI (SATA)
- **USB**: xHCI (AMD Ryzen)
- **Audio**: USB Audio Class 2.0

---

## Próximos pasos

1. **AHCI write**: Habilitar escritura a disco
2. **Proceso Ring 3**: Lanzar "Hello World" desde Ring 3
3. **NIC link up**: Cable ethernet funcional
4. **Welcome screen**: Verificar pitch fix en hardware real
5. **SMP per-CPU scheduler**: Multi-core real
6. **ÑEXO commands**: Snapshot/Rollback desde CLI

---

## Principios

- **GOP primero**: Todo visual por framebuffer, sin drivers GPU
- **Ring 0 + Ring 3**: No Rings 1/2 (x86-64 moderno)
- **Sin firmware obligatorio**: Boot sin blobs privados
- **Modular**: Cada subsistema es independiente
- **Depurable**: Diag integrado desde el primer byte
