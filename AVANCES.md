# AVANCES — estado de BMO-X (recuperación de contexto)

> Documento vivo para retomar el proyecto desde cero (chat nuevo). Resumen de
> QUÉ funciona, QUÉ falta, DÓNDE está cada cosa y CÓMO se trabaja. Para el
> detalle cronológico ver los commits y `BITACORA.md`.

**BMO-X** = sistema operativo bare-metal en Rust: microkernel de **capabilities**
con **superficie congelada de 3 syscalls** (`INVOKE`/`CHANNEL_KICK`/`WAIT`) +
subsyscalls; arranca en **hardware real** (MSI A320M PRO MAX + Ryzen 5 5600X),
sin QEMU. Toolchain propio (C/C++/COBOL → BEF → BEX nativo).

---

## Estado global

| Componente | Estado |
|---|---|
| Boot chain (UEFI shim + s1_cpu + s2_mem) | ✅ arranca en HW real |
| Ring 0 (kernel: scheduler preemptivo, mm, caps, IPC) | ✅ estable en HW |
| Ring 3 (userspace) | ✅ ejecuta: hola-mundo CPL3→INVOKE→CPL0→EXIT |
| Fault isolation (crash R3 mata la tarea, no el kernel) | ✅ implementado |
| Boot cinemático (logo→RING0→RING3, escenas) | ✅ |
| Teclado USB (xHCI+HID) | ✅ **ESCRIBE en HW** — el Interval del endpoint era un EXPONENTE (2^n x125us) y se escribia el bInterval crudo: un teclado que pedia 24 ms quedaba programado a 35 minutos entre sondeos. Layouts es-latam/es-espana/us, teclas muertas, AltGr, Ctrl, repeticion al mantener, LEDs, historial |
| Mouse USB | ✅ enumera (falta puntero/scroll: es territorio del compositor F5) |
| **CABINA** (telemetría omnisciente) | ✅ **viva**: cockpit + color semántico + bitácora de eventos (narrador) + detección de disco PCI |
| Toolchain reorganizado (lang/forge/tools) | ✅ |
| sem-asm (encoder tabla→bytes + intrínsecos) | ✅ C lo usa; fusión sem-asm↔C hecha |
| BMO COBOL | ◐ base sólida (~15%); ver abajo |
| **BMO C ("CONTROL ABSOLUTE")** | ✅ **C esencial ~C11 muy completo** (85 tests); Fase 0/1/2 hechas — ver abajo |
| C++ frontend | ◐ mínimo (~900 líneas); será barato encima de C |
| Desktop / compositor (F5) | ⬜ pendiente — es el momento library-OS de verdad (el compositor recibe UNA VEZ la capability del framebuffer y dibuja directo) |
| **Driver de disco (AHCI/SATA)** | ✅ **BMO-X LEE SU DISCO**: sectores + tabla GPT del Kingston 480 GB, verificado sector a sector. SOLO LECTURA a proposito. El NVMe de esta maquina es el disco de **Windows** — nunca se toca |

---

## Kernel (Ultra_kernel_x86-64/)

Funciona en HW real: boot chain unificado (BOOTX64.EFI embebe s1/s2/kernel),
GDT/IDT propias, paginación (physmap 16 GiB, kernel-half pre-poblado),
scheduler preemptivo por LAPIC timer, Capability Engine, BMO Channel (IPC),
3 syscalls, fault isolation. **Bugs raíz históricos resueltos** (ver BITACORA):
CS fantasma UEFI, split-brain de gs, framebuffer bajo CR3 usuario, stacks no
contiguos.

**Teclado USB — diagnóstico HW (2026-07-24, debuggeado a fotos vía CABINA)**:
el teclado (un **numpad**) ENUMERA en slot 2, endpoint DCI 5 (control transfers
OK), pero su **endpoint de interrupción NUNCA completa una transferencia** →
`tev=1` pegado (ruido de slot 1 EP0), `kev=0`, teclas no llegan. Fixes probados:
mapeo del keypad (translate 0x47-0x53), Max ESIT Payload en el Endpoint Context
(era 0 → xHC no agendaba; necesario pero NO bastó). Hipótesis viva: el numpad es
**low/full-speed detrás de un hub interno** (el `slot 1` misterioso) → xHCI los
agenda distinto (intervalo FS/LS + TT). SIGUIENTE: probar un **teclado USB normal
en puerto trasero** (aísla numpad-vs-driver), o codificar el intervalo FS/LS.
Todo el debug vive en CABINA (fila usb: `kev/tev/hev/dci/lev` + evento FAULT).

