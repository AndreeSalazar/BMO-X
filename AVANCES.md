# AVANCES — estado de BMO-X (recuperación de contexto)

> Documento vivo para retomar el proyecto desde cero (chat nuevo). Resumen de
> QUÉ funciona, QUÉ falta, DÓNDE está cada cosa y CÓMO se trabaja. Para el
> detalle cronológico ver los commits y `BITACORA.md`.

**BMO-X** = sistema operativo bare-metal en Rust: microkernel de **capabilities**
con **superficie congelada de 3 syscalls** (`INVOKE`/`CHANNEL_KICK`/`WAIT`) +
subsyscalls; arranca en **hardware real** (MSI A320M PRO MAX + Ryzen 5 5600X),
sin QEMU. Toolchain propio (C/C++/COBOL → BEF → BEX nativo).

---

## Cómo leer este documento

Hay **tres estados**, y confundirlos es lo que hace que uno se sienta perdido:

- ✅ **corre en metal** — se ha visto funcionar en el Ryzen, con foto o con
  línea de CABINA. Es lo único que cuenta como hecho.
- ✍️ **escrito sin estrenar** — compila, enlaza, `bex-link` verifica sus
  direcciones… y ningún CPU ha ejecutado una sola de sus instrucciones. **No es
  lo mismo que hecho.** Es exactamente la clase de cosa que en otros proyectos
  acaba existiendo sólo en la documentación.
- ⬜ **diseño** — pensado o escrito en un documento, sin código vivo.

---

## Estado global

| Componente | Estado |
|---|---|
| Boot chain (UEFI shim + s1_cpu + s2_mem) | ✅ arranca en HW real |
| Ring 0 (kernel: scheduler preemptivo, mm, caps, IPC) | ✅ estable en HW |
| Ring 3 (userspace) | ✅ varios procesos, cada uno con su espacio y sus caps |
| Fault isolation (crash R3 mata la tarea, no el kernel) | ✅ implementado |
| Boot cinemático (logo→RING0→RING3, escenas) | ✅ |
| Teclado USB (xHCI+HID) | ✅ **ESCRIBE en HW** — el Interval del endpoint era un EXPONENTE (2^n x125us) y se escribia el bInterval crudo: un teclado que pedia 24 ms quedaba programado a 35 minutos entre sondeos. Layouts es-latam/es-espana/us, teclas muertas, AltGr, Ctrl, repeticion al mantener, LEDs, historial |
| Mouse USB | ✅ enumera y entrega puntero+botones por `KIND_INPUT` |
| **CABINA** (telemetría omnisciente) | ✅ **viva**: cockpit + color semántico + bitácora de eventos (narrador) + detección de disco PCI |
| **`KIND_FRAMEBUFFER`** (la pantalla es una capability) | ✅ Ring 3 pinta con `mov`; el kernel contesta 4 preguntas y se aparta |
| **`KIND_INPUT`** (ratón, teclado **y modificadores**) | ✅ en metal; `Ctrl+Alt` detectado sin romper `AltGr` |
| **Compositor** (Ultra_userspace/services/gui) | ✅ **se carga de `apps/gui.bex`**, fuera del kernel |
| **Terminal de Ring 3** (caja Win+R + comandos) | ✅ **corre**: historial, TAB que completa, editor de línea con cursor, portapapeles, `ls`, `Ctrl+Alt` para invocar |
| **`KIND_CONSOLE`** (la salida es una capability, en LOS DOS sentidos) | ✅ el hijo escribe y el terminal lee; el terminal escribe y el hijo lee (`ACCEPT`) |
| **`KIND_DIRECTORIO`** (preguntar qué hay en el disco) | ✅ `ls` en el terminal, iteración sin cursor en el driver |
| **Calculadora con botones** | ✅ cara en Rust, cálculo en BMO COBOL |
| **`ring0/lanzar.rs`** (buscar+firma+admitir, un solo camino) | ✅ lo usa `run` en metal |
| **ESTRATOS** | ✅ montado, superbloque leído, **firma verificada antes de ejecutar** |
| Toolchain reorganizado (lang/forge/tools) | ✅ |
| sem-asm (encoder tabla→bytes + intrínsecos) | ✅ C lo usa; fusión sem-asm↔C hecha |
| BMO COBOL | ◐ base sólida pero **corre en metal**, y ya LEE y ESCRIBE: `DISPLAY <var>` + `ACCEPT`. 66 tests |
| **BMO C ("CONTROL ABSOLUTE")** | ✅ **C esencial ~C11 muy completo** (85 tests); corre en metal |
| C++ frontend | ◐ mínimo (~900 líneas); será barato encima de C |
| **Driver de disco (AHCI/SATA)** | ✅ **LEE Y MONTA**: GPT + FAT32 + volumen de datos con escritor. El NVMe de esta maquina es el disco de **Windows** — nunca se toca |
| **XSAVE per-task** | ✅ **resuelto y confirmado en metal** (ver abajo: la causa raíz) |

