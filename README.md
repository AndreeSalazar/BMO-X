# BMO-X — Bare Metal Orchestrator

Sistema operativo bare metal escrito en Rust, con un kernel de **capabilities y superficie de 3 syscalls congelada**. GPU por UEFI GOP (framebuffer). Sin dependencias de drivers propietarios.

**Estado**: ✅ **Arranca en hardware real** y llega hasta arriba — Ring 0 completo, escritorio en Ring 3 y **tres lenguajes propios ejecutando en silicio** (BMO C, BMO COBOL y Ada), compilados por el toolchain de la casa. Banco de pruebas: MSI A320M PRO MAX / AMD Ryzen 5 5600X (Zen 3), sin QEMU. Boot chain: UEFI unificado (BOOTX64.EFI con las etapas embebidas) → s1_cpu → s2_mem → kernel.

**El número que lo resume**: BMO-X ocupa **5.4 MiB de 14.8 GiB** de RAM en la máquina de pruebas.

**Superficie ABI**: `INVOKE` · `CHANNEL_KICK` · `WAIT` (congelada) + Capability Engine en Ring 0.

---

## Layout (multi-arch from day one)

BMO se divide en un **core agnóstico de CPU** y un **árbol de kernel por-CPU**.

```
BMO/
├── Ultra_kernel_x86-64/      ← kernel x86-64: shim UEFI unificado + 2 etapas + Ring 0
│   ├── uefi_chain/           ← shim UEFI: embebe s1/s2/kernel, arranca sin leer disco
│   ├── faggin/               ← etapas de arranque: s1_cpu, s2_mem, serial_shared
│   ├── boot_context/         ← contrato de handoff shim→s1→s2→kernel (magic + version)
│   └── kernel/               ← Ring 0: Capability Engine, scheduler, mm, syscall, UI
├── Ultra_userspace/          ← lado Ring 3, también x86-64 (workspace hermano)
├── platform/                 ← CORE agnóstico de CPU: bmo-abi, bmo-rt, drivers, servicios
│   ├── abi/                  ← bmo-abi (surface, capability, handle, BEF/BEX), bmo-rt
│   ├── shared/               ← bmo-hal, bmo-channel, hw-profile
│   ├── drivers/              ← xhci, ahci, nvme, fat32, net, audio, input, uhid, gpu/rdna4
│   └── services/             ← cabina-core, byte-defender, timeback
├── toolchain/                ← Build-time: frontends de lenguaje → BEF → linker → BEX
│   ├── lang/                 ← C, C++, COBOL (biblioteca de dialectos) → BEF
│   ├── bmo-linker/           ← extracción de símbolos / BMO_SYMBOLS.toml
│   ├── bef-bootstrap/        ← generador del payload init de Ring 3
│   └── sem-asm/              ← semantic-assembly (arch/standards/stdlib)
└── Ultra_kernel_aarch64/     ← (planeado) misma estructura, cadena ARM
```

`platform/` es la parte de BMO **verdaderamente agnóstica de CPU** — el formato
BEF, la superficie de syscalls, el canal lock-free, el control de versiones y los
frontends de lenguaje funcionan igual en cualquier CPU. Para portar a otra
arquitectura, duplicas `Ultra_kernel_x86-64/` como `Ultra_kernel_<arch>/` y
reescribes las **2 etapas** (`faggin/s1_cpu`, `faggin/s2_mem`, el único código
CPU-específico) más el asm inline del `_start` del kernel.



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

## Memory Allocator Architecture

```
alloc_pages_contiguous() / free_pages()   ← public API (unchanged)
        │
        ▼
  Per-CPU pagesets (orders 0..4, 16 slots each)
  Cache hot path → lock-free on local CPU
        │
        ▼
  BackingAllocator trait (pluggable)
      ├── BuddyAllocator  [default, --features alloc-buddy]
      │   O(log n) free lists, automatic coalescing
      │   1 byte metadata per physical page
      │
      └── LLFree          [opt-in, --features alloc-llfree]
          Lock-free, bitfield-based lower + tree-based upper
          Per-CPU tree reservations → zero contention on N cores
          Crash-consistent (persistent memory ready)
          Reference: Wrenger et al., USENIX ATC '23
```

