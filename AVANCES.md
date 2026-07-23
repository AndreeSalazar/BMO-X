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
| Teclado USB (xHCI+HID) | ◐ **enumera** (kbd ready); falta que las teclas lleguen al shell |
| Mouse USB | ◐ enumera; eventos sin cablear (esperan compositor) |
| Toolchain reorganizado (lang/forge/tools) | ✅ |
| sem-asm (encoder tabla→bytes) | ✅ C y COBOL lo usan |
| BMO COBOL | ◐ base sólida (~15%); ver abajo |
| BMO C ("CONTROL ABSOLUTE") | ◐ el más desarrollado (~4500 líneas) |
| C++ frontend | ◐ mínimo (~900 líneas) |
| Desktop / compositor (F5) | ⬜ pendiente |

---

## Kernel (Ultra_kernel_x86-64/)

Funciona en HW real: boot chain unificado (BOOTX64.EFI embebe s1/s2/kernel),
GDT/IDT propias, paginación (physmap 16 GiB, kernel-half pre-poblado),
scheduler preemptivo por LAPIC timer, Capability Engine, BMO Channel (IPC),
3 syscalls, fault isolation. **Bugs raíz históricos resueltos** (ver BITACORA):
CS fantasma UEFI, split-brain de gs, framebuffer bajo CR3 usuario, stacks no
contiguos.

**Pendiente kernel**: teclado→shell (el USB enumera, falta cablear el stream
del endpoint de interrupción a `shell_read_line`), demand paging, XSAVE
(AVX per-task), endpoint RPC (servidores Ring 3), EXIT-reclaim.

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

## BMO COBOL (toolchain/lang/cobol/) — el foco reciente

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

**Tests del frontend COBOL:**
```bash
cargo test -p bmo-cobol-front
```

---

## Docs de referencia

- `BITACORA.md` — bitácora de guerra del debugging en HW (9 episodios).
- `README.md` (raíz) — arquitectura, Subsyscalls, boot path.
- `toolchain/lang/cobol/ARCHITECTURE.md` — pipeline COBOL completo + roadmap.
- `toolchain/lang/cobol/cobol.md` — esencia/teoría de COBOL en BMO.
- `toolchain/forge/README.md` + `toolchain/README.md` — pipeline y estructura.
- `toolchain/tools/cobol-gen/README.md` — la fábrica Python.
- `platform/abi/bmo-abi/src/ENDPOINT_RPC.md` — diseño RPC a Ring 3.

---

## Próximos frentes (prioridad)

**Kernel/HW:**
1. Teclado→shell (cablear el stream USB HID al input) — desbloquea interacción.
2. Fault isolation: probar con un payload crasher.

**Lenguajes (C es el MÁS factible; ver nota):**
3. **BMO C — "CONTROL ABSOLUTE"** (bautizo oficial): el mismo C de Ritchie
   (esencia hasta C11, sin deriva posterior), devuelto a su hábitat natural:
   escribir el sistema con privilegio directo al metal. En BMO-X nada se
   interpone entre C y el hardware — C *es* la capa. El más desarrollado y
   de menor espec; el camino pragmático a un lenguaje *usable* de verdad
   (systems + drivers). Nota: el privilegio es del código sobre el hardware;
   la *autoridad* siempre la acotan las capabilities (3 syscalls congelados).
4. **COBOL**: la base está; crecer por features (records+OCCURS → IF/EVALUATE →
   PERFORM VARYING → COMPUTE → edit-masks → File I/O).
5. **BMO C++ (esencial, ACOTADO)**: NO es "todo C++". Alcance deliberado =
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
