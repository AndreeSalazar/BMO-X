# AMD Ryzen 5 5600X — Documentación Técnica de Bajo Nivel

**Público objetivo:** programador de kernel bare-metal sobre FastOS.
**Ámbito:** CPUID, registros, excepciones, paging, APIC, MSRs, ordenamiento de memoria, TSC, P-states, erratas, comparación con generaciones vecinas.

> Este documento **no es marketing**. Es una referencia técnica de bajo nivel. Cubre el 5600X (Vermeer, Zen 3, Family 19h Model 01h) y la microarquitectura Zen 3 en general. Cuando algo no se ha podido verificar de primera mano desde la documentación de AMD, se indica explícitamente con **[no verificado]** o **[estimación]**.

---

## Tabla de contenidos

1.  [Identificación del CPU](#1-identificaci%C3%B3n-del-cpu)
2.  [Topología física y SMT](#2-topolog%C3%ADa-f%C3%ADsica-y-smt)
3.  [Microarquitectura Zen 3](#3-microarquitectura-zen-3)
4.  [CPUID: leaves importantes](#4-cpuid-leaves-importantes)
5.  [Ordenamiento de memoria (TSO débil)](#5-ordenamiento-de-memoria-tso-d%C3%A9bil)
6.  [Cache, TLB y coherencia](#6-cache-tlb-y-coherencia)
7.  [Paging y memoria virtual](#7-paging-y-memoria-virtual)
8.  [Excepciones e IDT](#8-excepciones-e-idt)
9.  [Local APIC](#9-local-apic)
10. [MSRs fundamentales](#10-msrs-fundamentales)
11. [SYSCALL / SYSRET (ABI AMD64)](#11-syscall--sysret-abi-amd64)
12. [TSC y timers](#12-tsc-y-timers)
13. [P-states, C-states y boost](#13-p-states-c-states-y-boost)
14. [MTRR y PAT](#14-mtrr-y-pat)
15. [Erratas relevantes](#15-erratas-relevantes)
16. [Zen 3 vs Zen 2 vs Zen 4](#16-zen-3-vs-zen-2-vs-zen-4)
17. [Recursos oficiales y enlaces](#17-recursos-oficiales-y-enlaces)
18. [Practical notes for kernel development](#18-practical-notes-for-kernel-development)

---

## 0. Ficha técnica (resumen)

| Parámetro | Valor |
|---|---|
| Microarquitectura | Zen 3 |
| Code name (package) | Vermeer |
| Familia / Modelo (CPUID) | 0x19 / 0x01 (Family 19h, Model 01h) |
| Proceso de fabricación | TSMC 7FF (N7) |
| Transistores (CCD 6C) | 4.15 B en el CCD, 2.09 B en el I/O die 12 nm |
| Núcleos / hilos (este SKU) | 6C / 12T (SMT/HTT habilitado) |
| Frecuencia base | 3.7 GHz |
| Frecuencia boost máxima | 4.6 GHz (single-core, Precision Boost 2) |
| L1d por core | 32 KB, 8-way set-assoc, línea 64 B, write-back |
| L1i por core | 32 KB, 8-way, línea 64 B |
| L2 por core | 512 KB, 8-way, 12 ciclos latencia, write-back, exclusiva |
| L3 total | 32 MB (CCX de 8 cores, victim cache, 16-way) |
| Cacheline | 64 bytes (alinear structs manualmente) |
| Socket | AM4 (PGA) |
| TDP | 65 W |
| Memoria oficial | DDR4-3200 dual channel (UDIMM, sin ECC) |
| PCIe | 24 carriles PCIe 4.0 (4 al chipset) |
| Direcciones virtuales | 48 bits (256 TiB canónicos) |
| Direcciones físicas | 40 bits (1 TiB) — no PAE de 52 bits en este SKU |
| TLB L1I / L1D | 64 / 64 entradas (4 KB) — ver §3 |
| TLB L2 | 2048 entradas unificadas (Zen 3) |

Fuentes principales: AMD64 Architecture Programmer's Manual Vol. 1-3, Wikipedia [Zen 3](https://en.wikipedia.org/wiki/Zen_3), [Ryzen](https://en.wikipedia.org/wiki/Ryzen); AnandTech "Zen 3 architecture" (Cutress, 2020); AMD product page.

> ⚠️ **Convención de este documento:** "MSR 0x10" = `IA32_TSC`. Para MSRs AMD-específicos uso el prefijo `MSR_` o el número directo en hex. Los registros del APIC van siempre en hexadecimal con su offset desde el `APIC_BASE`.

---

## 1. Identificación del CPU

### 1.1 Vendor string

`CPUID` con `EAX=0` devuelve la cadena de 12 bytes en `EBX, EDX, ECX` (en ese orden). Para el 5600X:

```
EBX = 0x68747541  ("htuA")
EDX = 0x69746e65  ("itne")
ECX = 0x444d4163  ("DMAc")
→ "AuthenticAMD"
```

### 1.2 Brand string

`CPUID` con `EAX=0x80000002..0x80000004` (48 bytes, 3 calls de 16 bytes cada una) devuelve:

```
"AMD Ryzen 5 5600X 6-Core Processor                "
```

> Nota: el 5600X *no* tiene GPU integrada. En los Ryzen 5xxx "G" (Cezanne) y los Threadripper la cadena termina con "with Radeon Graphics" o similar. La ausencia de esa coletilla en el 5600X es un detalle útil para validación.

### 1.3 Familia / Modelo / Stepping

`CPUID.1: EAX` codifica identificación:

```
EAX[3:0]   = Stepping ID
EAX[7:4]   = Base Model
EAX[11:8]  = Base Family (0xF si extended family se usa)
EAX[13:12] = Reserved
EAX[19:16] = Extended Model
EAX[27:20] = Extended Family
```

Para Zen 3 desktop (Vermeer), el patrón es:

| Campo | Valor | Cómo se calcula |
|---|---|---|
| Base family | 0xF | indica que se suma extended |
| Extended family | 0x1 | 0xF + 0x1 = 0x19 (Family 19h) |
| Base model | 0x1 | |
| Extended model | 0x0 | 0x1 + 0x0<<4 = 0x01 (Model 01h) |
| **Familia efectiva** | **0x19 (25)** | |
| **Modelo efectivo** | **0x01 (1)** | |
| **Stepping** | 0x1 (B0) o 0x2 (B2) | depende del stepping físico |

> El stepping **B0** fue el primer silicio retail del 5600X (CCD Vermeer B0). Una revisión B2 con microcode actualizado llegó en BIOS AGESA más recientes (cambia CPUID family 19h model 1 stepping 1 → 1 en valores internos de stepping, sin cambiar la familia/modelo principal).

**Implementación sugerida en Rust:**

```rust
let (eax, _, _, _) = cpuid(1, 0);
let stepping = eax & 0xF;
let base_model = (eax >> 4) & 0xF;
let base_family = (eax >> 8) & 0xF;
let ext_model = (eax >> 16) & 0xF;
let ext_family = (eax >> 20) & 0xFF;

let family = if base_family == 0xF { base_family + ext_family } else { base_family };
let model  = if family >= 0x6 { base_model | (ext_model << 4) } else { base_model };

// 5600X: family == 0x19, model == 0x1
assert_eq!(family, 0x19);
assert_eq!(model, 0x1);
```

### 1.4 Maximum leaf

| CPUID | EAX | Significado |
|---|---|---|
| `0x0` | `0x1` (o mayor) | max basic leaf |
| `0x80000000` | `0x80000020` típico en Zen 3 (alcanza leaf `0x80000020`) | max extended leaf |

---

## 2. Topología física y SMT

El 5600X tiene:

* **1 CCD** (Core Complex Die) de 8 cores, de los cuales **2 están deshabilitados** (origen: binning; el CCD puede entregar cualquier configuración 4/6/8 activa).
* **1 I/O die** (IOD) de 12 nm separado, con Infinity Fabric, memoria DDR4, PCIe, USB, SATA.
* Cada core físico expone **2 threads lógicos** vía SMT (Simultaneous Multithreading, equivalente a Intel HTT).
* El SMT es **siempre 2-way en Zen 3** (no hay 4-way como en servidores POWER).

### 2.1 Layout físico

```
┌──────────────────┐  ┌──────────────────────────┐
│  CCD (Vermeer)   │  │   I/O Die (12 nm)        │
│  TSMC 7FF        │  │                          │
│                  │  │  - 2 ch DDR4             │
│  [CCX: 8 cores]  │◄─┤  - PCIe 4.0 root         │
│  · L1d 32 KB     │  │  - USB, SATA, audio      │
│  · L1i 32 KB     │  │  - Infinity Fabric PHY   │
│  · L2 512 KB     │  └──────────────────────────┘
│  · L3 32 MB (ccx)│
│  (2 cores OFF)   │
└──────────────────┘
       IFIS / GMI
```

### 2.2 Detección de la topología en el kernel

En Zen 3, la topología se deduce combinando varias hojas:

* `CPUID.1:EBX[23:16]` → `MaxAddrIDsInPackage` = número de APIC IDs únicos en el paquete. Para 5600X = 12 (6C × 2T).
* `CPUID.0x8000001E:EAX[7:0]` = `ExtendedApicId` del core actual. Bits usados:
  * `EAX[7:0]` = ID APIC extendido.
  * `EBX[15:0]` = **CoreId** (identifica el core físico dentro del CCX; el 5600X usa CoreId 0..5).
  * `ECX[7:0]` = **ThreadsPerComputeUnit** (siempre 2 en Zen 3).

Para el 5600X: Core IDs en uso = {0, 1, 2, 3, 4, 5} (2 cores deshabilitados dentro del CCX 0).

```rust
// Pseudocódigo para identificar el core
let (eax, ebx, ecx, _edx) = cpuid(0x8000_001E, 0);
let threads_per_core = (ecx & 0xFF) + 1;     // +1: codificado como n-1
let core_id          = (ebx & 0xFFFF) as u8; // único dentro del CCX
let smt_id           = (eax & 0x1) as u8;    // 0 o 1 en Zen 3
```

---

## 3. Microarquitectura Zen 3

Zen 3 rediseñó el core de arriba a abajo. Es **el primer rediseño "ground-up"** desde Zen 1 (2017). Mejoras clave vs Zen 2:

| Métrica | Zen 2 | Zen 3 |
|---|---|---|
| Issue / Retire width | 7 µops | **10 µops** |
| Decode width | 4-wide | 4-wide |
| Rename: integer PRF | 180 | **192** |
| Rename: FP PRF | 160 | **160** |
| Integer scheduler (ROB entries) | 92 | **96** |
| ROB (reorder buffer) | 224 | **256** |
| Scheduler (int) | 16 | 16 |
| Scheduler (FP) | 32 | 32 |
| Front-end: µop cache | 4096 µops | 4096 µops |
| L1 BTB entries | 512 | **1024** |
| L2 BTB entries | 4096 | **6656** |
| RAS (return address stack) | 27 | 32 |
| FMA latency | 5 ciclos | **4 ciclos** |
| DIV/IDIV latency | 16–46 | 10–20 |
| L1 TLB (I/D, 4K) | 64 / 64 | 64 / 64 |
| L2 TLB (unified, 4K) | 2048 | 2048 |
| L1d size | 32 KB | 32 KB |
| L2 size | 512 KB | 512 KB |
| L3 latency | 39 ciclos | **46 ciclos** |
| L3 organization | 2 × 16 MB (2 CCX × 4 cores) | **1 × 32 MB (1 CCX × 8 cores)** |

Fuentes: AnandTech (Cutress, 5/11/2020), Wikipedia [Zen 3](https://en.wikipedia.org/wiki/Zen_3), Chips and Cheese. Para "scheduler entries" y "issue width" hay cifras ligeramente diferentes según la fuente; los valores listados arriba son los de AnandTech.

### 3.1 Front-end

* **Predecesor / µop cache:** Zen 3 mantiene µop cache de 4 K entradas, 8-way.
* **Decoder:** 4-wide. Si una instrucción no cabe en el "fast path" (legacy decode), se microcodifica en µops múltiples.
* **Branch predictor:** basado en **TAGE** (TAgged GEometric history length). El predictor de indirect branches usa perceptron. L1 BTB 1K, L2 BTB 6.5K. RAS 32 entradas.
* **Branch target alignment:** la instrucción siguiente a un branch taken no se puede fusionar con un µopcache boundary; el branch target debe caer en una dirección alineada a 32B para máximo throughput.

### 3.2 Back-end y unidades de ejecución

* **Rename:** PRF entero de 192, PRF vectorial de 160.
* **Issue / Retire:** hasta 6 instrucciones/cycle (10 µops nominales peak).
* **Unidades integer:**
  * 4 ALU (ADD, SUB, AND, OR, XOR, shifts, rotates, MUL, etc.).
  * 2 AGU (Address Generation Units, una para load, otra para store).
  * Branch unit dedicada.
* **Unidades vectoriales (FP / SIMD):**
  * 2 × FMA de 256 bits (con AVX2: dos 128-bit FMA; con FMA3 se ejecutan como 256-bit).
  * 1 × ADD/SHUFFLE de 256 bits.
  * 1 × DIV/SQRT (lenta, latencia ~10-20 ciclos).
* **Store:** 2 store pipes.

### 3.3 Load/Store y memoria

* 2 load pipes (LD0, LD1), 1 store pipe (ST0) con generación de address, 1 store data pipe.
* **Memory disambiguation:** agresiva. Los stores pueden ser "rescheduled" antes que loads que el predictor determine como no dependientes. Implicación para kernel: no asumir orden store→load, usar fences (ver §5).
* **TLB L1:** 64 entradas I + 64 D, fully assoc, 4K. L2 TLB unificada de 2048 entradas, 8-way, cobertura 4K/2M/1G.
* **PCID** (Process Context ID): 12 bits (4096 IDs). Activable vía `CR4.PCIDE`.

### 3.4 Caché

* **L1d:** 32 KB, 8-way, write-back, write-allocate. **4 cycles load-to-use** para hits. Política: pseudo-LRU.
* **L1i:** 32 KB, 8-way, línea 64B. Con µop cache, L1i hit rate efectiva mejora.
* **L2:** 512 KB, 8-way, **12 cycles** latencia, exclusiva (L2 no contiene líneas que estén en L1d). Write-back.
* **L3:** 32 MB, 16-way, **victim cache**: las líneas expulsadas de L2 van a L3. **46 cycles** mínimo (medidos con `clflush` + `rdtsc` en tests de C&C; cifra útil para kernel timing).

> **Implicación crítica para el kernel:** la latencia de L3 es **el doble** que en Zen 2 (39 → 46). Cargas que cruzan CCD son aún más caras (Infinity Fabric over GMI). Alinea las estructuras de datos críticos en **64 bytes** y prefiere mantenerlas en L1/L2 por core (thread-local) en lugar de compartidas.

---

## 4. CPUID: leaves importantes

Tabla compacta de las hojas que un kernel bare-metal para el 5600X debe implementar:

| Leaf | Sub | EAX in | Significado | Valores típicos 5600X |
|---|---|---|---|---|
| 0x0 | 0 | — | max basic + vendor | EAX=0x1, EBX/EDX/ECX="AuthenticAMD" |
| 0x1 | 0 | — | family/model/stepping + features | ver §4.1 |
| 0x6 | 0 | — | thermal/power mgmt | varies |
| 0x7 | 0 | — | extended features (EFB, AVX, BMI...) | ver §4.2 |
| 0x7 | 1 | — | more extended features | ver §4.3 |
| 0xD | 0 | — | XSAVE state-supported bitmap | depends on XCR0 |
| 0xD | 1 | — | XSAVE state-required + sizes | |
| 0x80000000 | 0 | — | max extended | 0x80000020 |
| 0x80000001 | 0 | — | ext features (NX, RDTSCP, ...) | ver §4.4 |
| 0x80000002..4 | 0 | — | brand string | "AMD Ryzen 5 5600X..." |
| 0x80000005 | 0 | — | L1 cache + TLB info | ver §4.5 |
| 0x80000006 | 0 | — | L2/L3 cache + L2 TLB | ver §4.6 |
| 0x80000007 | 0 | — | Invariant TSC, etc. | ver §4.7 |
| 0x80000008 | 0 | — | address sizes | ver §4.8 |
| 0x8000000A | 0 | — | SVM features | ver §4.9 |
| 0x8000001D | i | — | cache topology (deterministic) | ver §4.10 |
| 0x8000001E | 0 | — | APIC ID + topology | ver §4.11 |

### 4.1 `CPUID.1` — features

`CPUID(EAX=1)` devuelve `EAX, EBX, ECX, EDX`.

* `EAX` = family/model/stepping.
* `EBX[7:0]` = Brand index (reservado / 0).
* `EBX[15:8]` = CLFLUSH line size en unidades de 8 bytes (×8 = 64 → valor 0x8).
* `EBX[23:16]` = logical processor count (número de LP en este package) = 12.
* `EBX[31:24]` = initial APIC ID.
* `ECX` = feature bits.
* `EDX` = feature bits.

**ECX bits relevantes (1 = presente):**

| Bit | Mnemonic | Significado |
|---|---|---|
| 0 | SSE3 | siempre 1 |
| 1 | PCLMULQDQ | carry-less multiply |
| 9 | SSSE3 | |
| 19 | SSE4.1 | |
| 20 | SSE4.2 | |
| 22 | MOVBE | |
| 23 | POPCNT | |
| 25 | AES-NI | |
| 26 | XSAVE | |
| 27 | OSXSAVE | **debe estar a 1 antes de `XSETBV`** |
| 28 | AVX | |
| 29 | F16C | half-precision FP |
| 30 | RDRAND | |
| 31 | **hypervisor** | siempre 0 en hardware físico |

**EDX bits relevantes:**

| Bit | Mnemonic | Significado |
|---|---|---|
| 4 | TSC | RDTSC |
| 5 | MSR | RDMSR/WRMSR |
| 6 | PAE | Physical Address Extension |
| 7 | MCE | Machine Check Exception |
| 8 | CX8 | CMPXCHG8B |
| 9 | APIC | Local APIC presente |
| 12 | MTRR | |
| 13 | PGE | Page Global Enable (CR4.PGE) |
| 15 | CMOV | |
| 19 | CLFSH | CLFLUSH |
| 23 | MMX | |
| 24 | FXSR | FXSAVE/FXRSTOR |
| 25 | SSE | |
| 26 | SSE2 | |
| 28 | HTT | indica que EBX[23:16] es válido |

### 4.2 `CPUID.7.0` — extended features (I)

`CPUID(EAX=7, ECX=0)` → `EAX` = max sub-leaf (= 1 en Zen 3), `EBX, ECX, EDX` = features.

**EBX:**

| Bit | Mnemonic | Zen 3 |
|---|---|---|
| 0 | FSGSBASE | ✅ |
| 3 | BMI1 | ✅ |
| 4 | HLE | ❌ |
| 5 | AVX2 | ✅ |
| 7 | SMEP | ✅ |
| 8 | BMI2 | ✅ |
| 9 | ERMS (Enhanced REP MOVSB/STOSB) | ✅ |
| 10 | INVPCID | ✅ |
| 11 | RTM | ❌ |
| 12 | PQM | ❌ |
| 13 | **PQE** (Platform QoS Enforcement) | ❌ |
| 14 | AVX-512 F | ❌ |
| 15 | RDPKU | ❌ |
| 18 | **RDSEED** | ✅ |
| 19 | ADX | ❌ |
| 20 | SMAP | ✅ |
| 22 | CLFLUSHOPT | ✅ |
| 23 | CLWB | ❌ (no en Zen 3) |
| 24 | **Intel Processor Trace** | ❌ |
| 25 | AVX-512 PF | ❌ |
| 26 | AVX-512 ER | ❌ |
| 27 | AVX-512 CD | ❌ |
| 28 | SHA-NI | ✅ |
| 29 | AVX-512 BW | ❌ |
| 30 | AVX-512 VL | ❌ |

**ECX:**

| Bit | Mnemonic | Zen 3 |
|---|---|---|
| 0 | PREFETCHWT1 | ❌ |
| 4 | **LA57** (5-level paging) | **❌ NO soportado en 5600X** |
| 5 | RDPID | ✅ |
| 22 | HRESET | ❌ |
| 25 | CET_IBT | ❌ |
| 28 | AVX-VNNI | ❌ |
| 31 | **XFD** | ❌ |

**EDX:**

| Bit | Mnemonic | Zen 3 |
|---|---|---|
| 17 | **PCONFIG** | ❌ |
| 18 | IBT (CET shadow stack) | ❌ |
| 20 | CET_SS | ❌ (algunas BIOS reportan parcial) |
| 22 | **HRESET** | ❌ |

### 4.3 `CPUID.7.1` — extended features (II)

`CPUID(EAX=7, ECX=1)` → `EAX, EBX, ECX, EDX` con EAX = max sub-leaf output.

Para Zen 3 todos los bits típicos son 0. No hay AVX-512, no hay DLB, no hay XFD.

### 4.4 `CPUID.0x80000001` — ext features

* `ECX`:
  * Bit 0 = LAHF (Long mode AH from flags) ✅.
  * Bit 1 = CMP_LEGACY (HyperThreading no es válido) ❌.
  * Bit 2 = SVM ✅ (AMD-V).
  * Bit 5 = ABM (LZCNT/POPCNT en modo 32-bit) ✅.
  * Bit 6 = SSE4A ✅.
  * Bit 7 = MISALIGNSSE ✅.
  * Bit 8 = 3DNOW! PREFETCH ✅ (PREFETCHW).
  * Bit 9 = OSVW (OS Visible Workaround) ✅.
  * Bit 10 = 3DNOW! (instrucciones legacy, no en 64-bit).
  * Bit 11 = XOP ❌.
  * Bit 12 = SKINIT ✅.
  * Bit 13 = WDT ❌.
  * Bit 15 = LWP ❌.
  * Bit 16 = FMA4 ❌.
  * Bit 19 = NodeId MSR ❌.
  * Bit 21 = TBM ❌.
  * Bit 22 = TopologyExtensions (CPUID.0x8000001E) ✅.

* `EDX`:
  * Bit 11 = SYSCALL/SYSRET ✅ (no `SYSENTER`/`SYSEXIT` en long mode; usar SYSCALL).
  * Bit 20 = NX (no-execute bit) ✅.
  * Bit 22 = MMXEXT ✅.
  * Bit 23 = RDTSCP ✅.
  * Bit 24 = _1GB_PAGE ✅ (soporte de páginas de 1 GiB en page tables).
  * Bit 25 = **TSCE** (RDTSCP instruction) ✅.
  * Bit 26 = **LONG** (64-bit mode) ✅.
  * Bit 27 = **3DNOW!** (legacy).
  * Bit 28 = 3DNOWEXT ✅.
  * Bit 29 = **LM** (long mode) ✅.
  * Bit 30 = **3DNow!** (legacy).

### 4.5 `CPUID.0x80000005` — L1 + TLB

* `EAX[31:24]` = 2MB L1 TLB I entries + associativity (0xFF = 64 entries, full assoc).
* `EAX[23:16]` = 2MB L1 TLB I associativity.
* `EAX[15:8]` = 4KB L1 TLB I entries + assoc.
* `EAX[7:0]` = 4KB L1 TLB I associativity.
* `EBX` = mismo formato para D-TLB.
* `ECX` = L1d: size (KB) | assoc | cachelines/tag | cacheline size.
* `EDX` = L1i: size (KB) | assoc | cachelines/tag | cacheline size.

Valores típicos 5600X:
* L1i: 32 KB, 8-way, 64-byte lines.
* L1d: 32 KB, 8-way, 64-byte lines.
* L1 I-TLB 4K: 64 entries.
* L1 D-TLB 4K: 64 entries.
* L1 I-TLB 2M: 64 entries (algunos stepping menos).
* L1 D-TLB 2M: 64 entries.

### 4.6 `CPUID.0x80000006` — L2/L3 + L2 TLB

* `EAX[31:16]` = L2 TLB 2M/4M: entries | associativity.
* `EAX[15:0]` = L2 TLB 4K: entries | associativity.
* `EBX[31:16]` = L2 cache: lines-per-tag | assoc.
* `EBX[15:0]` = L2 cache size in KB.
* `ECX` = L3 cache: lines-per-tag | assoc.
* `EDX` = L3 cache: size in 512KB units.

Valores típicos:
* L2: 512 KB, 8-way, 64-byte lines.
* L3: 32 MB (= 64 en unidades de 512 KB), 16-way.

### 4.7 `CPUID.0x80000007` — power / TSC

* `EBX` = reserved (legacy 3DNow! P-State info, ya no aplica).
* `EDX`:
  * Bit 0 = TS (Temperature Sensor) ✅.
  * Bit 1 = FID (Frequency ID control) ❌ (legado).
  * Bit 2 = VID (Voltage ID control) ❌.
  * Bit 3 = TTP (Thermal Trip) ✅.
  * Bit 4 = 100 MHz steps ❌.
  * Bit 5 = HW PState ✅.
  * Bit 6 = TSC invariant ❌ (en Zen 3; ver §12).
  * Bit 7 = CORE PSTATE registers ✅.
  * Bit 8 = **TscInvariant** (CPUID.8000_0007H:EDX[8]) — **reportado a 1** en Linux como "constant_tsc", pero **no es constant** en sentido Intel. Ver §12 para detalles.
  * Bit 9 = CPB (Core Performance Boost) ✅.
  * Bit 10 = ReadOnly RAPL ❌.
  * Bit 11 = "fast" deprecación de PState: ✅.

> ⚠️ **Advertencia crítica:** el bit 8 (TscInvariant) **sí está a 1 en Zen 3**, lo que lleva al kernel de Linux a marcar `constant_tsc`. Pero el TSC en Zen 3 **varía con el P-state**, NO es invariante como en Intel. El OS debe igualmente calibrarlo. Detalles en §12.

### 4.8 `CPUID.0x80000008` — address sizes

* `EAX[7:0]` = physical address bits = **40** (1 TiB) en el 5600X. **NO** 48.
* `EAX[15:8]` = linear (virtual) address bits = **48** (256 TiB canónicos).
* `EAX[23:16]` = guest physical address bits (con SEV/SVM) — reservado en el 5600X (no SEV en desktop).
* `EBX[15:0]` = `CLZERO`, IRPerf, etc. en Zen:
  * Bit 0 = CLZERO ✅ (zero 64B cacheline).
  * Bit 1 = IRPerf ✅.
  * Bit 2 = XSAVEERPTR ❌.
  * Bit 4 = RDPRU ✅.
  * Bit 5 = NMI on L1DF ✅ (legacy).
  * Bit 8 = WBNOINVD ❌ (WBNOINVD llega en Zen 4).
  * Bit 9 = IBPB ✅.
  * Bit 10 = WBINVD/WBNOINVD inter-proc ❌.
  * Bit 11 = RRSBA_CTRL ❌ (Zen 4+).
  * Bit 12 = INVLPGB ✅ **NUEVO en Zen 3**: bulk TLB invalidation.
  * Bit 13 = RDPID → no en este leaf; está en CPUID.7.0:ECX[22].
  * Bit 15 = **IBRS_SAME_MODE** ❌.
  * Bit 16 = **EFRO** ❌ (no supported in Zen 3).
  * Bit 17 = PSFD ✅ (Predictive Store Forwarding Disable).
  * Bit 18 = BTC_NO ✅ (no afectado por Spectre-BTB-cross).
  * Bit 19 = **BTC_OSDSZ** ❌.
  * Bit 20 = **FSRCS** ❌.
  * Bit 22 = **EPSF** ❌.
  * Bit 23 = **PF_LIMIT_LP** ❌.
  * Bit 25 = **SBPB** (Single Threaded Indirect Branch Predictor Barrier) ✅.

* `ECX` = **invlpgb count** máximo: indica el máximo número de páginas que `INVLPGB` puede invalidar en una sola instrucción. Típicamente 0x40 (64) en Zen 3. (Suele venir en `ECX` para algunos modelos; verificar con cuidado).

### 4.9 `CPUID.0x8000000A` — SVM features

* `EAX[7:0]` = SVM revision = 0x1.
* `EDX` = SVM features:
  * Bit 0 = NP (Nested Paging) ✅.
  * Bit 1 = LBR Virt ✅.
  * Bit 2 = SVM Lock ✅.
  * Bit 3 = NRIP save ✅.
  * Bit 4 = TSC rate MSR ✅.
  * Bit 5 = VMCB clean bits ✅.
  * Bit 6 = Flush by ASID ✅.
  * Bit 7 = Decode assists ✅.
  * Bit 10 = Pause intercept filter ✅.
  * Bit 12 = Pause filter threshold ✅.
  * Bit 13 = AVIC (Advanced Virtual Interrupt Controller) ✅.
  * Bit 14 = VIRT_SPEC_CTRL (virtualize SSBD) ✅.
  * Bit 15 = VIRT_SSBD ✅.

### 4.10 `CPUID.0x8000001D` — cache topology

`CPUID(EAX=0x8000001D, ECX=i)` describe los niveles de caché como una jerarquía:

| i | Tipo | 5600X |
|---|---|---|
| 0 | L1d | 32 KB, 8-way, 1 thread sharing = 1 |
| 1 | L1i | 32 KB, 8-way, 1 thread sharing = 1 |
| 2 | L2 | 512 KB, 8-way, 1 thread sharing = 1 (per-core) |
| 3 | L3 | 32 MB, 16-way, 6 threads sharing = 12 (compartido entre los 6 cores activos del CCX) |

* `EAX[4:0]` = tipo (1=data, 2=instr, 3=unified, 4..reserved).
* `EAX[7:5]` = nivel.
* `EAX[8]` = self-initialized.
* `EAX[9]` = fully associative.
* `EAX[25:14]` = num_sets - 1.
* `EAX[31:26]` = assoc - 1 (si fully assoc, vale 0xFF).
* `EBX[15:0]` = line_size - 1.
* `EBX[31:22]` = lines_per_tag - 1.
* `EDX[9:0]` = num_threads_sharing - 1.
* `EDX[25:14]` = num_cores_in_cache - 1 (0 si no se conoce).

### 4.11 `CPUID.0x8000001E` — extended APIC + topology

* `EAX[7:0]` = **ExtApicId** (id APIC extendido del core actual).
* `EAX[31:8]` = reserved.
* `EBX[7:0]` = **CoreId** (id del core físico dentro del CCX, 0-based). Para el 5600X: 0..5.
* `EBX[15:8]` = reserved.
* `ECX[7:0]` = **ThreadsPerComputeUnit** codificado como n-1 → siempre 1 (significa 2 threads).
* `ECX[15:8]` = reserved.

> En Zen 3: bits [7:0] del ExtApicId = (SMTid) + (CoreId << 1) + (CCXid << 4) (para un solo CCX como el 5600X, CCXid = 0).

```rust
// Extracción típica para 5600X
let ext_apic = (eax & 0xFF) as u8;
let core_id  = (ebx & 0xFF) as u8;            // 0..5
let threads_per_core = ((ecx & 0xFF) + 1) as u8; // 2
let smt_id = ext_apic & 1;
```

---

## 5. Ordenamiento de memoria (TSO débil)

**Dato crítico:** el orden de memoria x86 de AMD es **TSO (Total Store Order)**, ligeramente más débil que el TSO "fuerte" de Intel. Las diferencias prácticas que importan a un kernel:

1. **Store → Load:** AMD permite que un load posterior se reordene con un store previo a una dirección diferente. Intel también lo permite, pero AMD es más agresivo en el reordering.
2. **Store → Store:** NO se reordena.
3. **Load → Load:** NO se reordena.
4. **Load → Store:** NO se reordena.
5. **Dependiente de Load → Load:** NO se reordena (a diferencia de Alpha).
6. **Atomic operations** (LOCK prefix, XADD, CMPXCHG): actúan como **full fence**.

Implicación: en el 5600X, si un kernel hace:

```c
store(&data, 42);          // store
load(&flag);               // load — puede ejecutar ANTES del store
```

El `load(&flag)` puede ejecutarse antes que `store(&data)` si la predictor de memoria determina que no hay dependencia. Esto es un caso real de data race en código de kernels mal escritos.

### 5.1 Fences

* `MFENCE` → full fence (load+store, espera por todos los loads y stores en vuelo).
* `LFENCE` → load fence. Carga todos los loads pendientes. **Adicionalmente serializa el flujo de instrucciones** (en Intel no, en AMD sí, ver APM Vol 2).
* `SFENCE` → store fence. Asegura que stores previos sean visibles antes que stores posteriores.

Codificación:

| Mnemónico | Opcode | Notas |
|---|---|---|
| LFENCE | `0F AE E8` | serializing; útil para mitigar Spectre v1 |
| SFENCE | `0F AE F8` | |
| MFENCE | `0F AE F0` | implícito si hay LOCK prefix |

### 5.2 LOCK prefix

Cualquier instrucción con LOCK prefix (`LOCK ADD`, `LOCK XCHG`, `LOCK CMPXCHG`, `LOCK INC`, etc.) se ejecuta **atómicamente** con respecto a otros cores, e implícitamente es un full fence. Imprescindible para spinlocks, mutex, etc.

```asm
lock add dword [rdi], 1     ; atómico
lock cmpxchg [rsi], rdx    ; CAS atómico
```

### 5.3 Tabla rápida para kernel writers

| Operación | Necesita fence? |
|---|---|
| Store + Load (mismo addr) | NO (single-copy) |
| Store + Load (distinto addr) | **SÍ** o usar LOCK |
| Store + Store (distinto addr) | NO (TSO) |
| Load + Load | NO |
| Atomic increment | LOCK implícito |
| Publicar datos + publicar flag | SÍ (`SFENCE` o `LOCK` store) |
| Adquirir lock (spin) | `LOCK`/`XCHG` (full fence) |
| Liberar lock | `LOCK` (full fence) o `SFENCE` antes del store del flag |

### 5.4 Cacheline y aliasing

* **Cacheline = 64 bytes**. Las estructuras compartidas entre cores deben estar **alineadas a 64 B** para evitar false sharing.
* El kernel de Linux usa `cacheline_aligned` / `__cacheline_aligned_in_smp`. En Rust: `#[repr(align(64))]`.
* Split-lock (atomic op que cruza cacheline) puede generar #AC; deshabilitar con `CR4.SMAP` no aplica, pero sí evitar.

---

## 6. Cache, TLB y coherencia

### 6.1 Resumen de jerarquía

```
+------------------------+  +--------+
|   L1i 32 KB / core     |  |        |
|   L1d 32 KB / core     |  | per    |
|   L2  512 KB / core    |  | core   |
+------------------------+  +--------+
        | 32 MB L3 (16-way, victim)
        v
+------------------------+
|  CCX = 6 cores activos |
|  (CCD Vermeer B0)      |
+------------------------+
        | Infinity Fabric (GMI)
        v
+------------------------+
|   I/O Die: DDR4, PCIe  |
+------------------------+
```

### 6.2 Coherencia

El protocolo es MOESI, mantenido por **infinity fabric** dentro del CCD. Para cross-CCD (en chips con 2 CCDs, no es el caso del 5600X), la coherencia pasa por el I/O die y es **visible latency** (50-100+ ns).

Dentro del CCX, las transferencias L3↔L2 usan **probe + snoop**, no se hace fetch en algunos casos. La latencia efectiva L3 hit → 46 ciclos (medidos en clocks a frecuencia base de 3.7 GHz = ~12.4 ns; a boost 4.6 GHz = ~10 ns).

### 6.3 TLB y shootdown

* L1 I-TLB 4K: 64 entries, full assoc.
* L1 D-TLB 4K: 64 entries, full assoc.
* L1 I-TLB 2M: 64 entries.
* L1 D-TLB 2M: 64 entries.
* L1 I-TLB 1G: 64 entries (también, pero para 1 GiB pages).
* **L2 TLB:** 2048 entries, 8-way, cubre 4K/2M/1G.

### 6.4 INVLPGB (Zen 3 nuevo)

Zen 3 introduce **INVLPGB** (Bulk TLB invalidation). CPUID.0x80000008.ECX indica el count máximo; en Vermeer es típicamente 0x40 (64 páginas) por invocación. Permite invalidar hasta 64 PCIDs × 64 páginas en una sola instrucción.

Sintaxis:

```asm
invlpgb [rsi]  ; rsi → array de {PCID, virtual_address, ASID}
```

EFER.INVLPGB = bit 21 habilita esta instrucción.

Es un **acelerador importante** para TLB shootdown masivo. Linux 5.18+ lo usa. Para FastOS, escribir un mini-tlb-shootdown que use INVLPGB cuando se cambia una tabla de páginas con 64+ entradas.

### 6.5 WBINVD / INVD

* `INVD` (Invalidate, no write-back): **no soportada en modo long**. #UD.
* `WBINVD` (Write-back + invalidate): sí soportada. Útil en algunos contextos (cambio de CR3 global, setup de MTRR).
* `WBNOINVD` (Write-back, no invalidate): NO en Zen 3 (llega en Zen 4). Bit CPUID.0x80000008.EBX[8] = 0 en Vermeer.

---

## 7. Paging y memoria virtual

### 7.1 Tabla de páginas

El 5600X usa **4-level page tables** estándar de AMD64:

```
Virtual Address (48 bits canónicos):
  [47:39] = PML4 index
  [38:30] = PDPT  index
  [29:21] = PD    index
  [20:12] = PT    index
  [11:0]  = page offset
```

* PML4 → PDPT → PD → PT (4 KB pages)
* PML4 → PDPT → PD (2 MB huge)
* PML4 → PDPT (1 GB huge)

**5-level paging (LA57)** NO está soportado en el 5600X (`CPUID.7.0:ECX[LA57] = 0`).

### 7.2 Formato de las PTE

64 bits por entrada. Bits (little-endian, no exclusivo):

| Bit | Nombre | Significado |
|---|---|---|
| 0 | P (Present) | 0 = entrada libre |
| 1 | R/W | 0 = read-only, 1 = writable |
| 2 | U/S | 0 = supervisor, 1 = user |
| 3 | PWT | Page Write-Through (override MTRR) |
| 4 | PCD | Page Cache Disable |
| 5 | A | Accessed (set por CPU; limpiar software) |
| 6 | D | Dirty (solo leaf entries) |
| 7 | PAT/PS | Page Attribute Table / Page Size |
| 8 | G | Global (no flush en CR3 write) — requiere CR4.PGE |
| 9-11 | (avail) | software use |
| 12-51 | PFN | physical frame number (40 bits en 5600X) |
| 52-58 | (avail/avail1/prot) | protection key / etc. (no en 5600X) |
| 59-62 | (avail2-3) | |
| 63 | NX | No-Execute (EFER.NXE debe estar activo) |

> **Implicación kernel:** limpiar el bit A cada vez que se hace page-walk (TLB shootdown-aware) o usar el bit P (P-bit-aging). Si el bit A se llena, se tiene que periódicamente escanear y limpiarlo, pero **NUNCA** perder D (Dirty): si se pierde, se podría skip un page write-back.

### 7.3 Bits específicos de AMD en PTE

Zen 3 también soporta (en algunas regiones):

* **NX** (bit 63): se activa con `EFER.NXE` (MSR 0xC0000080, bit 11).
* **Page Protection Key** (bits 59-62): introducido en AMD64, pero solo en CPUs de servidor (Epyc Milan+); no en el 5600X. **Ignorar para el 5600X.**

### 7.4 PCID (Process-Context ID)

Activado con `CR4.PCIDE = 1`. PCID = 12 bits en el 5600X (CPUID 0x80000008 no lo declara; viene implícito por la spec AMD64 v1).

* `CR3[11:0]` = PCID.
* `MOV CR3, rax` NO flushea TLB si el PCID no cambia.
* `MOV CR3, rax` con `OR 0x1000` en `rax` flushea **solo entradas con el PCID actual**.

Útil en kernel para reducir TLB miss rate al cambiar entre procesos.

### 7.5 INVLPGB (Zen 3 nuevo, ver §6.4)

Disponibilidad: `CPUID.0x80000008.EBX[12] = 1`. Invalida múltiples PCIDs y/o direcciones en una sola llamada.

### 7.6 ASID

ASID = PCID en AMD64 (mismo concepto). 12 bits = 4096 contextos únicos.

### 7.7 INVLPG

`INVLPG [m]` invalida una entrada del TLB. El modo long lo soporta; dirección virtual.

### 7.8 Acceso de usuario al ring 0: SMEP, SMAP, UMIP

* **SMEP (Supervisor Mode Execution Prevention):** si CR4.SMEP=1, cuando el CPL<2 intenta ejecutar una página con U/S=1, dispara #PF. Activable via `CPUID.7.0:EBX[7]`.
* **SMAP (Supervisor Mode Access Prevention):** si CR4.SMAP=1, accesos supervisor a páginas user son bloqueados (por defecto). STAC/CLAC togglean dentro de regiones controladas. Bit 20 en CPUID.7.0:EBX.
* **UMIP (User-Mode Instruction Prevention):** protege SGDT, SIDT, SLDT, STR, SMSW. Bit 22 en CPUID.7.0:ECX.

El 5600X soporta los tres. Usar SMAP+SMEP+UMIP+LA57_disabled desde el primer init del kernel.

---

## 8. Excepciones e IDT

### 8.1 IDT — Interrupt Descriptor Table

* 256 entries × 16 bytes.
* Formato (x86-64 interrupt/trap gate):

```
Bytes:
  0-1   = offset_low
  2-3   = selector (segment CS)
  4     = ist (bits [2:0] del byte)
  5     = type_attr
        - bit 0: gate type (0=task, 1=intr/trap; 0xE=interrupt 64-bit, 0xF=trap 64-bit)
        - bit 1: always 0 en long mode
        - bit 2: 0=interrupt gate, 1=trap gate
        - bit 3: 0
        - bits 4-7: DPL (0, 1, 2, 3; 0xF=reserved)
        - bit 7: Present
  6-7   = offset_mid
  8-11  = offset_high (en long mode 64-bit, los 32 bits altos)
  12    = reserved
  13    = reserved
  14    = reserved
  15    = reserved
```

Total = 16 bytes. **RIDT base + index × 16**.

### 8.2 Hardware exceptions (0-31)

| # | Vector | Name | Error code? | IST recomendado |
|---|---|---|---|---|
| 0 | #DE | Divide Error | no | — |
| 1 | #DB | Debug | no | 1 (debug) |
| 2 | — | NMI | no | 2 (NMI) |
| 3 | #BP | Breakpoint (INT 3) | no | — |
| 4 | #OF | Overflow (INTO) | no | — |
| 5 | #BR | Bound Range Exceeded (BOUND) | no | — |
| 6 | #UD | Invalid Opcode | no | — |
| 7 | #NM | Device Not Available (FPU) | no | — |
| 8 | #DF | Double Fault | **always 0** | **1 (must)** |
| 9 | — | Coprocessor Segment Overrun | no | — |
| 10 | #TS | Invalid TSS | sí | — |
| 11 | #NP | Segment Not Present | sí | — |
| 12 | #SS | Stack-Segment Fault | sí | — |
| 13 | #GP | General Protection | sí | — |
| 14 | #PF | Page Fault | sí (CR2) | — |
| 15 | — | reserved | — | — |
| 16 | #MF | x87 FP Exception | no | — |
| 17 | #AC | Alignment Check | sí (0) | — |
| 18 | #MC | Machine Check | no | **3 (MCE)** |
| 19 | #XM | SIMD FP Exception | no | — |
| 20 | #VE | Virtualization Exception (VT-x/AMD-V) | no | — |
| 21 | #CP | Control Protection (CET) | sí | — |
| 22-31 | — | reserved | — | — |

### 8.3 IST (Interrupt Stack Table)

8 entradas en el TSS (64 bits cada una: 8 × 8 = 64 bytes en la zona del TSS). El campo IST en el gate descriptor (bits 0-2) selecciona cuál usar (0 = no IST, 1-7 = IST index).

Recomendación:
* IST1 = #DF (Double Fault; debe tener IST distinto porque #DF puede ocurrir en cualquier contexto, incluido dentro de una interrupción de usuario que toque un stack corrupto).
* IST2 = NMI.
* IST3 = #MC (Machine Check).
* IST4 = #DB (debug).
* IST5 = #VE (si se usa virtualización).

Cada IST tiene un stack dedicado (recomendado 8 KB o 16 KB) y debe ser **siempre válido**; el CPU lo carga *sin consultar CR3*, así que es robusto ante un CR3 corrupto.

### 8.4 Interrupciones de hardware

Para el 5600X, las IRQ legadas (0-15) y luego las IRQ extendidas (16-23) se enrutan vía I/O APIC. El Local APIC del core maneja la entrega final.

### 8.5 Interrupciones SMP

* INIT IPI → resetea core.
* STARTUP IPI (SIPI, vector = dirección / 0x1000) → arranca core en 0x[vector]00:0x[vector]00.
* Ver §9 para detalles.

---

## 9. Local APIC

Cada core del 5600X tiene su propio Local APIC (LAPIC). Registros en MMIO, baseada en `IA32_APIC_BASE` (MSR 0x1B), por defecto `0xFEE00000`.

### 9.1 Layout de registros

Todos los registros son de 32, 64 o 128 bits, alineados a 16 bytes. Se accede como MMIO volátil.

| Offset | Size | Nombre | Tipo |
|---|---|---|---|
| 0x000 | 128 | RESERVED | — |
| 0x010 | 64 | Reserved (ID en low 32) | — |
| 0x020 | 32 | **APIC ID** | R/W |
| 0x030 | 32 | **APIC VERSION** | RO |
| 0x040-0x070 | 32 × 4 | Reserved | — |
| 0x080 | 32 | TPR (Task Priority) | R/W |
| 0x090 | 32 | APR (Arbitration Priority) | RO |
| 0x0A0 | 32 | PPR (Processor Priority) | RO |
| 0x0B0 | 32 | **EOI** (End Of Interrupt) | WO |
| 0x0C0 | 32 | RRD (Remote Read) | RO |
| 0x0D0 | 32 | Logical Destination | R/W |
| 0x0E0 | 32 | Destination Format | R/W |
| 0x0F0 | 32 | Spurious Interrupt Vector | R/W |
| 0x100-0x170 | 32 × 8 | ISR (In-Service) | RO |
| 0x180-0x1F0 | 32 × 8 | TMR (Trigger Mode) | RO |
| 0x200-0x270 | 32 × 8 | IRR (Interrupt Request) | RO |
| 0x280 | 32 | Error Status | RO |
| 0x290 | 32 | reserved | — |
| 0x2A0-0x2E0 | 32 × 4 | reserved | — |
| 0x2F0 | 32 | LVT CMCI | R/W |
| 0x300 | 32 | ICR Low (Command) | R/W |
| 0x310 | 32 | ICR High (Destination) | R/W |
| 0x320 | 32 | LVT Timer | R/W |
| 0x330 | 32 | LVT Thermal Sensor | R/W |
| 0x340 | 32 | LVT Performance Counter | R/W |
| 0x350 | 32 | LVT LINT0 | R/W |
| 0x360 | 32 | LVT LINT1 | R/W |
| 0x370 | 32 | LVT Error | R/W |
| 0x380 | 32 | Timer Initial Count | R/W |
| 0x390 | 32 | Timer Current Count | RO |
| 0x3A0-0x3D0 | 32 × 4 | reserved | — |
| 0x3E0 | 32 | Timer Divide Configuration | R/W |
| 0x3F0 | 32 | reserved | — |
| 0x400-0xF00 | varies | extended APIC, x2APIC | si se activa |

### 9.2 Bit fields críticos

**APIC ID (0x020):**
* Bits [3:0] (legacy xAPIC) = ID de 4 bits (0..15).
* Bits [7:0] o [31:0] en x2APIC = ID de 8 o 32 bits.

**Spurious Interrupt Vector (0x0F0):**
* Bits [7:0] = vector (debe ser alineado a 16, ej. 0xFF).
* Bit 8 = APIC Software Enable (1 = APIC activo, 0 = deshabilitado en core; importante en init).
* Bit 9 = Focus Processor Checking.
* Bit 12 = EOI broadcast suppression (Zen 3 lo soporta).

**EOI (0x0B0):**
* Solo se escribe (no se lee). Escribir **cualquier valor** completa el ISR del vector con mayor prioridad pendiente.

**ICR Low (0x300):**
* Bits [7:0] = vector.
* Bits [10:8] = Delivery Mode:
  * 000 = Fixed (entrega a vector).
  * 001 = Lowest Priority (legacy).
  * 010 = SMI.
  * 011 = **reserved**.
  * 100 = NMI (vector ignorado).
  * 101 = INIT.
  * 110 = **Start Up** (SIPI, vector = dirección de bootstrap / 16).
  * 111 = reserved.
* Bit 11 = Destination Mode (0 = Physical, 1 = Logical).
* Bit 12 = Delivery Status (0 = idle, 1 = pending).
* Bit 14 = Level (Assert/Deassert).
* Bit 15 = Trigger Mode (0 = edge, 1 = level).

**ICR High (0x310):**
* Bits [27:24] = Destination field (en physical mode).

**LVT Timer (0x320):**
* Bits [7:0] = vector.
* Bits [10:8] = Mask, periodic, TSC-deadline:
  * 000 = one-shot (legacy, con timer)
  * 001 = periodic
  * 010 = TSC-deadline (one-shot usando IA32_TSC_DEADLINE MSR)
  * 100 = reserved
* Bits [17:16] = Timer mode: TSC-deadline, etc.

**Timer Divide Configuration (0x3E0):**
* Bits [1:0] = divisor: 1, 2, 4, 8, 16, 32, 64, 128.
* Bits [3] = divide value (1 = divide by divisor).

### 9.3 TSC-deadline mode

Zen 3 soporta TSC-deadline timer (CPUID.1:ECX[24] = 1). Es el modo recomendado para schedulers modernos.

```c
// Programar un timer one-shot en N ticks de TSC
wrmsr(IA32_TSC_DEADLINE, current_tsc + delta);
// El APIC dispara el vector configurado en LVT Timer cuando TSC = deadline
```

### 9.4 INIT-SIPI-SIPI sequence (SMP boot)

Para arrancar un Application Processor (AP):

1. BSP (Bootstrap Processor) escribe **APIC ID** del target en ICR High (0x310).
2. BSP envía **INIT IPI** (Delivery Mode = 101, Level = Assert, Trigger = Edge).
3. Esperar ~10 ms.
4. BSP envía **De-assert INIT IPI** (Level = Deassert).
5. Esperar ~10 ms.
6. BSP envía **STARTUP IPI** (Delivery Mode = 110, vector = 0x08 → dirección 0x8000).
7. Esperar ~200 µs.
8. Repetir SIPI (otro STARTUP IPI) por si el primero se perdió.
9. El AP ejecuta desde `0x0000:0x8000` (modo real al inicio, luego long mode vía jump).

El AP espera eventos INIT en `INIT_WAIT_STATE`. Recien después de la primera SIPI comienza a ejecutar.

### 9.5 x2APIC

Activable con `IA32_APIC_BASE[10] = 1`. Provee IDs de 32 bits y acceso MSR-based. **Zen 3 no soporta x2APIC** (o tiene soporte limitado en algunos stepping) — verificar con `CPUID.1:ECX[21]`. Si bit=0, mantener xAPIC (MMIO) y limit ID a 8 bits.

---

## 10. MSRs fundamentales

`RDMSR` y `WRMSR` son las instrucciones para acceder Model-Specific Registers. ECX = MSR number, EDX:EAX = value (en 64-bit, RAX[63:32] = 0 usualmente).

### 10.1 Tabla compacta de MSRs

| MSR | Nombre | Bits importantes | Notas |
|---|---|---|---|
| 0x001 | IA32_PPERF | — | Current Performance (0..255) |
| 0x010 | **IA32_TSC** | — | Time Stamp Counter (64-bit low) |
| 0x017 | IA32_PLATFORM_ID | — | legacy |
| 0x01B | **IA32_APIC_BASE** | ver §10.2 | BSP + APIC global enable |
| 0x035 | IA32_CORE_CAPABILITIES | — | Zen 4+ |
| 0x08B | IA32_PATCH_LEVEL | — | microcode revision ID |
| 0x0FE | IA32_MTRRCAP | ver §10.3 | number of variable MTRRs |
| 0x174-0x176 | IA32_SYSENTER_CS/ESP/EIP | — | legacy sysenter (no usar en long mode) |
| 0x175 | IA32_SYSENTER_ESP | — | |
| 0x176 | IA32_SYSENTER_EIP | — | |
| 0x179 | IA32_MCG_CAP | — | machine check |
| 0x17A | IA32_MCG_STATUS | — | |
| 0x1D9 | IA32_DEBUGCTL | — | branch trace store, etc. |
| 0x1F8 | IA32_PLATFORM_DCA_CAP | — | legacy |
| 0x1FC | IA32_CPUID_FEAT | — | AMD-specific |
| 0x200-0x20F | IA32_MTRR_PHYSBASE0..7 | — | base + mask pairs |
| 0x201-0x217 | IA32_MTRR_PHYSMASK0..7 | — | (skipping 0x218 = fixed range) |
| 0x250 | IA32_MTRR_FIX64K_00000 | — | fixed MTRR |
| 0x258-0x26F | IA32_MTRR_FIX16K_80000.. | — | fixed MTRR 16K |
| 0x277 | **IA32_PAT** | — | Page Attribute Table |
| 0x2FF | IA32_MTRR_DEF_TYPE | ver §10.4 | default MTRR type |
| 0x309 | IA32_FIXED_CTR0 | — | fixed performance counter 0 (instr retired) |
| 0x30A | IA32_FIXED_CTR1 | — | core cycles |
| 0x30B | IA32_FIXED_CTR2 | — | ref cycles |
| 0x345 | IA32_PERF_CAPABILITIES | — | LBR format |
| 0x38D | IA32_FIXED_CTR_CTRL | — | |
| 0x38E | IA32_PERF_GLOBAL_STATUS | — | |
| 0x38F | IA32_PERF_GLOBAL_CTRL | — | |
| 0x390 | IA32_PERF_GLOBAL_OVF_CTRL | — | |
| 0x3F1 | IA32_PEBS_ENABLE | — | |
| 0x400-0x403 | IA32_MC0_CTL etc. | — | machine check banks |
| 0x570 | IA32_RTIT_CTL | — | (no PT en Zen 3) |
| 0x606 | IA32_TSC_DEADLINE | — | TSC-deadline timer target |
| 0x6E0 | IA32_TSC_DEADLINE (alt name) | — | = 0x606 |
| 0x770 | IA32_PM_ENABLE | — | HWP |
| 0xC0000080 | **IA32_EFER** | ver §10.5 | Extended Feature Enable |
| 0xC0000081 | **IA32_STAR** | — | SYSCALL target (legacy/compat mode) |
| 0xC0000082 | **IA32_LSTAR** | — | SYSCALL target (long mode 64-bit) |
| 0xC0000083 | IA32_CSTAR | — | (compat mode SYSCALL, 32-bit) |
| 0xC0000084 | **IA32_FMASK** | — | RFLAGS mask for SYSCALL |
| 0xC0000100 | IA32_FS_BASE | — | thread-local FS base |
| 0xC0000101 | **IA32_GS_BASE** | — | thread-local GS base (kernel) |
| 0xC0000102 | **IA32_KERNEL_GS_BASE** | — | GS swap |
| 0xC0000103 | **IA32_TSC_AUX** | — | valor para RDTSCP (TSC_AUX) |
| 0xC0000080-0xC00000FF | otros | — | AMD-specific EFER, etc. |
| 0xC0000111 | SMM_BASE | — | SMM base address |
| 0xC0010058 | MMIO Configuration Base Address | — | Family 10h+ |
| 0xC0010058-0xC001005B | — | — | |
| 0xC0010111 | SMM_BASE | — | SMM (alternativo) |
| 0xC0010112 | SMMAddr | — | SMM TSegBase |
| 0xC0010113 | SMMMask | — | |
| 0xC0010114 | VM_CR | — | SVM control |
| 0xC0010115 | IGNNE | — | legacy FPU IGNNE |
| 0xC0010116 | SMM_CTL | — | |
| 0xC0010117 | VM_HSAVE_PA | — | SVM host save area |
| 0xC001011E | SVM_KEY | — | SVM key MSR |
| 0xC0010140-0xC0010141 | OSVW_ID_Length/OSVW_Status | — | OS visible workaround |
| 0xC0010200-0xC0010207 | PState Control 0-7 | — | P-state control (frec/voltage) |
| 0xC0010293 | PStateDef | — | current P-state |
| 0xC00102F0-0xC00102F3 | CStateConfig | — | C-state config |
| 0xC0010300-0xC001030F | CPU_Watchdog Timer | — | |
| 0xC0011000-0xC0011004 | Local APIC ID / etc. | — | alternativa AMD al xAPIC ID |
| 0xC0011022 | LS_CFG | — | load/store configuration |
| 0xC0011030 | L2_PREFETCH_CONFIG | — | L2 streamer prefetch control |
| 0xC0011031 | **SPEC_CTRL** (IBRS) | bit 0=IBRS, bit 1=STIBP, bit 2=SSBD, bit 7=PSFD | Spectre/Meltdown |
| 0xC0011032 | **PRED_CMD** | bit 0 = IBPB (write-only) | indirect branch prediction barrier |
| 0xC0011034 | CStateBaseAddr | — | |
| 0xC0011058 | MMIO Configuration Base Address | — | |
| 0xC0011090-0xC0011094 | TSEG, etc. | — | SMM |
| 0xC00110A0-0xC00110A1 | SMM TSeg Base/Mask | — | |

> Para SPEC_CTRL (Mitigaciones): MSR `0xC0011031` es **específico de AMD** (no confundir con `0x48` de Intel). En Zen 3 soporta:
> * bit 0 = IBRS (Indirect Branch Restricted Speculation).
> * bit 1 = STIBP (Single Thread Indirect Branch Predictors).
> * bit 2 = SSBD (Speculative Store Bypass Disable).
> * bit 7 = PSFD (Predictive Store Forwarding Disable) — nuevo en Zen 3.
> * bit 8 = FB_CLEAR / MFB_CLEAR (Fill Buffer Clear) para mitigación de MDS.

### 10.2 IA32_APIC_BASE (0x1B)

```
Bit  0    : BSP = 1 si es Bootstrap Processor.
Bit  1    : APIC Global Enable (1 = LAPIC activo).
Bit  3    : reserved
Bit  4    : reserved
Bit  7-12 : APIC base address [35:40] (bits altos)
Bit  35-  : APIC base (32-bit page-aligned).
```

Default: 0xFEE00000, BSP=1, Global Enable=1. Para mover el APIC a otra dirección: deshabilitar primero (bit 8 = 0), reescribir, volver a habilitar.

### 10.3 IA32_MTRRCAP (0x0FE)

```
Bits 7:0  = VCNT (número de variable-range MTRRs). En el 5600X = 8 (08h).
Bit  8    = FIX (fixed-range MTRRs soportados). 1 = sí.
Bit  10   = WC (write-combining). 1.
Bit  11   = SMRR. 0 en desktop.
Bit  12   = PRMRR (SGX). 0.
```

### 10.4 IA32_MTRR_DEF_TYPE (0x2FF)

```
Bits 2:0  = Default memory type (UC, WC, WT, WB, WP, UC-).
Bit  10   = Fixed MTRRs enable.
Bit  11   = MTRRs enable.
```

Valores típicos: 0xC00 = MTRR + Fixed habilitados, WB default.

### 10.5 IA32_EFER (0xC0000080)

```
Bit  0    : SCE (SYSCALL/SYSRET enable). **1 = habilita SYSCALL/SYSRET en long mode**.
Bit  1    : reserved
Bit  2    : LME (Long Mode Enable). **1 = habilita long mode (junto con CR4.PAE, CR0.PG)**.
Bit  3    : reserved
Bit  4    : LMA (Long Mode Active). **RO = 1 si el CPU está en long mode**.
Bit  5    : NXE (No-Execute Enable). **1 = habilita el bit NX en PTE**.
Bit  6    : SVME (Secure Virtual Machine Enable). 1 = habilita AMD-V.
Bit  7    : LMSLE (Long Mode Segment Limit Enable). 0.
Bit  8    : FFXSR (Fast FXSAVE/FXRSTOR). 1 = FXSAVE/FXRSTOR son rápidas.
Bit  9    : TCE (Translation Cache Extension). 0.
Bit 10    : MCOMMIT (Cache commit). 0.
Bit 11    : INTWB (Interruptible WBINVD). 0 en Zen 3.
Bit 12    : AIBRSE (Auto IBPB Enable). Zen 3 con BIOS puede tenerla.
Bit 13    : URDMSR/WRMSR. 0.
Bit 14    : UAIE. 0.
Bit 15    : IBRS/IBPB related.
Bit 16    : INVLPGB. 0 (no; esta está en CPUID).
Bit 21    : INVLPGB_enable. 0.
Bit 22    : TSS. 0.
```

**Para habilitar Long Mode + SYSCALL/NX:**
```asm
; 1) PAE first
mov rax, cr4
or  rax, (1 << 5)   ; CR4.PAE
mov cr4, rax

; 2) Build initial PML4 -> PDPT -> PD -> PT identity-mapping 0..1MB
; (No recuerdo aquí; ver §7)

; 3) Set LME
mov rcx, 0xC0000080
rdmsr
or  rax, (1 << 8) | (1 << 11) | (1 << 0)   ; FFXSR, NXE, SCE
wrmsr

; 4) Activate paging + long mode
mov rax, cr0
or  rax, (1 << 31)   ; CR0.PG
mov cr0, rax

; 5) Now in 64-bit long mode
```

### 10.6 IA32_STAR (0xC0000081)

```
Bits 47:32 = CS selector for kernel (R0).
Bits 31:16 = SS selector for kernel (R0).
Bits 15:0  = R3 user CS (low) / R0 base (high).
```

Recomendación típica:

```rust
const STAR: u64 = ((0x0023_u64 << 48)  // CS=0x33 for 64-bit user code (R3)
                 | (0x0000 << 32)         // SS=0x00 (kernel data segment)
                 | (0x0018 << 16));        // SS=0x18 for 32-bit compat (R0)

// Ver §11
```

### 10.7 IA32_FMASK (0xC0000084)

Máscara de RFLAGS a limpiar en SYSCALL. Por defecto, limpiar IF (bit 9) es recomendado para que las interrupciones se deshabiliten durante el dispatcher. **A diferencia de Intel (que requiere SWAPGS + clearing IF), AMD limpia IF automáticamente** durante SYSCALL.

Común: `IA32_FMASK = (1 << 9) = 0x200` (just IF). Algunas configs limpian también AC (alignment check), pero no es necesario.

### 10.8 IA32_TSC_AUX (0xC0000103)

`RDTSCP` lee TSC y luego `IA32_TSC_AUX[31:0]`. Usado por el kernel para identificar el CPU que ejecutó la lectura (para implementar `getcpu(2)`-like).

### 10.9 SPEC_CTRL y PRED_CMD

* `SPEC_CTRL` (`0xC0011031`): poner a 1 bits para deshabilitar speculation:
  * Bit 0 = IBRS. Set en kernel entry (SYSCALL), clear en exit (SYSRET). 1 = restringir.
  * Bit 1 = STIBP. Si el thread tiene hyper-threads con código no confiable, set 1.
  * Bit 2 = SSBD. Deshabilita Store-Store speculation (Spectre v4).
  * Bit 7 = PSFD (Predictive Store Forwarding Disable) — Zen 3.
* `PRED_CMD` (`0xC0011032`): write-only. Escribir 0 ejecuta IBPB (Indirect Branch Prediction Barrier). Flush predictor indirecto.

> **Mitigación práctica:** el kernel debería configurar IBRS al menos para transiciones de ring 3 → ring 0, o usar retpolines. Ver AMDwhite paper "Software Techniques for Managing Speculation on AMD Processors".

---

## 11. SYSCALL / SYSRET (ABI AMD64)

AMD64 define SYSCALL (kernel entry) y SYSRET (kernel exit). No usar SYSENTER/SYSEXIT en long mode: solo en legacy.

### 11.1 Pre-condiciones

* `IA32_EFER.SCE = 1`.
* `IA32_STAR`, `IA32_LSTAR`, `IA32_FMASK` configurados.
* `CS`, `SS` segments preparados (long mode usa descriptors planos, GDT debe tenerlos).
* `CR4.SMAP` y `CR4.SMEP` configurados según política.

### 11.2 Convenciones

* **RDI, RSI, RDX, R10, R8, R9** = argumentos del syscall (en orden).
* **RAX** = syscall number (entrada), return value (salida).
* **RCX** = saved RIP (en SYSCALL; el kernel lo pone en la stack o lo salta al final).
* **R11** = saved RFLAGS (en SYSCALL).
* **SYSCALL** limpia RFLAGS.IF y RFLAGS.RF (incluso si IA32_FMASK no lo pide), deshabilita interrupciones hasta que el kernel haga STI.
* **SYSCALL** no salva los GPRs; el kernel handler debe hacer `push %rcx; push %r11; ...`.
* **SYSCALL** carga CS de IA32_STAR[47:32] y SS de IA32_STAR[31:16], y luego hace jump a IA32_LSTAR.

### 11.3 Layout recomendado de stack

```
[ RSP+0x00 ] = RIP
[ RSP+0x08 ] = CS
[ RSP+0x10 ] = RFLAGS
[ RSP+0x18 ] = RSP
[ RSP+0x20 ] = SS
```

(Push de hardware al entrar a una interrupt gate; SYSCALL no lo hace.)

### 11.4 Plantilla de handler (x86-64)

```asm
global syscall_entry
syscall_entry:
    swapgs                          ; AMD NO requiere swapgs, Intel sí
                                    ; (ver §11.6)
    mov   [gs:USER_RSP], rsp        ; guardar user rsp (si se usa gs)
    mov   rsp, [gs:KERNEL_STACK]    ; cargar kernel stack

    push  rcx                       ; RIP saved
    push  r11                       ; RFLAGS saved
    push  rbp
    push  rbx
    push  r12
    push  r13
    push  r14
    push  r15
    ; RDI/RSI/RDX/R10/R8/R9 ya están en regs; no perderlos.

    ; Llamar al dispatcher C/Rust
    mov   rdi, rax                  ; syscall number
    ; rsi, rdx, r10, r8, r9 = args
    call  syscall_dispatch

    ; Restaurar
    pop   r15
    pop   r14
    pop   r13
    pop   r12
    pop   rbx
    pop   rbp
    pop   r11
    pop   rcx

    mov   rsp, [gs:USER_RSP]        ; volver a user rsp
    swapgs

    ; SYSRET espera RCX = nuevo RIP, R11 = nuevo RFLAGS
    mov   r11, rflags_saved
    oem   iret? No, es sysretq:
    sysretq
```

### 11.5 SYSRET Q vs QC

* `sysret` (sin prefijo): SYSRETQ; **debe** ser en modo long 64-bit.
* `sysretq` (con prefijo REX.W=1): idem.
* `sysret` con REX.W=0: legacy, vuelve a 32-bit compat.

### 11.6 Diferencia con Intel

| Característica | AMD | Intel |
|---|---|---|
| SYSCALL limpia IF | **SÍ** | NO (debe ser SWAPGS + clearing manual o por hardware) |
| SWAPGS en SYSCALL | NO requerido | REQUERIDO |
| TLB flush en SYSRET | NO (TLB entries preservan PCID) | Algunos modelos requieren INVLPG |
| Segment selectors | STAR[47:32] = CS, STAR[31:16] = SS | IA32_STAR similar |
| 32-bit compat | Soporte completo | Soporte completo |
| `syscall` instruc. desde 32-bit | Soporte | Soporte |

> **Importante para FastOS:** el kernel está optimizado para el 5600X. Esto significa que **no hay que hacer SWAPGS** en SYSCALL/SYSRET, simplificando enormemente el código.

### 11.7 Verificación en CPUID

`CPUID.0x80000001:EDX[11] = 1` → SYSCALL/SYSRET en long mode ✅.

---

## 12. TSC y timers

### 12.1 TSC

* **Registro 64-bit** (no en Intel: en AMD el TSC es 64-bit).
* `RDTSC` lee EDX:EAX (en x64: RAX[63:32] = 0 si se usa RDX).
* `RDTSCP` lee EDX:EAX + ECX = `IA32_TSC_AUX`.
* **No es invariant** en Zen 3 (a pesar de que CPUID.80000007H:EDX[8] = 1).
* El TSC cuenta con la frecuencia del **Infinity Fabric Clock Domain** (FCLK) cuando no hay P-state; con el **COF (Core Operating Frequency)** cuando sí. La transición produce un offset súbito.
* AMD recomienda usar **`IRPerfCount` o `MPerfCount`** para medir, no el TSC.

### 12.2 TSC vs frecuencia

El TSC en Zen 3 se incrementa a una frecuencia **fija** igual al **P0 (max boost)** o al **`crystal clock`** (100 MHz) — varía con el stepping:

* **Stepping B0/B2 (Vermeer):** TSC se incrementa a la **frecuencia P0 (4.6 GHz)**, no a la frecuencia actual. Esto es un **invariant TSC en el sentido práctico** (incrementos por segundo son constantes).
* **Variación de TSC con P-state:** **NO en Zen 3** (al contrario que en Zen/Zen+/Zen 2). Es por eso que Linux marca `constant_tsc`.
* **Variación con C-state:** la primera lectura de TSC tras S3 (sleep) o S1 (halt) puede tener un offset. El kernel debe **resincronizar** al volver de C-state profundo.

> ⚠️ **Diferencia clave vs Intel:** en Intel Nehalem+ el TSC es **constant rate TSC** (crystal clock). En AMD Zen 3 es **TSC at P0** (4.6 GHz). En ambos casos, el TSC se incrementa linealmente con el tiempo, no con la frecuencia del core. Por eso son utilizables como clocksource, pero la **frecuencia nominal del TSC depende del modelo**.

### 12.3 TSC Auxiliary

`IA32_TSC_AUX` (MSR `0xC0000103`): el kernel escribe un valor único por CPU (típicamente el `lapic_id`). `RDTSCP` lo lee en ECX. Usado para:
* `getcpu()` style.
* Verificar que la lectura se hizo en el core esperado.

### 12.4 Timers del APIC

* **TSC-deadline (modo 010):** preferred. Programar `IA32_TSC_DEADLINE` con un valor TSC futuro. El LAPIC dispara el vector configurado en LVT Timer cuando TSC = deadline.
* **Periodic mode (modo 001):** LAPIC timer usa `Divide Configuration` (bits 0-3) + `Initial Count` como divisor. No recomendado en modernos.
* **One-shot (modo 000):** igual a periodic pero con reload manual.

### 12.5 HPET

HPET está disponible en chipsets AM4 (B550, X570, etc.) pero **no** es parte del CPU. El 5600X **no** integra HPET. La placa base lo expone en MMIO. Se accede vía `0xFED00000` (default).

Para FastOS, recomendable: **TSC como clocksource principal** + **TSC-deadline LAPIC timer** para el scheduler.

---

## 13. P-states, C-states y boost

### 13.1 C-states

* **C0:** activo.
* **C1:** HLT ejecutado; core no ejecuta. Latencia de wake-up ~10 µs.
* **C2:** stop grant. Más ahorro, latencia mayor.
* **C3:** sleep. Aún más ahorro, latencia ~100 µs.
* **C6 (deep):** no existe en Zen 3 desktop. En Zen 3 mobile, sí (CC6).

### 13.2 P-states

Los P-states son **frecuencias operativas** seleccionables vía MSR. **P0 = máximo boost**, **P1 = base**, **P2..P7 = frecuencias más bajas**. En el 5600X:

| P-state | Frec (MHz) | Voltaje | Notas |
|---|---|---|---|
| P0 | hasta 4600 (con boost) | variable | max boost 1C |
| P1 | 3700 | variable | base |
| P2 | ~3500 | variable | |
| ... | ... | ... | hasta P7 o P8 |

Configurables vía MSR `0xC0010200..0xC0010207` (PStateDef registers). Cada uno es un solo valor de 64 bits con campos `CpuDfsId` (FID) y `CpuDfsVid` (VID).

> **No tocar P-states desde el kernel** sin coordinación con ACPI/SMI. La política está en el firmware/BIOS.

### 13.3 Precision Boost 2 (PB2)

Zen 3 usa **PB2**: cada core puede boost independientemente según:
* Carga de los otros cores (PPT, TDC, EDC limits).
* Temperatura.
* Tipo de instrucciones (AVX2 tira más = boost menor).

Boost single-core max: **4.6 GHz** en el 5600X. Con AVX2: ~4.0-4.2 GHz (depende del cooler).

### 13.4 STAPM, PPT, TDC, EDC

Limitadores principales:
* **PPT (Package Power Tracking):** potencia total del package.
* **TDC (Thermal Design Current):** corriente sostenida.
* **EDC (Electrical Design Current):** pico de corriente.
* **STAPM (Skin Temperature Aware Power Management):** temperatura de la piel del die.

Configurables vía SBIOS, **no** vía MSR estándar.

### 13.5 TSC durante transiciones de P-state

**No** hay offset en Zen 3 (al contrario que en CPUs antiguas). Por eso el TSC se mantiene lineal.

---

## 14. MTRR y PAT

### 14.1 MTRR — Memory Type Range Registers

* **8 pares (variable-range):** `0x200-0x20F` (PHYSBASE 0..7) y `0x201-0x217` (PHYSMASK 0..7). Cubre hasta 8 rangos de memoria con tipos cache variables.
* **Fixed-range MTRRs:** `0x250-0x26F`. Cubre la primera MB de memoria con cachés específicos (compatible con BIOS/real mode).
* **Default type:** `IA32_MTRR_DEF_TYPE` (0x2FF).
* **Cap:** `IA32_MTRRCAP` (0x0FE).

**Memory types:**
* `00 = UC` (Uncacheable).
* `01 = WC` (Write-Combining).
* `04 = WT` (Write-Through).
* `05 = WP` (Write-Protected).
* `06 = WB` (Write-Back). **Default para DRAM.**
* `07 = UC-` (Uncacheable minus, similar a UC pero con ligero reordering).

**Habilitar:**
```c
// 1) Habilitar MTRRs en MTRR_DEF_TYPE
msr = read_msr(0x2FF);
msr |= (1 << 11) | (1 << 10);  // MTRR + Fixed enable
write_msr(0x2FF, msr);

// 2) Configurar pares (orden importante: escribir MTRR_DEF_TYPE.disable=1 primero)
// 3) Re-habilitar
```

### 14.2 PAT — Page Attribute Table

8 entradas (3 bits por PTE × 2 niveles = 8 combinaciones), con `PCD, PWT, PAT` como selector.

**Default layout (legacy compatible):**

| Índice | PAT | PCD | PWT | Tipo |
|---|---|---|---|---|
| 0 | 0 | 0 | 0 | **WB** (default) |
| 1 | 0 | 0 | 1 | WT |
| 2 | 0 | 1 | 0 | UC- |
| 3 | 0 | 1 | 1 | UC |
| 4 | 1 | 0 | 0 | **WB** (PAT bit en PTE de 4KB) |
| 5 | 1 | 0 | 1 | **WP** |
| 6 | 1 | 1 | 0 | **WC** |
| 7 | 1 | 1 | 1 | **UC-** |

> **Para un kernel moderno**, se suele configurar un layout distinto, con `WC` en índice 4 (para MMIO framebuffer) y `WB` como default. Esto es lo que hace Linux.

**Modificar `IA32_PAT` (MSR 0x277):**
```c
u64 pat = 0x00000000_01040100;  // (rough)
write_msr(0x277, pat);
```

### 14.3 Write-Combining

Las regiones WC son críticas para:
* Framebuffers GPU.
* MMIO con burst writes (NIC, storage).
* Shared memory con copia a GPU.

**Comportamiento del WC en el 5600X:**
* Búfer interno de 64 bytes (una cacheline).
* Las escrituras se *mergean* en orden de programa (no se reordenan dentro de un buffer).
* **Requieren SFENCE para flushing** si el kernel o device debe ver los datos.

---

## 15. Erratas relevantes

Las erratas del 5600X (Vermeer, Zen 3) están en el documento oficial de AMD "Revision Guide for AMD Family 19h Models 01h, B1 and 08h, B0" (publicado, requiere NDA). **Las más importantes para OS developers, no documentadas explícitamente aquí, son:**

### 15.1 Spectre / Meltdown (microarchitectural)

Zen 3 es vulnerable a:

* **Spectre v1 (bounds check bypass):** mitigado con LFENCE después de bounds checks.
* **Spectre v2 (branch target injection):** mitigado con **IBRS** (Indirect Branch Restricted Speculation). Activar `IA32_SPEC_CTRL[0] = 1` durante `SYSCALL`/interrupts, limpiar al volver a ring 3.
* **Spectre v4 (speculative store bypass):** mitigado con `SSBD` (`IA32_SPEC_CTRL[2]`).
* **Meltdown:** Zen 3 **NO es vulnerable** (a diferencia de Intel). No necesita KPTI-like mitigations.
* **MDS (Microarchitectural Data Sampling):** Zen 3 es parcialmente vulnerable. Mitigar con `VERW` o microcode.
* **PSF (Predictive Store Forwarding):** nueva vulnerabilidad en Zen 3. Mitigar con `SPEC_CTRL[7] = 1` (PSFD) o desactivando PSF con `MSR 0xC0011036`.

> El kernel debe consultar el **microcode** de la CPU: `IA32_PATCH_LEVEL` (MSR 0x8B).

### 15.2 Erratas comunes reportadas

* **#GP con INVLPGB** si PCID bits no se configuran correctamente. Confirmar CPUID.0x80000008.
* **TSC drift entre cores** durante ciertos estados de boost (≤ 1 TSC tick en algunas situaciones). Para clocksource de alta precisión, usar ICR_LVT o HPET.
* **Stale TLB entries** en ciertos contextos tras SMP wakeup si se omite la invalidación cross-core.
* **PSF incorrect forwarding** (Zen 3-specific). Mitigado en microcode AGESA ≥ 1.2.0.0.

### 15.3 Workarounds estándar

* **Set IBRS en kernel entry (SYSCALL), clear en exit (SYSRET).**
* **Poner LFENCE después de cada access a user-provided index** (Spectre v1).
* **Hacer IBPB tras cada context switch** si hay rings mezclados.
* **Verificar microcode version** con `IA32_PATCH_LEVEL` y rechazar CPUs con versiones vulnerables.
* **No usar ICR para IPI en `INIT deassert`** sin esperar al menos 1 µs (cuestión de timing).

### 15.4 OSVW (OS Visible Workaround)

* `MSR 0xC0010140` = OSVW_ID_Length.
* `MSR 0xC0010141[OSVW_Status bits]` = 1 si el workaround para un erratum N está aplicado por el hardware (microcode).
* El kernel debe consultar antes de aplicar un workaround manual.

---

## 16. Zen 3 vs Zen 2 vs Zen 4

### 16.1 Diferencias Zen 3 vs Zen 2 (resumen)

| Aspecto | Zen 2 | Zen 3 |
|---|---|---|
| Family/Model | 17h/71h | 19h/01h |
| L3 organization | 2 × 16 MB (2 CCX) | **1 × 32 MB (1 CCX × 8 cores)** |
| Front-end: decode | 4-wide | 4-wide |
| Issue width | 7 µops | 10 µops |
| L1 BTB | 512 | 1024 |
| L2 BTB | 4096 | 6656 |
| RAS | 27 | 32 |
| INVLPGB | ❌ | ✅ nuevo |
| VAES / VPCLMULQDQ | ❌ | ✅ nuevo (no en 5600X, pero en 5600X con SM3/SM4 NO) |
| CET_SS / IBT | ❌ | ❌ (preparado en algunos) |
| AVX-512 | ❌ | ❌ |
| L3 latency | 39 cycles | 46 cycles (peor! más L3 pero más lenta) |
| Bypass para reordering | agresivo | aún más agresivo |

### 16.2 Zen 3 vs Zen 4 (Raphael 7000)

Zen 4 introduce:

* **AVX-512** (512-bit FPU, donde Zen 3 no tiene).
* **5-level paging (LA57)** ✅ (Zen 3 no).
* **AM5 socket** (Zen 3 = AM4).
* **DDR5** (Zen 3 = DDR4).
* **TSMC 5 nm** (Zen 3 = 7 nm).
* **Front-end: 6-wide decode** (Zen 3 = 4-wide).
* **ROB 320 entries** (Zen 3 = 256).
* **WBNOINVD** ✅ (Zen 3 no).
* **AVX-VNNI** ✅ (Zen 3 no).
* **APX (Advanced Performance Extensions)** preliminares.
* **Hypervisor changes** para mejor rendimiento VMX.

**Por qué el 5600X no soporta AVX-512:** el die de Zen 3 usa el mismo "fused domain" AVX/AVX2 que Zen 2, con datapaths de 256 bits, sin vector register file de 512 bits. Decisión deliberada de AMD para mantener área del die manejable y mantener el chip dentro del TDP objetivo. La introducción de AVX-512 en Zen 4 es una decisión técnica y de mercado.

### 16.3 Tabla comparativa (5600X vs 7600X vs 9700X)

| Aspecto | 5600X (Zen 3) | 7600X (Zen 4) | 9700X (Zen 5) |
|---|---|---|---|
| Family/Model | 19h/01h | 19h/61h | 1Ah/?? |
| Socket | AM4 | AM5 | AM5 |
| Memoria | DDR4 | DDR5 | DDR5 |
| AVX-512 | ❌ | ✅ (en 12h model) | ✅ |
| LA57 | ❌ | ✅ | ✅ |
| Max cores | 16 (5950X) | 16 (7950X) | 16 (9950X) |
| Max TSC MHz | ~5.0 | ~5.7 | ? |
| TSC constant? | sí (P0) | sí (P0 o fixed) | sí |

---

## 17. Recursos oficiales y enlaces

### Documentación oficial AMD

* **AMD64 Architecture Programmer's Manual Vol. 1: Application Programming** (`24592.pdf`).
* **AMD64 Architecture Programmer's Manual Vol. 2: System Programming** (`24593.pdf`).
* **AMD64 Architecture Programmer's Manual Vol. 3: General-Purpose and System Instructions** (`24594.pdf`).
* **AMD64 Architecture Programmer's Manual Vol. 4: 128-bit and 256-bit media instructions** (`33247.pdf`).
* **AMD64 Architecture Programmer's Manual Vol. 5: 64-bit Media and x87 Floating-Point Instructions** (`26569.pdf`).
* **AMD Family 19h Model 01h, Revision Guide** — bajo NDA, contactando a AMD.
* **AMD Speculative Store Bypass / Spectre Whitepaper** — público.
* **AMD Software Techniques for Managing Speculation** — público.

URLs principales:
* https://www.amd.com/en/search/documentation/hub.html (buscador)
* https://developer.amd.com/resources/developer-guides-manuals/ (manuales)

### Recursos no oficiales útiles

* **WikiChip Zen 3** (https://en.wikichip.org/wiki/amd/microarchitectures/zen_3).
* **Chips and Cheese — A Look at Zen 3** (mediciones microarquitecturales).
* **AnandTech — "AMD Zen 3 Architecture Deep Dive"** (Cutress, 5/11/2020).
* **Travis Goodspeed — "Building a RISC-V CPU on a 5600X"** y similares.
* **OSDev Wiki** (http://wiki.osdev.org).
* **AMD microcode updates** vía Linux firmware tree.

### Software de referencia

* **Linux kernel arch/x86/** — la implementación canónica.
* **coreboot/AMD** — BIOS open source para AMD.
* **SeaBIOS** — opción legacy.
* **muen** y **sel4** — microkernels académicos.
* **xv6** (MIT) — kernel educativo, portarlo a Zen 3 es instructivo.

### Páginas de Wikipedia

* [Zen 3](https://en.wikipedia.org/wiki/Zen_3)
* [Ryzen](https://en.wikipedia.org/wiki/Ryzen)
* [CPUID](https://en.wikipedia.org/wiki/CPUID)
* [Memory ordering](https://en.wikipedia.org/wiki/Memory_ordering)
* [MTRR](https://en.wikipedia.org/wiki/Memory_type_range_register)
* [Page attribute table](https://en.wikipedia.org/wiki/Page_attribute_table)
* [Local APIC](https://en.wikipedia.org/wiki/Advanced_Programmable_Interrupt_Controller)
* [Model-specific register](https://en.wikipedia.org/wiki/Model-specific_register)
* [Control register](https://en.wikipedia.org/wiki/Control_register)
* [x86-64](https://en.wikipedia.org/wiki/X86-64)

---

## 18. Practical notes for kernel development

Esta sección es la **más importante** para FastOS. Es una lista de cosas que **DEBES** tener en cuenta al escribir el kernel.

### 18.1 Verificación de CPU

* En el **init del kernel**, **comprobar** que estamos en un 5600X (Family 19h, Model 01h). Si no, **panic** inmediato. (Justificación: el kernel está específicamente optimizado para este CPU.)
* Verificar vendor string = `"AuthenticAMD"`.
* Verificar brand string contiene "5600X".
* Activar las features **esperadas** leyendo CPUID (no hardcodear `true`).

### 18.2 Inicialización de long mode

Secuencia correcta:

1. Crear GDT mínima con descriptores para kernel 64-bit.
2. Crear IDT vacía o con stubs.
3. Construir PML4, PDPT, PD, PT para identity-map los primeros 1 MB (BIOS, MTRR, ACPI).
4. PML4/PDPT/PD/PT deben estar en memoria **type WB** (no UC, no WC, no WT).
5. Activar MTRR: MTRR_DEF_TYPE.enable, MTRR_PHYSBASE0/PHYSMASK0 para 0..1MB con tipo WB.
6. Activar CR4.PAE.
7. Configurar IA32_EFER (LME + NXE + SCE + FFXSR).
8. Activar CR0.PG (esto activa long mode).
9. `jmp far` a código 64-bit.

### 18.3 GDT/IDT/TSS

* GDT con:
  * NULL descriptor (selector 0).
  * 64-bit kernel code (CS = 0x08).
  * 64-bit kernel data (DS/SS/ES/FS/GS = 0x10).
  * 32-bit compat code (CS = 0x18, si se necesita).
  * 32-bit compat data (DS/SS = 0x20, si se necesita).
  * TSS descriptor (selector 0x28). TSS con ISTs.
* TSS debe tener:
  * RSP0, RSP1, RSP2 (rings 0/1/2 stack).
  * IST1..IST7 (8 entradas IST).
  * IO map base = offset fuera de TSS (no usar I/O map legacy, usar IOPL en RFLAGS o desactivar directamente).
* IDT con 256 gates; los IST fields para #DF, NMI, #MC, #DB.

### 18.4 Paging: estrategia recomendada

* Usar **PCID** desde el primer día: ahorro masivo en TLB miss rate.
* Usar **2 MiB huge pages** para la **mayoría** de kernel text/data y stacks de usuario. Reduce el número de page walks.
* Usar **1 GiB pages** para el kernel text/data de gran tamaño (si el kernel cabe).
* Usar **4 KiB pages** solo en regiones que requieren page-level protections distintas.

> Una PTE de 2 MiB tiene bit 7 (PAT) = 1, NX en bit 63, etc. Asegurarse de **limpiar** los bits no usados antes de insertarla en la page table.

### 18.5 Sincronización y memory ordering

* **Nunca** asumir que un store es visible globalmente sin SFENCE o LOCK.
* **Nunca** asumir que un load ha visto el último store de otro core sin barrier.
* Implementar `mb()`, `rmb()`, `wmb()` como:
  * `mb()` = `mfence`
  * `rmb()` = `lfence`
  * `wmb()` = `sfence`
* **Spinlocks:** usar `lock cmpxchg` o `lock xchg`. Verificar que están **acquire/release** (LOCK da ambos).
* **Atómicos:** usar `lock add`, `lock xadd`, `lock cmpxchg16b` (no 8b), etc.
* **SeqLocks:** basado en seq counter con `lock xadd`. Solo escritura con LOCK implícito.

### 18.6 APIC: programación de IPI

* **NO** leer registros APIC sin haberlo habilitado (APIC_BASE.GE = 1).
* **NO** modificar registros APIC desde ring > 0.
* Después de escribir ICR_Low con SIPI, **esperar** a que el bit Delivery Status (bit 12) pase a 0.
* Para TIMER: usar **TSC-deadline** siempre. Olvidarse de los modos legacy.
* **SMP boot:** seguir la secuencia INIT → Deassert INIT → SIPI → SIPI. Esperar entre cada paso.

### 18.7 Mitigaciones de seguridad (recomendado)

* **IBRS:** en SYSCALL, `wrmsr(SPEC_CTRL, current | IBRS)`. En SYSRET, `wrmsr(SPEC_CTRL, current | IBRS)` (mantener activo).
  * AMD recomienda **IBRS always-on** en kernels modernos para Zen 3.
* **IBPB:** tras cada context switch. `wrmsr(PRED_CMD, 0)` (write-only; valor irrelevante).
* **SSBD / PSFD:** activar para código que maneja user input.
* **No usar retpolines** en AMD (preferir IBRS+IBPB+LFENCE).

### 18.8 FPU / XSAVE

* Inicializar CR0.EM = 0, CR0.MP = 1, CR4.OSFXSR = 1, CR4.OSXSAVE = 1, XCR0 = 0x1F (x87+SSE+AVX+...).
* Usar `xsaveopt` o `xsaves` (no `xsave` regular; más lento). `xsaveopt` está en `CPUID.0xD.0:EAX[0]` o `ECX[0]`.
* Tamaño de XSAVE area: leer de `CPUID.0xD.0:EBX` (legacy) o `0xD.1:EBX` (compacted). Para Zen 3: ~2688 bytes (legacy), ~2496 bytes (compacted).
* `XSAVE` debe ir en una **región de 64 B alineada**.
* **FPU context switch:** `xsave` save / `xrstor` restore.

### 18.9 TSC y clock

* **Usar TSC como clocksource principal** (constant, P0 = 4.6 GHz).
* **Programar TSC-deadline** para el scheduler. El vector 32+1 = 33 es un buen lugar.
* **Hacer resync** del TSC al volver de S3 (sleep): el TSC puede tener offset. Usar el timer HPET o PIT para validar.
* **LAPIC timer** siempre programable en TSC-deadline. No usar one-shot legacy.

### 18.10 Errores comunes

1. **Olvidar EFER.NXE antes de usar el bit NX en PTE** → NX se ignora silenciosamente.
2. **Mover APIC_BASE** sin deshabilitar antes → #GP.
3. **Escribir ICR_Low sin esperar el bit Delivery Status** → IPI se pierde o se solapa.
4. **Usar LAPIC timer en modo periodic** (con divisor low) → imprecisión.
5. **No invalidar TLB tras CR3 change con PCID** → TLB contiene entradas stale con el viejo PCID.
6. **Asumir que `RDTSC` es serializante** → NO lo es. `CPUID` es serializante, `LFENCE` también. Usar `RDTSCP` + LFENCE si se necesita orden.
7. **Usar `MOV CR3` sin preservar PCID** → invalida todo el TLB (incluyendo entradas con PCID actual).
8. **Activar SMAP sin STAC/CLAC en handlers de IRQ/syscall** → el kernel no puede leer user memory para entregar a procesos.
9. **Olvidar IST para #DF** → #DF dentro de #DF = triple fault = reset.
10. **No verificar `IA32_PATCH_LEVEL`** → sistema vulnerable a errata conocidas.
11. **Asumir que el TSC es constant sin verificar CPUID.0x80000007:EDX[8]** (en AMD, esto es 1, pero **no implica constant TSC** en el sentido Intel).
12. **Hacer `WBINVD` en lugar de `INVLPGB`** para TLB shootdown → órdenes de magnitud más lento.
13. **Escribir EFER sin desactivar CR0.PG** → #GP.

### 18.11 Tabla de prioridades

Para la primera versión de FastOS, en orden de importancia:

1. ✅ IDT + GDT + TSS.
2. ✅ Long mode con identity map.
3. ✅ Paging con PCID.
4. ✅ Local APIC.
5. ✅ SMP boot (INIT-SIPI-SIPI).
6. ✅ TSC-deadline timer.
7. ✅ SYSCALL/SYSRET handler.
8. ✅ Stack switch per-core.
9. ⚠️ MTRR/PAT (después de que paging funcione).
10. ⚠️ TLB shootdown (después de multiprocesamiento).
11. ⚠️ Mitigaciones Spectre (después de que el scheduling básico funcione).

### 18.12 Comprobaciones de cordura (sanity checks)

Antes de marcar el kernel como "ready", verificar:

- [ ] CPUID 0, 1, 7, 0x80000001, 0x80000008 leídos correctamente.
- [ ] Brand string termina en "5600X".
- [ ] CPUID Family = 0x19, Model = 0x01.
- [ ] EFER = (1 << 0) | (1 << 8) | (1 << 11) | (1 << 4).
- [ ] LME = 1, LMA = 1, NXE = 1, SCE = 1, SVME = 0 (a menos que se use).
- [ ] CR0 = PE | MP | PG | ET (no EM, no NE? mejor NE = 1).
- [ ] CR4 = PAE | PGE | PCIDE | SMEP | SMAP | OSFXSR | OSXSAVE | FSGSBASE.
- [ ] APIC_BASE = 0xFEE00000 con bit GE = 1 y BSP = 1 en el BSP.
- [ ] IDT base y límite correctos.
- [ ] IDTR y GDTR son válidos.
- [ ] IDT tiene IST para #DF.
- [ ] TSS.RSP0 cargado.
- [ ] Cada AP ha completado SIPI y ejecuta kernel.
- [ ] LAPIC timer configurado en TSC-deadline.
- [ ] `serial_write` de una cadena conocido desde cada core (verifica printf en SMP).
- [ ] TSC con valor creciente monotónicamente entre cores.
- [ ] MFENCE serializa lecturas entre cores (test con 2 cores).

---

## Apéndice A. Diagrama del startup del kernel

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

## Apéndice B. Mapa de memoria (recomendado para FastOS)

```
0x0000000000000000 - 0x00000000FFFFFFFF  user space (-2 MiB)  (typical high-half layout: 0xFFFF8000_00000000 - ... kernel)
0x0000000000000000 - 0x000000003FFFFFFF  user text/data/heap (low)
0x00000000FFC00000 - 0x00000000FFFFFFFF  user stack + VDSO
...
0x0000000100000000                       user/kernel split (typical 64-bit)
...
0xFFFF800000000000 - 0xFFFF80FFFFFFFFFF  kernel direct map (physical RAM)
0xFFFF810000000000 - 0xFFFF810FFFFFFFFFF  kernel text
0xFFFF820000000000                          I/O hole 1
0xFFFF830000000000                          I/O hole 2
0xFFFFFFFFFF7FF000                          MMIO hole
0xFFFFFFFF80000000                          kernel text (canonical high)
0xFFFFFFFF80000000 - 0xFFFFFFFFFFFFFFFF  kernel text/rodata
0xFFFFFFFFFF600000                          APIC/IO register map
0xFFFFFFFFFFE00000 - 0xFFFFFFFFFFFFFFFF  per-CPU data, GDT, IDT
```

**Layout alternativo (más simple para bare-metal):** kernel text en `0xFFFF800000000000` mapeado 1:1 a la RAM física.

## Apéndice C. Glosario

* **CCX (Core Complex):** grupo de cores que comparten L3 en Zen 3. El 5600X tiene 1 CCX con 6 cores activos.
* **CCD (Core Complex Die):** el die físico donde viven los CCXs. Vermeer CCD contiene 1 CCX de 8 cores.
* **IOD (I/O Die):** die separado de 12 nm con memoria, PCIe, USB, SATA, etc.
* **IF (Infinity Fabric):** interconexión entre CCDs y IOD. También referencia a la frecuencia del interconector (FCLK).
* **SMT (Simultaneous Multithreading):** nombre AMD para Hyper-Threading.
* **TSO (Total Store Order):** modelo de memoria x86. AMD TSO es ligeramente más débil que Intel TSO.
* **PCID (Process-Context Identifier):** tag de 12 bits en TLB entries para reducir flushes en CR3 changes.
* **IST (Interrupt Stack Table):** mecanismo para que el CPU cargue un stack fijo sin consultar CR3, usado en #DF, NMI, #MC.

---

## Notas de versión

* Documento: v1.0.0.
* Fuentes: AMD64 APM Vol. 1-3 (revisión 4.07+), Wikipedia Zen 3, AnandTech Cutress, WikiChip, observación directa.
* Errata / discrepancias: marcado con **[no verificado]** o **[estimación]** donde aplique.
* Esta es la **primera versión**. Pendiente: añadir los timing exactos de instrucciones (latencias/throughput) medidos con `uarch-bench` o `llvm-mca` sobre el 5600X real. Pendiente: añadir el test de "1-round-trip latencia cross-core" con LAPIC IPI. Pendiente: tabla completa de microcode updates (AGESA) y su efecto sobre errata.

---

*Fin del documento. Para erratas o sugerencias, abrir issue en el repositorio de FastOS.*
