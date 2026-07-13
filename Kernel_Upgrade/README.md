# FastOS — Kernel Upgrade (Faggin-Style Layered Boot)

## Filosofía

Cada etapa del boot es ultra-pequeña, minimalista, hace **una sola cosa** y pasa el control a la siguiente. El `BootContext` es la interfaz única entre etapas: un struct compartido que cada stage va llenando. El kernel en Ring 0 es el runtime vivo donde todo se despierta y se mantiene.

```
nano_wake → stage1_arch → stage2_mm → stage3_dev → kernel (Ring 0)
```

La razón de este diseño: **velocidad de boot**. Cada linker mide ~unos pocos KB, sin dependencias innecesarias. Cada etapa solo inicializa lo que toca y salta.

---

## Cadena de boot

### nano\_wake (stage0) — UEFI Bootloader
```
Entrada: UEFI firmware
Salida: BootContext con memory map, framebuffer, RSDP, direcciones de stages
```
- Cargado por UEFI como BOOTX64.EFI
- Lee los archivos stage1.bin, stage2.bin, stage3.bin, kernel.bin del ESP
- Obtiene GOP framebuffer (1920×1080 preferido)
- Obtiene memory map de UEFI
- Busca RSDP de ACPI
- Configura las direcciones físicas de cada stage en `BootContext.stage_base[]` / `stage_entry[]`
- Salta a stage1

### stage1\_arch — Arquitectura de CPU
```
Entrada: BootContext (memory map, framebuffer)
Salida: GDT + IDT + TSS + SYSCALL MSRs + CPU features + FPU + TSC + MTRR
```
| Subsystem | Qué hace |
|-----------|----------|
| Serial COM1 | 115200 8N1, inicializa debug output |
| GDT | Null, Kernel CS/DS, User DS/CS, TSS (16 bytes) |
| TSS | 3 RSP (kernel stack 16KB), 2 IST stacks (8KB c/u): IST1 para #PF/#GP/#DF, IST3 para #MC |
| IDT | 256 entradas, handlers `extern "x86-interrupt"` para todas las excepciones, #PF/#GP con CR2 dump |
| SYSCALL | MSRs STAR/LSTAR/FMASK, EFER.SCE, stub que hace `sysretq` |
| CPU detect | Vendor (AuthenticAMD/GenuineIntel), brand string (CPUID 0x80000002-04), features: XSAVE, SMEP, FSGSBASE, UMIP |
| CR0/CR4 | FPU enable (MP, NE), OSFXSR, OSXMMEXCPT, OSXSAVE, SMEP, FSGSBASE, UMIP |
| XCR0 | x87 + SSE + AVX bits via `xsetbv` |
| FPU | `fninit`, MXCSR 0x1F80, XSAVE capture initial state |
| TSC | Calibración por CPUID leaf 0x15, fallback 3.7 GHz (Ryzen 5 5600X) |
| MTRR/PAT | Default Write-Back, enable MTRRs |

**Salida a stage2**: `gdt_ptr`, `idt_ptr`, `tss_ptr`, `syscall_entry`, `tsc_freq`, `kernel_stack_top`

### stage2\_mm — Gestión de Memoria
```
Entrada: BootContext (con stage1 fields)
Salida: PML4 con identity map + higher-half + heap + frame allocator
```
| Subsystem | Qué hace |
|-----------|----------|
| phys\_to\_virt / virt\_to\_phys | `address + 0xFFFF_8000_0000_0000` |
| PML4 | Identity map 0-4MB, higher-half mirror 0-2GB, framebuffer WC |
| Frame allocator | Bitmap-based (1M frames ≈ 4TB), contiguous multi-page support |
| Buddy allocator | Órdenes 0-10 (4KB a 4MB), split/coalesce, list_head free lists |
| Slab heap | 8 size classes (16B a 2KB), free list por slab, buddy fallback |
| ACPI RSDP | Scan EBDA (0x40E) + BIOS ROM (0xE0000-0xFFFFF) |
| TLB flush | `invlpg` wrapper |

**Salida al kernel**: `pml4`, `heap_base`, `heap_size`

### stage3\_dev — Inicialización de Dispositivos
```
Entrada: BootContext (con stage1 + stage2 fields)
Salida: PCI devices + I/O APIC + LAPIC + HPET + i8042
```
| Subsystem | Qué hace |
|-----------|----------|
| ACPI parsing | RSDP → XSDT → MCFG (ECAM base y bus range), HPET (MMIO base), MADT (LAPIC addr, I/O APIC), FADT (PM timer port) |
| PCI enumeration | Si ECAM via MCFG → memory-mapped config reads, si no → IO ports 0xCF8/0xCFC. Escanea bus 0-255, guarda hasta 32 dispositivos en BootContext |
| I/O APIC | Lee ID y versión (reg 0/1), **mascara todas las entradas** del redirection table |
| LAPIC | SIVR enable (bit 8), TPR=0, LVT timer one-shot, divide by 16. Calibra con PIT (10ms via channel 2), configura periódico ~1000 Hz |
| HPET | Lee capacidad (period en femtosegundos), enable (bit 0 + legacy bit 1), resetea contador |
| i8042 PS/2 | Disable ports, test keyboard, enable IRQ1, enable scanning. Test mouse, enable IRQ12 + data reporting |