---

## Lo que corre en metal, verificado (arranque del 2026-07-27)

Esto no es una lista de intenciones — cada línea salió en pantalla o en CABINA:

- Arranque completo **sin pantalla azul**, shell vivo, 54 eventos en CABINA.
- `fs: volumen de datos montado para ESCRITURA` · `estratos: volumen montado y
  es de este disco` · superbloque generación 1.
- `sched: primer switch a CPL3` · `ring3: primer CONSOLE_WRITE` · cuatro
  procesos Ring 3 terminando **por su cuenta** (`EXIT`).
- `usb: primera tecla recibida: el teclado ESCRIBE`.
- **`run apps/COBOL.bex` desde ESTRATOS con la firma verificada** → `tid 7`.
  Y el programa imprimió `3 x 19.99 = 59.97 exacto`: **decimal exacto de COBOL,
  compilado por el toolchain propio, corriendo sobre el kernel propio, en un
  Ryzen de verdad.**
- La tabla `bex` con `asm`, `C`, `COBOL`, `srv`, `cli` y `COBOL.b` — y
  `leeme.t` marcado **RECHAZADO**: la admisión BEX rechaza lo que no es un
  programa en vez de saltar al vacío.

## Lo que está escrito y NUNCA ha corrido

Honestidad primero: esto es lo que hay que estrenar antes de construir encima.

- **La calculadora con botones** (`calc` en el terminal). El motor
  `apps/calcgui.bex` compila y el panel dibuja, pero nadie ha pulsado `=` en
  metal todavía.
- **`ACCEPT` de COBOL** — el canal de entrada de la consola está escrito y
  probado en el emulador (ida y vuelta: lo que se escribe se vuelve a leer),
  pero no se ha tecleado un importe en el Ryzen.
- **El editor de línea completo** (cursor, `Supr`, `Ctrl+C`/`V`) y el
  historial con flechas.

Lo que SÍ se estrenó: el terminal dibujando, la fuente en Ring 3, `tecla()`,
`OP_EJECUTAR`, el compositor desde disco, y `KIND_CONSOLE` — la foto del
`ls` y de los ecos lo demuestra.

---

## Kernel (Ultra_kernel_x86-64/)

Funciona en HW real: boot chain unificado (BOOTX64.EFI embebe s1/s2/kernel),
GDT/IDT propias, paginación (physmap 16 GiB, kernel-half pre-poblado),
scheduler preemptivo por LAPIC timer, Capability Engine, BMO Channel (IPC),
3 syscalls, fault isolation. **Bugs raíz históricos resueltos** (ver BITACORA):
CS fantasma UEFI, split-brain de gs, framebuffer bajo CR3 usuario, stacks no
contiguos.

**Teclado USB — RESUELTO.** El `Interval` del Endpoint Context de xHCI es un
**exponente** (2^n × 125 µs) y se escribía el `bInterval` crudo del descriptor,
que en Low/Full Speed viene en **milisegundos**: un teclado que pedía 24 ms
quedaba programado a **35 minutos** entre sondeos. Hoy `usb: primera tecla
recibida` sale en CABINA en cada arranque. El debug vive en la fila `usb`
(`kev/tev/hev/dci/lev`).

**XSAVE — la causa raíz (2026-07-27, cinco sondas y cuatro pantallas azules).**
`XSAVE` **no inicializa la cabecera XSAVE: hace MERGE.**

```text
XSTATE_BV <- (XSTATE_BV_viejo AND NOT RFBM) OR (XINUSE AND RFBM)
```

con `RFBM = EDX:EAX AND XCR0`, y **no toca** los 48 bytes reservados. Los stubs
tallan su área sobre la pila (`sub`+`and`), o sea sobre basura, y esa basura
sobrevivía al guardado en los bits altos → `XRSTOR` la rechaza con `#GP(0)`.
`trap::fabricate` nunca lo sufrió porque pone a cero los 1024 bytes antes de
nada; los stubs no. **Ésa era la asimetría.** Arreglo: los prólogos ponen a cero
la cabecera **entera** (512..575) antes del `xsave64`.

*La firma que lo delató*: los volcados daban `0x5F0FCB` y `0x37B`, y los dos son
**el valor viejo con los tres bits bajos puestos a 3** — que es exactamente
`XINUSE & 7`. Un campo corrupto con unos pocos bits bajos coherentes no es
corrupción: es una instrucción haciendo merge donde creíamos store.

