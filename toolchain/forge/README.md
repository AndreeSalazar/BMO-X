# forge/ — la fragua de BMO-X

Las **librerías compartidas del pipeline de compilación**, y el gate de
verificación. Aquí es donde la salida cruda de cada lenguaje se **forja** en
bytes válidos para el BMO ABI.

> **Ley (ver `../lang/cobol/cobol.md`)**: se comparten CONTRATOS y LIBRERÍAS,
> nunca CEREBROS. Nada de esto es un embudo obligatorio — cada frontend
> (`../lang/*`) **elige** qué enlazar. La esencia de cada lenguaje (parser,
> AST, su descenso propio) vive en su crate, jamás aquí.

## Lo que FUNCIONA hoy (regla: nada de stubs)

| Crate | Rol | Estado |
|-------|-----|--------|
| **`sem-asm/`** (`bmo-sem-asm`) | **Codificación**: motor que lee las tablas TOML (`tables/`) y encodea instrucciones + intrínsecos → bytes. Lo usa `lang/c/codegen.rs`. | ✅ funciona (7 tests) |
| **`bmo-verify/`** | **Gate de verificación**: valida el BEF (header, secciones, imports/relocs, firma, flags) antes del ABI. Delega en el validador REAL de `bmo-abi::bef::validator`. Habilita SIPs (Singularity). | ✅ **CABLEADO el 2026-08-02**: lo llaman los CUATRO frontends (C, COBOL, Ada, C++) **antes de escribir el fichero**. Hasta ese día existía y no lo llamaba nadie — el gate estaba escrito y abierto |
| **`bmo-lower/`** | **L1 — el descenso al ABI**: la puerta. Emite `INVOKE`/subsyscalls (`console::write_const`, `console::write_buffer`, `task::exit`). No sabe qué lenguaje la llamó. Lo usan `lang/c` y `lang/cobol`. | ✅ funciona (7 tests, incluye emulador x86-64) |

### La regla de L1 (`bmo-lower`)

> **L1 solo contiene lo expresable en la superficie congelada por valor.
> Todo lo que tenga semántica de lenguaje —formato `%d`, edición PIC,
> `operator<<`— se queda en L2 (el frontend).**

Es lo que impide que la puerta degenere en un embudo de mínimo común
denominador. `printf("%d", x)` formatea a bytes *dentro* del programa C;
`DISPLAY saldo` aplica la PIC en el suyo; ambos llaman a la misma puerta con
bytes crudos. Cuando entre un cuarto lenguaje, aquí no se toca nada.

Los tests de `bmo-lower` no comparan bytes contra bytes escritos a mano —eso
solo repite el error del autor—: **ejecutan** el código emitido en un
emulador x86-64 mínimo (`src/emu.rs`) que modela la puerta del kernel
(8 bytes LE, NUL-stop) y verifica que el texto reconstruido sea el original.
Un test compara además byte a byte con la secuencia de `tools/hello-bex`, la
única que sabemos con certeza que corrió en el Ryzen real.

---

## ⚖️ DECISIÓN ABIERTA: el enlazado — y por qué está sin decidir

> **Estado: SIN DECIDIR (2026-08-02).** Escrito aquí para que quien la tome
> —el dueño u otro— lo haga con los motivos delante y no reconstruyéndolos.
> Cuesta semanas de diferencia según el camino, y por eso no se decide de paso.

### El hecho, medido

**BMO no tiene enlazador.** El codegen de C lo dice él mismo cuando falta un
símbolo:

> *"no existe la funcion 'X' que se llama (aqui no hay enlazado: todo lo que se
> llama tiene que estar en esta unidad)"*

Y el estado de las piezas es asimétrico:

| Pieza | Estado |
|---|---|
| Tablas de **imports/exports** en BEF (`bmo-abi::bef::{imports,exports}`) | ✅ el formato lo soporta |
| `tools/bex-link` — ELF → BEX | ✅ funciona: así se construye el compositor |
| `tools/bmo-linker` — lee ELF y emite un TOML de símbolos | ◐ es un REGISTRO, no un enlazador |
| Resolución entre **unidades distintas** | ❌ no existe |

**La consecuencia práctica**: `lang/base/bmo-rt` —la libc: `crt0`, montón,
cadenas, `printf`— **no se puede usar**. No porque le falte código, sino porque
ningún `.bex` puede llamarla. Terminar `fopen` sin resolver esto sería escribir
más código muerto, que es justo lo que se limpió el 2026-08-02 borrando seis
crates huérfanos.

### Camino A — enlazador de verdad

`bmo-rt` se compila a un BEF con su tabla de exports; el frontend emite imports
y relocaciones; un paso de enlace resuelve y produce un `.bex`.

- **A favor**: es lo que hace que la libc sea *la* libc. Y desbloquea lo mismo
  para **C++ con unidades de compilación separadas**, que hoy tampoco puede.
- **En contra**: semanas. Y hay un problema real que resolver primero —
  `bmo-rt` es Rust y `bex-link` produce imágenes **ya enlazadas a base fija**
  (`0x40000000`); dos imágenes enlazadas no se concatenan. Hace falta trabajar
  con objetos reubicables, no con imágenes.

### Camino B — funciones sintetizadas

El codegen **inyecta** la función una vez en la imagen y todas las llamadas se
relocalizan a ella.

- **A favor**: el mecanismo **ya existe y ya corre en metal** —
  `__bmo_syscall_stub` se sintetiza así y funciona desde hace semanas. Y
  arregla la limitación que más duele hoy: **cada `malloc()` es un syscall y
  sólo hay cuatro por proceso**. DOOM pide un bloque grande y luego miles de
  trozos pequeños; con lo de hoy muere al quinto. `bmo-rt::heap::freelist`
  (247 líneas, probadas en el anfitrión) sería la **especificación** de lo que
  se emite.
- **En contra**: no es enlazado. Cada imagen lleva su copia de cada función que
  use, y C++ sigue sin poder separar unidades.
- **Coste**: una sesión.

### Lo que inclina la balanza, dicho para el que decida

**B no es un rodeo: la lista de libres hay que escribirla igual.** Y B desbloquea
DOOM; A no lo desbloquea antes.

El argumento del otro lado es de plazo largo: **C++ sin unidades separadas tiene
techo**, y ese techo llega el día que un programa no quepa en un fichero.

La pregunta que decide, y no es técnica: *¿qué llega antes, un programa ajeno
grande (A) o DOOM (B)?*

---

## Fases futuras (se crearán CON código real, no como stubs)

Cuando arranquen, estas librerías nacerán con lógica de verdad — no se
dejan andamios vacíos en el árbol:

- **`bmo-opt`** — Optimización genérica a dial: const-fold → DCE →
  strength reduction → register allocation lineal (el que importa para
  loops COBOL).

## Layout

```
toolchain/
  lang/           ← frontends (ESENCIA, individual): c, cobol, cpp, base
  forge/          ← ESTA carpeta: librerías compartidas del pipeline
    sem-asm/
      src/        (motor Rust que lee las tablas)
      tables/     (arch/, standards/, stdlib/ — las TOML)
    bmo-verify/   (gate: delega en bmo-abi::bef::validator)
    bmo-lower/    (L1: la puerta INVOKE — console::*, task::*)
  tools/          ← generadores: linker, bef-bootstrap, hello-bex, fontgen
```