**CABINA (ring0/cabina.rs)** — telemetría omnisciente, always-on desde el shell
loop (NO desde el timer IRQ: causaba cuelgue→reset). Da vida a `cabina-core`:
`snapshot()` desde contadores vivos + `render_hud()` pinta bitácora de 9 líneas
(eventos con severidad/capa/color) + 3 de telemetría compacta. `record()`/
`info/warn/fault` = el narrador; ring de 48 eventos. `find_storage()` en dev/pci
detecta el controlador de disco (NVMe/AHCI). Color: verde=bien, ámbar=atención,
rojo=problema. Anti-ghosting por change-detection + SCREEN_GEN.

**Pendiente kernel**: teclado→shell (endpoint de interrupción, ver arriba),
**driver de disco NVMe → CABINA caja negra en SSD**, mouse multi-device, demand
paging, XSAVE (AVX per-task), endpoint RPC (servidores Ring 3), EXIT-reclaim.

---

## Toolchain (toolchain/)

```
toolchain/
  lang/    frontends (esencia): c, cobol, cpp, base(stdlib)
  forge/   pipeline compartido: sem-asm(encoder ✅), bmo-verify(gate ✅)
  tools/   generadores: bef-bootstrap, hello-bex, fontgen, bmo-linker, cobol-gen(Python)
```

- **sem-asm** ✅: motor que lee `forge/sem-asm/tables/*.toml` y encodea
  instrucciones→bytes. C y COBOL migrados a usarlo (fuera bytes hardcodeados).
- **bmo-verify**: gate que valida el BEF (delega en `bmo-abi::bef::validator`,
  el validador real de 15 tests). `bmo-lower` (descenso ABI) y `bmo-opt`
  (optimización) se recrearán con código real al empezar su fase — no stubs.

---

## BMO C — "CONTROL ABSOLUTE" (toolchain/lang/c/) — MUY completo

C esencial de Ritchie (~C11). **85 tests verdes.** Módulos: `standard.rs`
(versiones C89..C23, tablas en forge/sem-asm), `lexer.rs`, `parser/mod.rs`,
`ast/`, `codegen.rs` (el "diccionario" → bytes exactos, sin cerebro intermedio
tipo LLVM), `module.rs`.

**Fases HECHAS (2026-07-23/24):**
- **F0 — cimientos honestos**: exterminados ~10 "silencios traicioneros" (bytes
  MAL sin avisar): offsets `a->b->c` anidados, `int **pp`, sufijos `10UL`,
  `arr[i]=x` que se descartaba, `TypeSpec::Array(elem,n)` con tamaño real,
  decls anidadas sin slot (for infinito), subscript array-vs-puntero, stores de
  campo con tamaño exacto (`pt.x` ya no pisa `pt.y`), casts reales (movsx/movzx),
  errores con LÍNEA real. Criterio: "un diccionario no adivina".
- **F1 — LA FUSIÓN sem-asm↔C**: `tables/arch/x86_64/intrinsics.toml` +
  `__hlt/__pause/__rdtsc/__outb/__inb/__wrmsr/__cpuid`. El compilador emite los
  BYTES EXACTOS de la tabla (no caja negra tipo `asm()`); agregar instrucción =
  1 entrada TOML, cero Rust.
- **F2 — completo**: punteros a función (`int (*op)(int)`, decadencia, call rax
  indirecto = base de vtables C++), subscript compuesto (`p->arr[i]` = IndexPtr),
  `(*fp)(args)` (CallPtr), **floats SSE** (ruta xmm paralela: literales, +−×÷,
  comparaciones comisd, cvtsi2sd/cvttsd2si, retorno en xmm0; float globales y
  args-de-función = deferido honesto).

**FALTA C**: float args por ABI xmm + printf %f, float globales, preprocesador
completo, stdlib impl.c. Base sólida para C++ (hereda lexer/tablas/intrínsecos/
codegen; solo pone RAII + vtables encima).

---

## BMO COBOL (toolchain/lang/cobol/)

Ver `ARCHITECTURE.md` y `cobol.md` en esa carpeta.

**HECHO** (la base, ~15%):
- **Lexer** (`lexer.rs`): Source→Tokens; `.` decimal vs terminador; usa tablas.
- **Parser de tokens** (`tparser.rs`): sentencias + DATA DIVISION + programa
  completo → AST. Camino paralelo al `parser.rs` por-líneas (aún el principal).