**Salida al kernel**: `ioapic_base`, `hpet_base`, `pci_count`, `pci_devices[]`, `rsdp`

---

## BootContext — La ABI entre etapas

```c
struct BootContext {
    // Stage 0 (nano_wake)
    magic: u64,           // "FOSCBOOT"
    version: u32,
    fb_addr, fb_width, fb_height, fb_stride, fb_pixel_format,
    memory_map_count, memory_map[64],
    rsdp: u64,
    stage_base[8], stage_size[8], stage_entry[8],

    // Stage 1 (stage1_arch)
    gdt_ptr, idt_ptr, tss_ptr, syscall_entry, tsc_freq, kernel_stack_top,

    // Stage 2 (stage2_mm)
    pml4, heap_base, heap_size,

    // Stage 3 (stage3_dev)
    ioapic_base, hpet_base, pci_count, pci_devices[32],

    // Kernel
    kernel_stack, ring3_stack,
    _reserved[32],
};
```

Cada etapa solo llena sus campos y salta a la siguiente. El kernel final lee todo.

---

## Layout de memoria física

| Rango | Uso |
|-------|-----|
| 0x000000 – 0x0FFFFF | BIOS / UEFI runtime (no tocar) |
| 0x100000 (1MB) | stage1\_arch |
| 0x200000 (2MB) | stage2\_mm |
| 0x300000 (3MB) | stage3\_dev |
| 0x400000 (4MB) | bmo-kernel-v2 |
| 0x1000000 (16MB)+ | Heap, PML4, page tables, frames |

Higher-half mapping: `0xFFFF_8000_0000_0000 + phys`

---

## Build

```powershell
.\build.ps1              # Solo compilar
.\build.ps1 -Flash       # Compilar + flashear a SSD (unidad S:)
.\build.ps1 -Flash -Drive D  # Flashear a unidad D:
.\build.ps1 -Clean       # Limpiar artefactos
```

Cada stage se compila desde su propio directorio con su propio `.cargo/config.toml` y linker script. `build.ps1` construye en orden: `nano_wake → stage1_arch → stage2_mm → stage3_dev → kernel`, convierte los ELF a binarios planos con `llvm-objcopy`, los copia a `staging/EFI/BOOT/` y opcionalmente a la SSD.

---

## Debug por serial

COM1 a 115200 8N1 imprime todo el progreso:

```
[stage1] Arch init — GDT, IDT, SYSCALL, CPU
[stage1] GDT loaded with TSS + IST stacks
[stage1] IDT loaded (256 entries)
[cpu] Vendor: AuthenticAMD
[cpu] AMD Ryzen 5 5600X 6-Core Processor
[cpu] XSAVE=Y SMEP=Y FSGSBASE=Y UMIP=Y
[cpu] CR0/CR4 configured
[cpu] XCR0 configured (x87 + SSE + AVX)
[cpu] FPU + MXCSR initialized
[cpu] TSC calibrated: 3700000000 Hz
[stage1] Context updated, jumping to stage2

[stage2] Memory init — PML4, allocators
[mm] PML4 at 0x10400000
[mm] Frame allocator ready (1024K frames)
[mm] Buddy allocator initialized (orders 0-10)
[mm] Slab heap ready (8 classes)
[mm] ACPI RSDP found at 0x7F66B000
[stage2] Context updated, jumping to stage3

[stage3] Device init — ACPI, PCI, APIC, HPET, i8042
[acpi] RSDP at 0x7F66B000
[acpi] MCFG: ECAM at 0xF0000000, bus 0-255
[acpi] HPET base at 0xFED00000
[acpi] LAPIC at 0xFEE00000
[acpi] I/O APIC at 0xFEC00000
[pci] Found 24 devices
[ioapic] All IRQs masked
[lapic] Timer initialized at ~1000 Hz
[hpet] Enabled
[i8042] PS/2 controller initialized
[stage3] Context updated, jumping to kernel
```

---

## Próximos pasos inmediatos

1. Compilar stage3_dev expandido y validar
2. Expandir kernel: scheduler round-robin con preempción por APIC timer, BMO Channel, HAL Services table
3. Ring 3 gateway: iretq + page tables de usuario, syscall dispatch completo
4. Module loader para cargar `.bmo` modules en Ring 3
5. SMP: INIT-SIPI-SIPI para despertar cores AP
