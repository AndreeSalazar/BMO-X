# Glosario del 5600X (Zen 3)

> Extraído de [`ryzen_5_5600x.md`](./ryzen_5_5600x.md) §Apéndice C.
> Definiciones de los términos técnicos usados en este directorio.

## Términos de hardware AMD

- **CCX (Core Complex):** grupo de cores que comparten L3 en Zen 3.
  El 5600X tiene **1 CCX con 6 cores activos** (de los 8 posibles en
  el die Vermeer; 2 cores deshabilitados por yield).
- **CCD (Core Complex Die):** el die físico donde viven los CCXs.
  Vermeer CCD contiene 1 CCX de 8 cores. El 5600X usa 1 CCD.
- **IOD (I/O Die):** die separado de 12 nm con el controlador de
  memoria, PCIe, USB, SATA, etc. En el 5600X está conectado al CCD
  por Infinity Fabric.
- **IF (Infinity Fabric):** interconexión entre CCDs y IOD. También
  referencia a la frecuencia del interconector (FCLK). En el 5600X
  FCLK = 1800 MHz (1:1 con MCLK a 3600 MHz DDR4).
- **SMT (Simultaneous Multithreading):** nombre AMD para
  Hyper-Threading. El 5600X tiene SMT habilitado (6C/12T).
- **Boost / Precision Boost 2:** algoritmo que sube la frecuencia de
  un core según la carga y los límites térmicos. Single-core boost
  del 5600X: 4.6 GHz.
- **PB2:** Precision Boost 2 (algoritmo de boost por core, no por
  package como PB1).
- **STAPM:** Skin Temperature Aware Power Management (límite por
  temperatura del package).
- **PPT:** Package Power Tracking (límite de potencia del socket).
- **TDC:** Thermal Design Current (límite de corriente sostenida).
- **EDC:** Electrical Design Current (límite de corriente pico).

## Términos de microarquitectura Zen 3

- **TAGE (TAgged GEometric history length):** predictor de branches
  usado en Zen 3. Reemplaza al predictor local/global de Zen 1/2.
- **Op Cache:** cache de µops decodificados (4096 entries en Zen 3).
  Alimenta el renamer sin pasar por el decoder.
- **BTB (Branch Target Buffer):** cache de targets de branches.
  Zen 3: L1 BTB 1024 entries, L2 BBT 6656 entries.
- **RAS (Return Address Stack):** pila de direcciones de retorno
  para predecir calls/returns. 32 entries en Zen 3.
- **µop (micro-op):** operación interna del CPU. Zen 3 usa µops
  fusionados (Macro-Op Fusion) para reducir presión en el Op Cache.
- **ROB (Reorder Buffer):** buffer donde las instrucciones se
  reordenan para ejecución out-of-order. 256 entries en Zen 3.
- **IQ (Instruction Queue / Scheduler):** scheduler de µops.
  Zen 3: 16 entries Integer + 32 entries FP/Vector.

## Términos de memoria y coherencia

- **TSO (Total Store Order):** modelo de memoria x86. AMD TSO es
  ligeramente **más débil** que Intel TSO: los loads pueden
  reordenarse con stores a direcciones distintas.
- **SFENCE / LFENCE / MFENCE:** instrucciones de fence de AMD.
  - **SFENCE:** ordering store-store
  - **LFENCE:** ordering load-load
  - **MFENCE:** full fence (load+store)
- **MTRR (Memory Type Range Registers):** registers que definen tipos
  de caché (UC, WC, WT, WB, WP) para rangos de memoria física.
  Zen 3: 8 pairs (PHYSMASK/PHYSBASE).
- **PAT (Page Attribute Table):** 8 entries que extienden MTRR
  a nivel de página. Entry 0 (WB) es el default; entry 1 (WC) es
  el más usado para framebuffers.
- **PCID (Process-Context Identifier):** tag de 12 bits en TLB
  entries para reducir flushes en CR3 changes.
- **ASID (Address Space ID):** sinónimo de PCID en AMD. 12 bits →
  4096 ASIDs.
- **INVLPGB:** instrucción de invalidación bulk de TLB (no en
  Zen 3, sí en Zen 4). En Zen 3 hay que usar INVLPG individual.
- **Cacheline:** 64 bytes. Crítico alinear structs compartidos
  entre cores (spinlocks, datos de coherencia).
- **Write-Combining (WC):** tipo de memoria que permite combinar
  múltiples writes en una sola transacción. Usado para framebuffers.

## Términos de interrupciones

- **IDT (Interrupt Descriptor Table):** tabla de 256 entries (8 bytes
  en 32-bit, 16 bytes en 64-bit) con handlers de excepciones y IRQs.
- **IST (Interrupt Stack Table):** mecanismo para que el CPU cargue
  un stack fijo sin consultar CR3. Usado en #DF, NMI, #MC. 8 entries
  disponibles; el 5600X permite IST en cualquier vector.
- **DPL (Descriptor Privilege Level):** 0-3, nivel de privilegio del
  gate. IDT gates típicos: DPL=0 (kernel only) o DPL=3 (user callable
  vía INT n).
- **Gate types:** interrupt gate (IF=0 al entrar), trap gate (IF=1),
  task gate (legacy, no usado en 64-bit).
