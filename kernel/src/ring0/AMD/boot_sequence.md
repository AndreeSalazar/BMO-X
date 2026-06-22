# Apéndice A. Diagrama del startup del kernel

> Extraído de [`ryzen_5_5600x.md`](./ryzen_5_5600x.md) §Apéndice A.
> Secuencia completa desde el reset hasta el scheduler en el 5600X.

```
[Bios POST]
  -> 16-bit real mode
  -> 32-bit protected mode
  -> 64-bit long mode (UEFI: 64-bit entry)
  -> salta a entry de FastOS
[FastOS entry]
  -> CPUID check (panic si no es 5600X)
  -> deshabilitar interrupciones (CLI)
  -> construir GDT mínima
  -> construir IDT con stubs
  -> construir TSS con ISTs
  -> LGDT, LIDT, LTR
  -> configurar PML4/PDPT/PD/PT identity-map primeros 1 MB
  -> MTRR: 0..1MB = WB
  -> CR4.PAE = 1
  -> IA32_EFER: LME=1, NXE=1, SCE=1, FFXSR=1
  -> CR0.PG = 1 (entra a long mode)
  -> jmp far a código 64-bit
  -> configurar APIC_BASE (si no está)
  -> habilitar LAPIC local
  -> configurar timer LAPIC en TSC-deadline
  -> configurar SPEC_CTRL (IBRS=1)
  -> BSP: configurar IA32_STAR, IA32_LSTAR, IA32_FMASK
  -> habilitar SYSCALL/SYSRET (EFER.SCE=1)
  -> BSP: detectar cores online (CPUID.1:EBX[23:16])
  -> BSP: para cada AP (CPUID.0x8000001E:EBX[CoreId]):
      -> enviar INIT IPI
      -> esperar 10 ms
      -> enviar Deassert INIT IPI
      -> esperar 10 ms
      -> enviar STARTUP IPI (vector 0x08)
      -> esperar 200 µs
      -> enviar STARTUP IPI de nuevo
  -> BSP: esperar a que todos los APs firmen vida
  -> kernel main:scheduler(), userland setup, ...
[AP entry (vector 0x8000)]
  -> configurar su propio stack
  -> configurar LAPIC local
  -> configurar timer LAPIC
  -> habilitar SYSCALL/SYSRET
  -> configurar SPEC_CTRL
  -> firma de vida
  -> halt
  -> esperar scheduling
```

## Notas

- **BSP** = Bootstrap Processor (el primer core que arranca; en el
  5600X es siempre Core 0, Thread 0).
- **AP** = Application Processor (todos los demás cores; aquí cores 0-1,
  1-1, 2-1, 3-1, 4-1, 5-1 — los "thread 1" de cada core).
- El **trampoline** (vector 0x8000) es código real-mode que el AP ejecuta
  al despertarse. Típicamente vive en RAM baja (< 1 MB) y se copia a
  0x8000. Ver `arch::trampoline` cuando se implemente SMP en FastOS.
- **SPEC_CTRL** (IBRS/STIBP/SSBD) son mitigaciones Spectre v2 / MDS.
  Habilitarlas desde el inicio simplifica el código (no hay que
  recordar activarlas después).
- **UEFI path**: el bootloader UEFI de FastOS (en `bootloader/`) ya
  pone el CPU en long mode y carga el kernel. El path real de FastOS
  salta la sección "BSP: construir GDT mínima" — el GDT del
  bootloader es válido. La sección que SÍ ejecuta el kernel es la
  "configurar LAPIC local" en adelante.

## Tiempo típico de boot al primer scheduler tick

- Bootloader (UEFI → kernel): ~50-100 ms
- Phase 0 (arch): ~5 ms
- Phase 1 (mem): ~20 ms
- Phase 2 (dev): ~30 ms
- Phase 3 (display): ~50 ms (pintar splash)
- Phase 4 (proc/scheduler): ~10 ms
- **Total**: ~165-215 ms

Medido en el 5600X con 32 GB DDR4-3600. Tiempos varían con la
cantidad de RAM detectada (más RAM = más phase 1).