- **Buddy** (default): 384 lines, proven, ideal for ≤6 cores
- **LLFree** (opt-in): 204 lines adapter + 2199 lines `llfree` crate (safe Rust), boots clean on Ryzen 5600X with identical behavior — no crashes, no regressions. Same binary footprint +25 KB.
- **Per-CPU pagesets** sit above both: each core caches 16 pages × 5 orders before touching the backing allocator. Zero lock contention on the hot path regardless of backing.

> **Tres estados, y confundirlos es lo que hace perder el hilo:**
> **✅ corre en metal** (se ha visto en el Ryzen, con foto o con línea de
> CABINA) · **✍️ escrito sin estrenar** (compila, enlaza, pasa sus tests… y
> ningún CPU lo ha ejecutado) · **⬜ diseño**. Sólo lo primero cuenta como
> hecho. La lista de lo que espera un arranque vive en `AVANCES.md`.

### ✅ Funciona en hardware real
- **Boot UEFI unificado** → `BOOTX64.EFI` con s1_cpu + s2_mem + kernel embebidos → GOP 1920x1080. Cero dependencia del lector FAT del firmware
- **GDT + TSS** Ring 0 / Ring 3 con IST1, e **IDT** de 256 entradas (#GP/#PF/#UD/#NM/#MF/#XM/#DE/#DF)
- **Tres syscalls congelados** — `INVOKE` · `CHANNEL_KICK` · `WAIT`. Todo lo demás son **subsyscalls**: operaciones sobre una capability, no puertas nuevas
- **Capability Engine**: 16 procesos × 64 ranuras, handles con generación anti-UAF; `revoke_all` al morir
- **Scheduler PREEMPTIVO** por timer del LAPIC, con switch real Ring 0 ↔ Ring 3 (`iretq` → CPL3 → `INVOKE` → CPL0 → `EXIT` → reap)
- **XSAVE per-task** — con su causa raíz pagada: `XSAVE` hace *merge* de la cabecera, no *store* (`BITACORA.md` Ep. 14)
- **Aislamiento de fallos**: un fault en CPL3 mata la tarea y BMO sigue
- **Page allocator** buddy (orders 0..11) + pagesets por CPU; **LLFree** (USENIX ATC '23) opcional con `--features alloc-llfree`
- **Heap** slab (16 tamaños) + **VMM** de 4 niveles con demand paging y CoW
- **Teclado USB propio** (xHCI + HID): layouts es-latam / es-españa / us en caliente, teclas muertas, AltGr real, edición estilo readline, repetición al mantener, LEDs, historial
- **Disco AHCI/SATA propio** + **GPT** + **FAT32**: el kernel lee y monta su disco. El volumen de datos se monta **para escritura**; el de arranque, nunca
- **ESTRATOS montado** con el superbloque leído y **la firma verificada antes de ejecutar**
- **La pantalla, la entrada, la consola, el directorio y los archivos son capabilities** (`KIND_FRAMEBUFFER` / `INPUT` / `CONSOLE` / `DIRECTORIO` / `ARCHIVO`): Ring 3 pinta con `mov` y el kernel se aparta
- **Endpoint RPC** (`KIND_ENDPOINT` + `KIND_REPLY`): dos procesos de Ring 3 hablándose a través del kernel, sin tocar los 3 syscalls
- **Compositor en Ring 3**, cargado de `sys/gui.bex` — cambiar el escritorio no recompila el kernel
- **Tres lenguajes propios en silicio**: BMO C, BMO COBOL (decimal exacto, File I/O, OCCURS) y **Ada** (ZFP + Annex F)
- **CABINA**: telemetría que GRABA en el instante del hecho (IRQ-safe), no encuesta
- **AMD Zen 3**: CPUID, erratas, calibración del TSC

### ✍️ Escrito y sin estrenar / parcial
- **Ratón USB**: enumera y entrega puntero y botones, pero el arreglo del **anillo de eventos compartido** (`BITACORA.md` Ep. 18) espera foto
- **Escritura de ESTRATOS**: la máquina de estados de la transacción existe y está probada; **nadie la ha cableado al dispositivo**. La ventana de datos lo dice en rojo, y tiene que decirlo
- **BEF nativo**: formato, validación, secciones, imports/exports; relocaciones y TLS en evolución
- **Linux Devour / Wine Devour**: leen ELF64 / PE64 y generan un contenedor BEF. No hay personalidad POSIX ni entorno Win32 — y **no está en la hoja de ruta** (ver "Estado de Linux y Wine")
- **ByteDefender**: sólo cabeceras BEF, sin heurística
- **TimeBack**: la API existe; captura y rollback no hacen nada todavía
- **C++**: frontend mínimo (~900 líneas), barato encima de C cuando toque
- **SMP**: el código de despertar los APs **existe** en `s1_cpu` (trampolín, INIT+SIPI, percpu) y **nadie lo llama**. Va el último a propósito: el día que corra un 2º núcleo, cada `static mut` del kernel es una carrera
- **BMO GPU**: esqueleto RDNA4 sin driver
- **Write-combining del framebuffer** (PAT): pendiente, y se notará — hoy cada píxel es una escritura sin caché

### ⬜ No existe, y varias a propósito
- **NVMe**: hay carpeta y no se usa. El NVMe de esta máquina lleva el **Windows del dueño**; el kernel pide el controlador **por TIPO**, nunca "el primero del barrido"
- **Red y audio**: sin pila de red; audio sólo `beep()`
- **I/O APIC**, **EDF scheduler**: no implementados
- **libc completa, ventanas con superficies compartidas, Vulkan/GPU, Wine**: descartados **de esta fase** a propósito — ver "Próximos pasos"

---

## Estrategia BEF/BMO y compatibilidad futura

La prioridad de FastOS es construir primero un ecosistema **nativo BMO**. Los
programas escritos o reconstruidos para BMO —por ejemplo en COBOL, BMO
Language, Rust o C— se compilan AOT a BEF y consumen directamente la ABI y las
librerías de Ring 3. Este camino permite estabilizar procesos, memoria,
filesystem, ventanas, IPC y seguridad sin depender de las reglas internas de
otro sistema operativo.

```text
COBOL / BMO / Rust / C → compilador AOT → BEF nativo → BMO ABI → FastOS
```

### Estado de Linux y Wine

Los módulos `mod_linux_devour` y `mod_wine_devour` ya se compilan y se incluyen
en `EFI/BOOT/modules`. Son prototipos de ingestión de ELF64 y PE64: reconocen el
formato original y pueden construir una representación BEF inicial. Esto
demuestra el punto de extensión, pero **no equivale todavía a ejecutar Linux o
Wine**.

Para que un BEF importado sea ejecutable se debe conservar o traducir
correctamente su layout virtual, relocaciones, imports, linker dinámico, TLS,
stack inicial y convenciones ABI. También hace falta traducir el significado
completo de cada servicio —argumentos, estructuras, errores y ciclo de vida—,
no solamente cambiar números de syscall.

Los árboles locales de `sources/linux` y `sources/wine` se conservan como
material upstream para estudiar UAPI, ABI, pruebas y componentes portables. El
kernel Linux completo no se pretende convertir en un módulo BEF. Wine sí puede
llegar a ejecutarse en Ring 3 cuando BMO disponga de una base POSIX suficiente.

### Evolución a un “triturador” real

El potencial a largo plazo de BEF es actuar como contenedor nativo, verificable
y cacheable para programas procedentes de otros formatos. La evolución prevista
es incremental:

1. **BEF/BMO nativo**: Ring 3 físico, procesos aislados, ABI estable y librerías
   para aplicaciones propias, priorizando la reconstrucción en COBOL/BMO.
2. **Linux mínimo**: personalidad Linux x86-64 para ELF estático con stack,
   memoria, archivos y syscalls compatibles.
3. **Linux dinámico**: threads, futex, señales, sockets, TLS, `.so` y linker
   dinámico; primero programas musl y herramientas pequeñas.
4. **Wine sobre BMO/POSIX**: `wineserver`, DLLs, handles, registro, filesystem
   DOS y conexión con el compositor BMO.
5. **Devour completo**: ELF/PE se analizan, normalizan, reciben capacidades BMO,
   se validan y se guardan como BEF reutilizable sin perder su semántica.

Por diseño, BEF es el **contenedor**, BMO es el **contrato de servicios y
seguridad**, y cada personalidad de compatibilidad es el **traductor semántico**.
Esta separación permite avanzar hoy con software nativo sin renunciar a la
compatibilidad Linux/Wine en el futuro.

---

## Estructura del proyecto

> ⚠️ **Este árbol es de FastOS, el proyecto anterior, y ya no existe así.** Se
> deja abajo como arqueología de lo que se devoró. **El layout vigente está al
> principio de este documento** ("Layout (multi-arch from day one)"), y el mapa
> de dentro del kernel es éste:
>
> ```
> Ultra_kernel_x86-64/kernel/src/ring0/
>   core/    entry.rs (_start), phase.rs (el arranque por fases), informe.rs, splash, font
>   cpu/  cpu_vendor/   GDT/IDT/TSS, XSAVE, Zen 3: CPUID, cachés, TSC, erratas
>   mm/      phys (frames), vmm (4 niveles), physmap de 16 GiB
>   task/    scheduler preemptivo, percpu, proc, el registro de programas
>   obj/     las capabilities: channel, input, framebuffer, console, archivo, endpoint
>   dev/     pci, usb (xHCI), disk (AHCI), console/serial, framebuffer, keyboard
>   fsys/    fat32 + el gate de identidad y la ventana de escritura
>   svc/     los servicios de Ring 0 registrados en el estuario 0
>   plat/    faults, timer (LAPIC)
>   cabina.rs   la telemetría que graba en el instante del hecho
> ```

```
FastOS/                      # ⚠️ HISTÓRICO — no es la estructura actual
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

El guion vive en `Ultra_kernel_x86-64/build.ps1`.

```powershell
# Compilar y validar, sin tocar ningun disco (es el valor por defecto)
.\build.ps1 -BuildOnly

# Ring 0 al volumen de arranque, y los programas al volumen de datos
.\build.ps1 -Flash -Drive A -Data A -Yes
```

**Las dos banderas están separadas a propósito**: `-Flash` toca la ESP de
arranque y `-Data` toca el volumen de programas. Que compartieran bandera
invitaría a escribir en uno cuando se quería el otro.

Y **nada se escribe fuera del árbol del proyecto sin tres cierres**: no puede
ser el disco del sistema, tiene que ser FAT/FAT32, y hay que teclear la frase
entera con la letra dentro (`-Yes` la salta, y es cosa del que lo teclea).
Todo lo copiado se verifica por **SHA-256** en el destino: un `.bex` a medio
copiar no falla al arrancar, falla en la admisión BEX — y ese mensaje manda a
buscar el bug al compilador en vez de al cable.

Lo que se despliega:

```
EFI\BOOT\    BOOTX64.EFI (848 KB, con las etapas y el kernel dentro) + BMO-MANIFEST.TXT
sys\         gui.bex          el compositor
cobol\ c\ ada\                los programas de ejemplo, por lenguaje
datos\       los .txt que leen esos programas
```

Requisitos:
- Rust **nightly** (el userspace se compila a `x86_64-unknown-none` con su
  propio guion de enlazado)
- UEFI con **Secure Boot desactivado**
- El disco de BMO montado con letra. En esta máquina es **A: (KINGSTON
  SA400S37480G SATA)**. ⚠️ El NVMe es el Windows del dueño y no se toca

El guion, en orden: valida el **contrato de syscalls** (drift guard) → s1_cpu →
s2_mem → **compositor de Ring 3** (`bex-link` traduce el ELF a `.bex` y fija las
direcciones) → los ejemplos de COBOL, Ada y C con los frontends propios →
kernel → `uefi_chain` que lo embebe todo → staging → despliegue verificado.

---

## Boot path

**No se lee ni un archivo del firmware.** `BOOTX64.EFI` lleva las dos etapas y
el kernel dentro (`include_bytes!`) y el kernel se copia a su dirección
**después** de `ExitBootServices` — el patrón del EFI stub de Linux. Es la
respuesta a la placa que nunca conectó un driver FAT (`BITACORA.md` Ep. 1).

```
UEFI Firmware
  → BOOTX64.EFI (uefi_chain: s1_cpu + s2_mem + kernel embebidos)
    1. Query GOP (1920x1080), memory map, RSDP
    2. ExitBootServices  ← aquí se acaban las mercedes del firmware
    3. Copiar las etapas y el kernel a sus direcciones y saltar
  → s1_cpu @0x100000   CPU: cli + enmascarar el PIC ANTES de tocar la GDT
                       (el firmware entrega con interrupciones ON, Ep. 2)
  → s2_mem @0x200000   memoria: mapa, physmap, handoff verificado por magic
  → kernel  @0x400000  ring0::core::entry::_start → phase::main(ctx)
    1. Validar BootContext (magic + version). Si no cuadra, se DICE en rojo
    2. xsave::init()      ← antes que nada que pueda atrapar: el área es fija
                            y el tamaño sólo lo sabe este CPU
    3. percpu + scheduler + mm (phys, vmm) + channel + servicios + syscall
    4. faults::init()     ← el reporte en pantalla ARMADO antes de que nada
                            pueda entrar a Ring 3
    5. timer::init()      ← tick del LAPIC: el scheduler pasa a preemptivo
    6. PCI → xHCI (teclado y ratón) → AHCI → GPT → FAT32 → ESTRATOS
    7. lanzar::ruta("sys/gui.bex")  ← el compositor, desde el DISCO
  → Ring 3: el escritorio. Si no arranca, la máquina se queda en el shell del
    kernel y CABINA dice por qué.
```

---

## Superficie congelada y Subsyscalls (teoría BMO)

**Subsyscall** (término BMO): una operación que viaja *dentro* de un syscall
congelado, dirigida a una capability. El kernel expone **3 puertas y solo 3
— para siempre**:

| # | Puerta | Rol |
|---|--------|-----|
| 0x00 | `INVOKE(cap, operation, a0..a3)` | Llamada síncrona — la única puerta de servicios |
| 0x01 | `CHANNEL_KICK(cap, seq)` | Notificar (async) |
| 0x02 | `WAIT(waitable, seq, timeout)` | Bloquear (async) |

Todo lo demás es un **subsyscall**: el par `(kind del handle × operation)`
resuelto por el Capability Engine. El sistema crece agregando *kinds* y
*operaciones* — **jamás** una puerta nueva.

### Subsyscalls registrados hoy

| Kind | Operation | # | Estado |
|------|-----------|---|--------|
| Task (`CURRENT_TASK`) | `GET_PID` | 0x01 | estable |
| Task | `GET_TID` | 0x02 | estable |
| Task | `YIELD` | 0x03 | estable |
| Task | `EXIT` | 0x04 | estable |
| Task | `CHANNEL_OPEN` | 0x05 | estable |
| Task | `CONSOLE_WRITE` | 0x06 | estable — encauza a `KIND_CONSOLE` si el proceso tiene una |
| Task | `ENDPOINT_CREATE` / `CONNECT` | 0x07 / 0x08 | *bootstrap* — falta servicio de nombres |
| Task | `FRAMEBUFFER_CLAIM` | 0x09 | estable — exclusivo |
| Task | `INPUT_CLAIM` | 0x0A | estable — exclusivo, ratón **y** teclado |
| Task | `RUTA` / `EJECUTAR` | 0x0B / 0x0C | estable — lanzar desde Ring 3, con gate de firma |
| Task | `CONSOLA_CREAR` | 0x0D | estable |
| Task | `DIR_ABRIR` | 0x0E | estable |
| Task | `CONSOLE_READ` | 0x0F | estable — la pareja de `CONSOLE_WRITE` |
| Channel | `GET_SEQ` | 0x01 | estable |
| Channel | `GET_INDEX` | 0x02 | estable |
| Framebuffer | `BASE` / `DIMS` / `STRIDE` / `BYTES` | 0x01–0x04 | estable |
| Input | `PUNTERO` / `EVENTOS` | 0x01 / 0x02 | estable |
| Input | `TECLA` / `MODIFICADORES` | 0x03 / 0x04 | estable |
| Console | `LEER` / `PERDIDOS` | 0x01 / 0x02 | estable |
| Console | `ESCRIBIR` / `HAY_HIJO` | 0x03 / 0x04 | estable |
| Directorio | `SIGUIENTE` / `NOMBRE` | 0x01 / 0x02 | estable |

**Cinco kinds, cero puertas nuevas.** Todo lo que se ha añadido —la pantalla,
la entrada, la consola con sus dos sentidos, los directorios, lanzar
programas— cabe en `INVOKE`. Eso es la prueba de que el ABI congelado
aguanta: el sistema creció y la frontera no se movió ni un número.

### Reglas del contrato

1. **Fuente única de verdad**: los números viven en `platform/abi/bmo-abi`
   (`syscalls/surface.rs`); el kernel los espeja y `build.ps1` tiene un
   drift-guard que rompe el build si divergen.
2. **Ciclo de vida**: un subsyscall puede nacer como *bootstrap* sobre Task
   (p.ej. `CONSOLE_WRITE`) y madurar a operación de una capability dedicada.
   Nacer es fácil; **la puerta nunca cambia**.
3. **By-value primero**: los argumentos viajan por registros. Payloads
   grandes van por BMO Channel (datos) — el subsyscall lleva el control.
4. **RPC a Ring 3**: el diseño Endpoint (`platform/abi/bmo-abi/src/ENDPOINT_RPC.md`)
   extiende `INVOKE` a servidores Ring 3 sin tocar la superficie.

### Prueba empírica (hardware real, 2026-07-22)

El primer programa Ring 3 vivió y murió con **9 llamadas por 1 sola puerta**
(8× `INVOKE·CONSOLE_WRITE` + 1× `INVOKE·EXIT`): superficie intacta.

### Compatibilidad futura (nota Wine/Win32)

Una capa de compatibilidad estilo Wine **no necesita puertas nuevas**: las
~450 NtXxx de Windows se traducen a subsyscalls y endpoints — exactamente el
patrón `wineserver` (un servidor de userspace que implementa la semántica NT
por IPC), que en BMO es *nativo* vía Endpoint RPC. La superficie ajena se
convierte en biblioteca + operaciones; la puerta sigue siendo `INVOKE`.

---

## Hardware

- **CPU**: AMD Ryzen 5 5600X (Zen 3, Family 19h, 6C/12T)
- **RAM**: Cualquier tamaño (detectado por UEFI memory map)
- **GPU**: UEFI GOP framebuffer (1920x1080 BGR, backbuffer)
- **Serial**: COM1 115200 baud (debug output)
- **PCI**: Enumeración por I/O ports (0xCF8/0xCFC)
- **ACPI**: RSDP/XSDT/MCFG/FADT
- **Disco**: AHCI/SATA propio (Kingston SA400S37480G, 447 GiB). OJO: el NVMe
  de esta máquina es el disco de Windows del dueño — el kernel pide el
  controlador POR TIPO, nunca "el primero del barrido"

---

## Próximos pasos

**Hitos conseguidos en hardware real** (Ryzen 5 5600X + MSI A320M PRO MAX):

- ✅ **Ring 3 ejecuta** (`179c19b1`): CPL3→INVOKE→CPL0→EXIT→reap, scheduler
  preemptivo por LAPIC.
- ✅ **Tres programas Ring 3 a la vez**, escritos en asm, BMO C y BMO COBOL,
  compilados por el toolchain propio a BEF nativo.
- ✅ **CABINA**: observador omnisciente que GRABA (no encuesta) — los módulos
  empujan su evento en el instante del hecho, incluso antes de que exista
  framebuffer.
- ✅ **Teclado y mouse USB propios** (xHCI + HID), con distribución española,
  teclas muertas, AltGr, Ctrl, repetición al mantener, LEDs e historial.
- ✅ **El kernel lee su disco** (`49d536e3`): AHCI/SATA propio, sectores y
  tabla GPT del Kingston de 480 GB, verificado sector a sector. El disco
  estaba en el puerto 2, que el firmware declaraba inexistente.
- ✅ **FAT32 montado** (`233edc1b`): la partición de arranque se monta y se
  recorre. De sectores a ARCHIVOS. El sistema de ficheros entra por un
  **contrato de bloques** (`BlockReader`/`BlockWriter`) y no sabe si debajo
  hay SATA o NVMe — el día que haya un NVMe cableado se le pasa otra función
  y ni se entera. Montado **sin escritor**: la imposibilidad de escribir es
  estructural, no una promesa.

**Conseguido después** (2026-07-27/28):

- ✅ **La pantalla, la entrada y la consola son capabilities.** Ring 3 pinta
  con `mov` sobre el framebuffer mapeado, recibe teclas y ratón, y tiene su
  propia consola con **los dos sentidos** — un terminal puede leer lo que
  imprime su hijo y mandarle lo que se teclea.
- ✅ **XSAVE per-task, con su causa raíz.** `XSAVE` hace *merge* de la
  cabecera, no *store* (ver `BITACORA.md` Ep. 14). Los prólogos ponen a cero
  la cabecera entera y los cinco epílogos la vigilan.
- ✅ **Escritorio con terminal**: caja estilo `Win+R` con historial, TAB que
  completa listando candidatos, editor de línea con cursor y portapapeles,
  `ls`, y `Ctrl+Alt` para invocar la ventana.
- ✅ **El compositor sale del kernel**: se carga de `sys/gui.bex` en el
  volumen de datos. Cambiar el escritorio ya no obliga a recompilar Ring 0.
- ✅ **BMO COBOL lee y escribe**: `DISPLAY <variable>` formatea en ejecución
  con la escala de su PIC, y `ACCEPT` lee del terminal que lo lanzó — en un
  proceso que **no tiene** la capability del teclado y no le hace falta.
- ✅ **Calculadora con botones**: la cara en Rust dentro del compositor, el
  cálculo en BMO COBOL con decimal exacto en centavos. Windows lleva el motor
  dentro de la app; aquí es otro proceso, y mañana puede ser Ada.

**Conseguido después** (2026-07-29/31):

- ✅ **PICTURE de edición en ejecución**, con foto: `$12,345.67`, `*****0.45` y
  `  120.00CR` alineados. La cadena entera —fuente COBOL → parser → codegen →
  BEF → CPU real— produce la línea de un banco.
- ✅ **File I/O de COBOL**: `SELECT`/`FD`/`OPEN`/`READ … AT END`/`WRITE`/`CLOSE`.
  `batch.bex` lee movimientos, totaliza en centavos y escribe el cierre en el
  disco. **Y OCCURS**, con guarda de rango que para el programa en vez de leer
  memoria ajena.
- ✅ **ADA EN SILICIO** — tercer lenguaje, el mismo día que nació su compilador.
  Perfil **ZFP secuencial + Annex F**: el hallazgo es que Annex F copió el
  `PICTURE` de COBOL, así que el decimal exacto ya estaba pagado.
- ✅ **Entrada en BMO C** (`getchar`/`scanf`), y el banco de pruebas de C
  ejecuta los programas en vez de mirarlos: **185 tests**.
- ✅ **El volumen de datos por categorías** (`sys/ cobol/ c/ ada/ datos/`) y
  el contador de programas arreglado: la máquina lanzaba tres y decía "sin
  hueco" con 58 ranuras libres — miraba una bitácora histórica de 8 entradas.
- ✅ **`info` / `cpu` / `mem` desde Ring 3**: 14.8 GiB totales, **BMO-X ocupa
  5.4 MiB**, TSC medido 3.70 GHz. Dos subsyscalls y una tabla de 20 campos:
  añadir un dato es una fila.
- ✍️ **El escritorio con foco de verdad**: F12 abre la consola de datos de
  ESTRATOS, **Alt+Tab** recorre la pila MRU con su ventanita, y hay tres modos
  (`normal` / `fijo` / `sigue al puntero`). La política vive en `bmo_input::foco`
  —donde se puede PROBAR—, y el compositor sólo pinta lo que decidió. Espera
  arranque.

**Lo que sigue, en orden:**

1. **Cablear la escritura de ESTRATOS al dispositivo.** Es lo único que separa
   "un almacén que se lee" de "un almacén". La transacción ya está escrita y
   probada; falta el `write` y el `FLUSH CACHE` de verdad.
2. **Capability de memoria.** Un proceso recibe su imagen y 64 KiB de pila y
   no puede pedir más. Bloquea dos cosas a la vez: cualquier lenguaje con GC,
   y las **superficies compartidas** que hacen falta para ventanas de verdad.
3. **Write-combining del framebuffer** (PAT). Barato, y se nota en cada píxel.
4. **Ada, hacia ACATS** como matriz de conformidad — el estándar tiene su
   propio banco de pruebas y es la forma honesta de medir cuánto Ada hay.
5. **Superficies y ventanas.** Hoy `KIND_FRAMEBUFFER` es exclusivo: un solo
   proceso es dueño de la pantalla. Wayland en pequeño, sobre el punto 2.
6. **SMP al final**: el código de despertar los APs ya existe en `s1_cpu`,
   pero el día que corra un segundo núcleo cada `static mut` del kernel es
   una carrera. El trampolín es el 10%; auditar el estado compartido es el 90%.

**Y lo que este objetivo DESCARTA**, que vale tanto como lo que exige: Vulkan
y GPU (otro proyecto del tamaño de éste), Wine (treinta años de trabajo para
una compatibilidad que este objetivo no usa), y una libc completa — prometer
compatibilidad que no existe es exactamente el fallo que hundió al proyecto
anterior. Son descartes **de esta fase**, no renuncias.

## Principios

- **GOP primero**: Todo visual por framebuffer, sin drivers GPU propietarios
- **Ring 0 + Ring 3**: Sin Rings 1/2 (x86-64 moderno)
- **Serial debug**: COM1 como canal de diagnóstico primario
- **Preemptivo**: scheduler por LAPIC timer con switch real Ring 0 ↔ Ring 3 (probado en hardware)
- **Modular**: Cada subsistema es independiente (ring0, bmo_core, cabina, defense, etc.)
- **Depurable**: Diag integrado desde el primer byte


"Athos, Porthos y Aramis

Mosquetero	Rol	Frase
Athos (Cabina)	Sabiduría, experiencia	"Yo vi qué pasó."
Porthos (TimeBack)	Fuerza, acción	"Yo lo deshago."
Aramis (ByteDefender)	Fe, protección	"Yo evito que pase.""