- **PIC propio** (`pic.rs`): 100% BMO, sin gnucobol-rs (GPL). Da la escala.
- **Decimal EXACTO** (`codegen.rs`): ADD/SUB/MUL/DIV escalan por el PIC →
  centavos sin float. **El alma bancaria de Grace Hopper.**
- **Fábrica Python** (`tools/cobol-gen/`): genera `generated/words.rs` (556
  reservadas separadas ESENCIA vs VENDOR, 55 intrínsecas). Organizada en
  `defs/{words,verbs,intrinsics,grammar}.py`.
- Pipeline end-to-end probado: Source→lexer→tparser→AST→codegen→BEF (magic BEF1).
- **32 tests verdes.**

**FALTA** (~85%, honesto):
- DATA: records anidados (grupos 01/05/10), OCCURS, REDEFINES, nivel 88/66,
  **PICTURE de edición** (`$$,$$9.99`/Z/CR/DB — motor de máscaras), COMP-3 real.
- ~44 verbos: IF/EVALUATE con condiciones, PERFORM VARYING, STRING/UNSTRING,
  INSPECT, SEARCH, CALL, File I/O (OPEN/READ/WRITE…), SORT, ACCEPT.
- Expresiones (COMPUTE con precedencia), variable+variable, subíndices.
- 55 intrínsecas (0 implementadas), runtime (bmo-rt), COPY, formato fijo/libre.
- Cablear `tparser::parse_program` como principal (jubilar `parser.rs`).

**Regla de la esencia**: "el encoder puede ser compartido; la aritmética de
COBOL jamás. El decimal es sagrado, vive solo en lang/cobol." GnuCOBOL infla a
1130+ palabras porque **traduce a C**; BMO compila **nativo** y separa esencia
de vendor. **COBOL devorado → BMO COBOL.**

---

## Filosofía / arquitectura (los principios)

1. **3 syscalls congelados + subsyscalls**: `INVOKE`/`KICK`/`WAIT` nunca
   cambian; todo lo demás son operaciones sobre capabilities (modelo seL4/Zircon,
   no Windows). Ver README raíz "Subsyscalls".
2. **Contratos y librerías, NUNCA cerebros**: se comparten formatos (BEF, ABI)
   y librerías opcionales; jamás un IR/embudo central (sería monolito).
3. **Library OS + Devour_System**: superficies ajenas (Win32, POSIX) se
   traducen a subsyscalls → nativo. El kernel no sabe que existieron.
4. **Borrar costos, no optimizarlos**: library OS borra la frontera de syscall;
   lenguajes nativos borran el impuesto del C ABI; perfil per-CPU borra el
   impuesto genérico.
5. **Python = fábrica de tablas** (dev-time), nunca entra a BMO. Genera lo
   TABULAR (~40%); la semántica/codegen es Rust (~60%).

---

## Flujo de trabajo

**Compilar el kernel + flashear a hardware:**
```bash
cd C:\Users\Salazar\Documents\BMO\Ultra_kernel_x86-64
.\build.ps1 -Flash -Drive A -Yes
bcdedit /set "{fwbootmgr}" bootsequence "{57cb1744-7f84-11f1-930d-c3a2d7ca848a}"
shutdown /r /t 5
```
(El one-shot arranca BMO-X una vez y vuelve a Windows. Si el video del firmware
falla: **apagado completo** re-inicializa el VBIOS. F11 tapado por Windows
Boot Manager primero en BootOrder.)