- **EOI (End Of Interrupt):** escribir 0 al registro APIC 0x0B0 para
  señalar fin de IRQ.
- **IPI (Inter-Processor Interrupt):** mensaje entre cores vía APIC.
  Tipos: INIT, STARTUP, Deassert, fixed, lowest-priority, etc.

## Términos de APIC

- **LAPIC (Local APIC):** controlador de interrupciones privado de
  cada core. MMIO en `0xFEE00000` por defecto.
- **IOAPIC (I/O APIC):** controlador de interrupciones de plataforma
  (en el IOD del 5600X). Maneja los IRQ del chipset.
- **ICR (Interrupt Command Register):** par de MSRs (ICR Low 0x300,
  ICR High 0x310) para enviar IPIs. Soporta los delivery modes
  INIT, STARTUP, Deassert, fixed, lowest-priority, SMI, NMI.
- **TSC-deadline mode:** modo del LAPIC timer que dispara IRQ cuando
  TSC >= IA32_TSC_DEADLINE. Más preciso que el timer periódico
  clásico.
- **TPR (Task Priority Register):** registro APIC que enmascara
  interrupciones por prioridad. Útil para critical sections.

## Términos de TSC y timers

- **TSC (Time Stamp Counter):** contador de 64 bits que se incrementa
  a la frecuencia actual del CPU. **No es invariant** en Zen 3 (cambia
  con P-state). Cuidado al medir tiempo.
- **ITSC (Invariant TSC):** TSC que NO cambia con P-state. **No
  soportado en Zen 3** (sí en CPUs móviles de bajo TDP).
- **HPET (High Precision Event Timer):** timer de plataforma MMIO.
  Útil como respaldo si el LAPIC timer no está disponible.
- **PIT (Programmable Interval Timer):** timer legacy 8253/8254.
  Útil para calibrar el TSC al boot (cuenta a 1.193182 MHz).

## Términos de paging

- **PML4 (Page Map Level 4):** nivel raíz de la tabla de páginas
  en 64-bit. Apunta a 512 PDPTs.
- **PDPT (Page Directory Pointer Table):** nivel 3. Apunta a 512 PDs.
- **PD (Page Directory):** nivel 2. Apunta a 512 PTs o huge pages (2MB).
- **PT (Page Table):** nivel 1. Apunta a 512 páginas de 4KB o 1GB huge.
- **LA57 (5-Level Paging):** añade un nivel más (PML5). **No
  soportado en Zen 3** (sí en Zen 4 y CPUs Intel recientes).
- **Huge page:** páginas de 2MB (con PD bit PS=1) o 1GB (con PDPT
  bit PS=1). Mejor TLB reach, peor fragmentación.
- **NX (No-Execute):** bit 63 de PTE. Previene ejecución de código
  en esa página. Necesita EFER.NXE=1.
- **CR3:** register que apunta al PML4 físico. Cambiar CR3 cambia
  completamente el address space.
- **invlpg:** instrucción para invalidar una entrada de TLB específica.
- **WBINVD:** instrucción para escribir de vuelta todas las cachelines
  modificadas y vaciar la cache. Útil en shutdowns de SMM.

## Términos de seguridad (Spectre / Meltdown)

- **IBRS (Indirect Branch Restricted Speculation):** bit de
  `IA32_SPEC_CTRL`. Restringe speculation de branches indirectas.
  Activarlo en kernel entry.
- **STIBP (Single Thread Indirect Branch Predictors):** bit de
  `IA32_SPEC_CTRL`. Aísla predictors entre threads del mismo core
  (mitigación Spectre v2 cross-thread).
- **SSBD (Speculative Store Bypass Disable):** bit de
  `IA32_SPEC_CTRL`. Desactiva speculative store bypass.
- **MDS (Microarchitectural Data Sampling):** clase de vulnerabilidades
  (RIDL, Fallout, Zombieload). Mitigación: desactivar simultaneous
  multi-threading o usar `MD_CLEAR` per-thread.
- **SPEC_CTRL MSR:** 0x48. Bits 0=IBRS, 1=STIBP, 2=SSBD.
- **PRED_CMD MSR:** 0x49. Escribir 1 (= `IBPB`) para invalidar el
  Branch Predictor.

## Términos de P-states y energía

- **P-state:** estado de voltaje/frecuencia. P0 = max, P1 = base, Pn = min.
- **C-state:** estado de sueño. C0 = active, C1 = halt, C2 = stop grant,
  C3 = sleep.
- **ACPI P-state driver:** driver del kernel que cambia entre P-states
  según carga. Típicamente implementado como cpufreq.
- **ACPI C-state driver:** análogo para C-states.
- **HW P-state (CPPC):** Cooperative Processor Performance Control.
  Preferred P-state en AMD (vs. legacy P-state).

---

Para más definiciones, consultar:
- [AMD64 Architecture Programmer's Manual Vol. 1](https://www.amd.com/system/files/TechDocs/24594.pdf)
- [AMD64 Architecture Programmer's Manual Vol. 2 (System Programming)](https://www.amd.com/system/files/TechDocs/24596.pdf)
- [AMD64 Architecture Programmer's Manual Vol. 3 (General-Purpose and System Instructions)](https://www.amd.com/system/files/TechDocs/24597.pdf)