*Defensas que quedan puestas*: guardia de cabecera en los cinco epílogos
(motivo `PODRIDO_CABECERA`), anillo de las últimas áreas publicadas
(`pub0..pub3`) con su tid, y las sondas `bv0`/`bvX`/`baseX`. El informe de fallo
es el único depurador que hay en esta máquina — por eso se quedan.

**CABINA (ring0/cabina.rs)** — telemetría omnisciente, always-on desde el shell
loop (NO desde el timer IRQ: causaba cuelgue→reset). Da vida a `cabina-core`:
`snapshot()` desde contadores vivos + `render_hud()` pinta bitácora de 9 líneas
(eventos con severidad/capa/color) + 3 de telemetría compacta. `record()`/
`info/warn/fault` = el narrador; ring de 48 eventos. `find_storage()` en dev/pci
detecta el controlador de disco (NVMe/AHCI). Color: verde=bien, ámbar=atención,
rojo=problema. Anti-ghosting por change-detection + SCREEN_GEN.

**Pendiente kernel**: capability de **memoria** — un proceso recibe su imagen
y 64 KiB de pila y no puede pedir más. Bloquea DOS cosas a la vez: cualquier
lenguaje con GC, y las **superficies compartidas** que hacen falta para
ventanas de verdad (hoy `KIND_FRAMEBUFFER` es exclusivo, un solo proceso es
dueño de la pantalla). Después: CABINA caja negra en disco, demand paging,
endpoint RPC (servidores Ring 3), EXIT-reclaim, SMP.

**Hecho desde entonces**: `KIND_DIRECTORIO` (hay `ls`), modificadores en
`INPUT_OP_MODIFICADORES` (hay `Ctrl+Alt`), `KIND_CONSOLE` en los dos sentidos
(hay `ACCEPT`).

**Deuda visible**: `services/input` es una carpeta que promete un multiplexor de
entrada y está vacía — la entrada la reclama el compositor directamente. O se
cablea o se borra, como se borró `apps/terminal`. Y el **manifest BEF**
(`provides`/`requires`, en `platform/abi/bmo-abi/src/bef/manifest.rs`) tiene
struct y parser TOML completos, y **el kernel no compila `bmo-abi`**: `build.ps1`
lo lee como TEXTO para el drift guard y nada más. Es el prerequisito si algún día
se quiere clasificar programas por lo que le PIDEN al kernel (AOT / GC / GIL).

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

**FALTA C** (por orden de lo que más duele):

1. **ENTRADA. No puede leer NADA** — ni `scanf` ni `getchar`. Tiene `printf` y
   106 tests verdes, o sea que habla y no escucha. Es exactamente el hueco que
   COBOL tenía hasta el 2026-07-28, y ahora es barato: `console::read_line` y
   `fmt::parse_decimal_scaled` ya existen en `bmo-lower` y **no son de ningún
   lenguaje** — se comparten igual que el conversor de enteros.
2. `printf %f` y float args por ABI xmm; float globales.
3. Preprocesador completo.
4. **stdlib (`impl.c`)** — y ésta es la de verdad: *la universalidad de C no
   viene del lenguaje, viene de libc*. Sin biblioteca estándar, C es un
   ensamblador portable con llaves. Es lo que `bmo-rt` tiene que llegar a ser.

Base sólida para C++ (hereda lexer/tablas/intrínsecos/codegen; solo pone RAII
+ vtables encima).

---

## BMO COBOL (toolchain/lang/cobol/)

Ver `ARCHITECTURE.md` y `cobol.md` en esa carpeta.

> **Aquí no se pone un porcentaje, y es a propósito.** "COBOL al 15%" da a
> entender que existe un 100% — un denominador. No existe: el estándar sigue
> creciendo y ningún compilador del mundo lo implementa entero. Medirse contra
> un infinito no informa de nada y sólo sirve para sentirse pequeño. Lo que sí
> se puede afirmar y comprobar es **qué corre**, y cada línea de abajo tiene su
> fila en la matriz de conformidad, que EJECUTA lo que dice soportar.

**CORRE** (verificado ejecutando, no leyendo bytes):
- **Lexer** (`lexer.rs`): Source→Tokens; `.` decimal vs terminador; usa tablas.
- **Parser de tokens** (`tparser.rs`): sentencias + DATA DIVISION + programa
  completo → AST. Camino paralelo al `parser.rs` por-líneas (aún el principal).
- **PIC propio** (`pic.rs`): 100% BMO, sin gnucobol-rs (GPL). Da la escala.
- **Decimal EXACTO** (`codegen.rs`): ADD/SUB/MUL/DIV escalan por el PIC →
  centavos sin float. **El alma bancaria de Grace Hopper.** Confirmado en el
  Ryzen: `3 × 19.99 = 59.97`.