**Regenerar las tablas COBOL (Python):**
```bash
py toolchain/tools/cobol-gen/generate.py
```
(Python 3.13 instalado en `%LOCALAPPDATA%\Programs\Python\Python313\`.)

**Tests de los frontends:**
```bash
cargo test -p bmo-c-front       # 85 verdes (C esencial)
cargo test -p bmo-cobol-front   # 32 verdes (COBOL base)
cargo test -p bmo-sem-asm -p bmo-verify -p cabina-core
```

**Compilar solo el kernel (sin flashear) para verificar cambios:**
```bash
cd Ultra_kernel_x86-64; .\build.ps1 -BuildOnly
```
(El kernel es bare-metal; `cargo build --workspace` falla al linkear con
link.exe del host — usar build.ps1. Nota commits: mensajes con `->`/comillas/
paréntesis rompen el heredoc de PowerShell — usar `git commit -F archivo`.)

---

## Docs de referencia

- `BITACORA.md` — bitácora de guerra del debugging en HW (11 episodios).
- `README.md` (raíz) — arquitectura, Subsyscalls, boot path.
- `toolchain/lang/cobol/ARCHITECTURE.md` — pipeline COBOL completo + roadmap.
- `toolchain/lang/cobol/cobol.md` — esencia/teoría de COBOL en BMO.
- `toolchain/forge/README.md` + `toolchain/README.md` — pipeline y estructura.
- `toolchain/tools/cobol-gen/README.md` — la fábrica Python.
- `platform/abi/bmo-abi/src/ENDPOINT_RPC.md` — diseño RPC a Ring 3.

---

## Próximos frentes (prioridad)

**Kernel/HW (orden acordado 2026-07-25):**
1. **FAT32 sobre la particion 2 (A: BMO)** — el disco YA se lee por sectores y
   la GPT esta parseada; falta el sistema de ficheros para leer ARCHIVOS. El
   primero que abra sera su propio BOOTX64.EFI. Desbloquea: volcado de CABINA a
   disco (la caja negra forense) y `.bex` fuera del kernel (hoy viajan
   embebidos con include_bytes!, o sea que añadir un programa exige recompilar
   BMO-X entero).
2. **Gate de identidad antes de escribir**: `IDENTIFY` ya da modelo/serie; falta
   que sea una COMPROBACION y no una linea informativa. Sin eso, la escritura
   sigue cerrada — con el Windows del dueño en el NVMe de la misma maquina, eso
   no es paranoia.
3. **XSAVE con bandera en el BEF**: hoy FXSAVE guarda x87+SSE; la mitad alta de
   los YMM NO se preserva, asi que AVX en Ring 3 es corrupcion silenciosa. El
   plan elegido: que el programa DECLARE en su contenedor que usa vectores
   anchos, y el kernel reserve el area grande solo para esos. Contratos, no
   adivinanzas.
4. **Endpoint RPC → compositor + mouse (F5)**: el momento library-OS.
5. **BMO-FS en BMO-DATA**, con el disco ya probado y sin arriesgar el arranque.
6. **SMP al final**: el codigo de despertar los APs YA EXISTE en s1_cpu
   (trampolin, INIT+SIPI, GDT/IDT), pero `smp_startup()` no tiene ni una
   llamada y `ap_entry64` solo cuenta y hace hlt. Va el ultimo a proposito: el
   dia que corra un 2o nucleo, cada `static mut` del kernel es una carrera.

**Palancas de velocidad ARQUITECTONICAS (no micro-optimizacion):** sin cruce de
anillos (library OS), DMA directo al buffer del llamante (hoy hay pagina de
rebote), NCQ (el HBA declara 32 ranuras, se usa 1) e interrupciones MSI en vez
de sondeo.

**Sistemas de ficheros ajenos:** leer NTFS es viable HOY — el crate `ntfs` de
ColinFinck es no_std, MIT/Apache y esta pensado para firmware y drivers de
kernel. Escribirlo no: no hay nada seguro que enlazar. La decision es del dueño,
no una imposibilidad tecnica.

**Filosofía política grabada (2026-07-24)**: BMO-X = "dictadura absoluta pero
benevolente" — cero-confianza en el CÓDIGO (capabilities + bmo-verify), soberanía
del DUEÑO, transparencia total (CABINA lo confiesa todo). Trade-off honesto:
software que exige opacidad (DRM/anti-cheat de kernel) se auto-excluye. No es
piratería; es "esta máquina me obedece solo a mí". Consola-con-esteroides + PC.

**Lenguajes:**
5. **BMO C++ (esencial, ACOTADO)** — SIGUIENTE lenguaje; barato encima de C
   (hereda todo). NO es "todo C++". Alcance deliberado =
   desde Bjarne (origen) hasta lo ESENCIAL de C++17, sin la bola moderna.
   - DENTRO: clases/structs, ctor/dtor (RAII), referencias, sobrecarga,
     herencia + virtuales (vtables, ya presente), namespaces, templates
     básicos, new/delete, auto, range-for, nullptr, constexpr básico, lambdas.
   - FUERA (la "basura" que hunde el barco, cf. Stroustrup "Remember the
     Vasa!"): concepts, coroutines, modules, ranges, STL gigante,
     metaprogramación pesada, C++20/23, el treadmill moderno.
   - Los 3 syscalls + runtime mínimo (bmo-rt) lo hacen FINITO/terminable:
     no necesita std::thread/filesystem/etc. **C++ congelado en su esencia.**

**Desktop (F5)**: compositor sobre Endpoint RPC, estética Win11+Mac cyberpunk.