- **Flujo de control real**: IF/ELSE anidado y con AND, PERFORM TIMES,
  PERFORM UNTIL, COMPUTE con precedencia y paréntesis.
- **DISPLAY** de literal y de variable, **ACCEPT** por el anillo de entrada
  de la consola.
- **PICTURE de edición EN EJECUCIÓN** (`edicion.rs`): `$$$,$$9.99`,
  `**,**9.99`, `Z,ZZ9.99CR`, `DB`, signos fijos y flotantes, `99/99/99`.
  El recorrido de la plantilla se emite como INSTRUCCIONES: en el `.bex` no
  queda ni la máscara ni un intérprete que la lea. Atado a `formatear` por
  238 casos ejecutados en el emulador. Ver `examples/extracto.cob`.
- **Fábrica Python** (`tools/cobol-gen/`): genera `generated/words.rs` (556
  reservadas separadas ESENCIA vs VENDOR, 55 intrínsecas). Organizada en
  `defs/{words,verbs,intrinsics,grammar}.py`.
- Pipeline end-to-end probado: Source→lexer→tparser→AST→codegen→BEF (magic BEF1).
- **71 tests verdes.**

**NO CORRE** (y se dice, en vez de fingirlo):
- **File I/O** (`SELECT`/`FD`/`OPEN`/`READ`/`WRITE`/`CLOSE`) — se RECHAZA con
  su motivo en vez de compilar un READ que no lee. **El siguiente grande**: sin
  ficheros no hay batch, y debajo ya están el disco, FAT32 y el gate.
- DATA: records anidados (grupos 01/05/10), OCCURS, REDEFINES, nivel 88/66,
  COMP-3 real.
- Verbos: EVALUATE, PERFORM VARYING, STRING/UNSTRING, INSPECT, SEARCH, CALL,
  SORT.
- Subíndices, 55 intrínsecas (0 implementadas), runtime (bmo-rt), COPY,
  formato fijo/libre.
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

**Copiar los programas de Ring 3 al volumen de datos (BMO-DATA):**
```bash
cd Ultra_kernel_x86-64; .\build.ps1 -Data E
```
El `.bex` del compositor sale a `staging\BMO-DATA\apps\` en cada build y de ahí
se copia. `RUTA_COMPOSITOR` en `phase.rs` es `apps/gui.bex` (8.3: el driver FAT32 no lee nombres largos y no recorta) — la ruta de
dentro del volumen es el contrato entre el build y el arranque.
Tres cierres antes de escribir un byte: **nunca el disco del sistema**, tiene que
ser FAT/FAT32, y hay que teclear `DATA <letra> BMO`. Es el ÚNICO sitio del build
que escribe fuera del árbol del proyecto. `-Flash` es aparte y es para Ring 0:
las dos banderas tocan discos distintos a propósito.

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

**HECHO desde el 2026-07-25** (estaban aquí y ya no): FAT32 + volumen de datos
montado, gate de identidad antes de escribir, XSAVE per-task (y su causa raíz),
`.bex` fuera del kernel (el compositor se carga de disco), ESTRATOS montado con
gate de firma.

**HECHO desde entonces** (2026-07-28): la caja estrenada, el terminal con
comandos e historial, modificadores (`Ctrl+Alt`), `KIND_DIRECTORIO` (`ls`),
`KIND_CONSOLE` en los dos sentidos, `DISPLAY <var>` y `ACCEPT` en COBOL, y la
calculadora.

**Kernel/HW (orden vigente 2026-07-28):**
1. **PICTURE de edición en EJECUCIÓN.** El motor está probado
   (`toolchain/lang/cobol/src/edicion.rs`, 13 tests) pero formatea un valor que
   el compilador conoce; para `MOVE X TO Y` con `Y` editada hay que emitir el
   recorrido de la plantilla como código. Es *la* función bancaria.
2. **Entrada en BMO C** — tiene `printf` y no puede leer nada. Barato ahora:
   `console::read_line` y `fmt::parse_decimal_scaled` ya existen en `bmo-lower`.
3. **Capability de MEMORIA.** Un proceso recibe su imagen y 64 KiB de pila.
   Bloquea DOS cosas: lenguajes con GC, y superficies compartidas.
4. **Superficies y ventanas** — hoy `KIND_FRAMEBUFFER` es exclusivo. Wayland
   en pequeño, encima del punto 3. Es lo que saca la calculadora del
   compositor a su propia ventana **sin tocar el COBOL**.
5. **Endpoint RPC → servicios Ring 3**: el momento library-OS.
6. **Ada**, en perfil restringido.
7. **SMP al final**: el codigo de despertar los APs YA EXISTE en s1_cpu
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
