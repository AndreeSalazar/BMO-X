# forge/ -- la fragua de BMO-X

Las **librerias compartidas del pipeline de compilacion**, y el gate de
verificacion. Aqui es donde la salida cruda de cada lenguaje se **forja** en
bytes validos para el BMO ABI.

> **Ley (ver `../lang/cobol/cobol.md`)**: se comparten CONTRATOS y LIBRERIAS,
> nunca CEREBROS. Nada de esto es un embudo obligatorio -- cada frontend
> (`../lang/*`) **elige** que enlazar. La esencia de cada lenguaje (parser,
> AST, su descenso propio) vive en su crate, jamas aqui.

## Lo que FUNCIONA hoy (regla: nada de stubs)

| Crate | Rol | Estado |
|-------|-----|--------|
| **`sem-asm/`** (`bmo-sem-asm`) | **Codificacion**: motor que lee las tablas TOML (`tables/`) y encodea instrucciones + intrinsecos -> bytes. Lo usa `lang/c/codegen.rs`. | ✅ funciona (7 tests) |
| **`bmo-verify/`** | **Gate de verificacion**: valida el BEF (header, secciones, imports/relocs, firma, flags) antes del ABI. Delega en el validador REAL de `bmo-abi::bef::validator`. Habilita SIPs (Singularity). | ✅ **CABLEADO el 2026-08-02**: lo llaman los CUATRO frontends (C, COBOL, Ada, C++) **antes de escribir el fichero**. Hasta ese dia existia y no lo llamaba nadie -- el gate estaba escrito y abierto |
| **`bmo-lower/`** | **L1 -- el descenso al ABI**: la puerta. Emite `INVOKE`/subsyscalls (`console::write_const`, `console::write_buffer`, `task::exit`). No sabe que lenguaje la llamo. Lo usan `lang/c` y `lang/cobol`. | ✅ funciona (7 tests, incluye emulador x86-64) |

### La regla de L1 (`bmo-lower`)

> **L1 solo contiene lo expresable en la superficie congelada por valor.
> Todo lo que tenga semantica de lenguaje --formato `%d`, edicion PIC,
> `operator<<`-- se queda en L2 (el frontend).**

Es lo que impide que la puerta degenere en un embudo de minimo comun
denominador. `printf("%d", x)` formatea a bytes *dentro* del programa C;
`DISPLAY saldo` aplica la PIC en el suyo; ambos llaman a la misma puerta con
bytes crudos. Cuando entre un cuarto lenguaje, aqui no se toca nada.

Los tests de `bmo-lower` no comparan bytes contra bytes escritos a mano --eso
solo repite el error del autor--: **ejecutan** el codigo emitido en un
emulador x86-64 minimo (`src/emu.rs`) que modela la puerta del kernel
(8 bytes LE, NUL-stop) y verifica que el texto reconstruido sea el original.
Un test compara ademas byte a byte con la secuencia de `tools/hello-bex`, la
unica que sabemos con certeza que corrio en el Ryzen real.

---

## ⚖ DECISION ABIERTA: el enlazado -- y por que esta sin decidir

> **Estado: SIN DECIDIR (2026-08-02).** Escrito aqui para que quien la tome
> --el dueno u otro-- lo haga con los motivos delante y no reconstruyendolos.
> Cuesta semanas de diferencia segun el camino, y por eso no se decide de paso.

### El hecho, medido

**BMO no tiene enlazador.** El codegen de C lo dice el mismo cuando falta un
simbolo:

> *"no existe la funcion 'X' que se llama (aqui no hay enlazado: todo lo que se
> llama tiene que estar en esta unidad)"*

Y el estado de las piezas es asimetrico:

| Pieza | Estado |
|---|---|
| Tablas de **imports/exports** en BEF (`bmo-abi::bef::{imports,exports}`) | ✅ el formato lo soporta |
| `tools/bex-link` -- ELF -> BEX | ✅ funciona: asi se construye el compositor |
| `tools/bmo-linker` -- lee ELF y emite un TOML de simbolos | ◐ es un REGISTRO, no un enlazador |
| Resolucion entre **unidades distintas** | ❌ no existe |

**La consecuencia practica**: `lang/base/bmo-rt` --la libc: `crt0`, monton,
cadenas, `printf`-- **no se puede usar**. No porque le falte codigo, sino porque
ningun `.bex` puede llamarla. Terminar `fopen` sin resolver esto seria escribir
mas codigo muerto, que es justo lo que se limpio el 2026-08-02 borrando seis
crates huerfanos.

### Camino A -- enlazador de verdad

`bmo-rt` se compila a un BEF con su tabla de exports; el frontend emite imports
y relocaciones; un paso de enlace resuelve y produce un `.bex`.

- **A favor**: es lo que hace que la libc sea *la* libc. Y desbloquea lo mismo
  para **C++ con unidades de compilacion separadas**, que hoy tampoco puede.
- **En contra**: semanas. Y hay un problema real que resolver primero --
  `bmo-rt` es Rust y `bex-link` produce imagenes **ya enlazadas a base fija**
  (`0x40000000`); dos imagenes enlazadas no se concatenan. Hace falta trabajar
  con objetos reubicables, no con imagenes.

### Camino B -- funciones sintetizadas

El codegen **inyecta** la funcion una vez en la imagen y todas las llamadas se
relocalizan a ella.

- **A favor**: el mecanismo **ya existe y ya corre en metal** --
  `__bmo_syscall_stub` se sintetiza asi y funciona desde hace semanas. Y
  arregla la limitacion que mas duele hoy: **cada `malloc()` es un syscall y
  solo hay cuatro por proceso**. DOOM pide un bloque grande y luego miles de
  trozos pequenos; con lo de hoy muere al quinto. `bmo-rt::heap::freelist`
  (247 lineas, probadas en el anfitrion) seria la **especificacion** de lo que
  se emite.
- **En contra**: no es enlazado. Cada imagen lleva su copia de cada funcion que
  use, y C++ sigue sin poder separar unidades.
- **Coste**: una sesion.

### Lo que inclina la balanza, dicho para el que decida

**B no es un rodeo: la lista de libres hay que escribirla igual.** Y B desbloquea
DOOM; A no lo desbloquea antes.

El argumento del otro lado es de plazo largo: **C++ sin unidades separadas tiene
techo**, y ese techo llega el dia que un programa no quepa en un fichero.

La pregunta que decide, y no es tecnica: *que llega antes, un programa ajeno
grande (A) o DOOM (B)?*

---

## Fases futuras (se crearan CON codigo real, no como stubs)

Cuando arranquen, estas librerias naceran con logica de verdad -- no se
dejan andamios vacios en el arbol:

- **`bmo-opt`** -- Optimizacion generica a dial: const-fold -> DCE ->
  strength reduction -> register allocation lineal (el que importa para
  loops COBOL).

## Layout

```
toolchain/
  lang/           <- frontends (ESENCIA, individual): c, cobol, cpp, base
  forge/          <- ESTA carpeta: librerias compartidas del pipeline
    sem-asm/
      src/        (motor Rust que lee las tablas)
      tables/     (arch/, standards/, stdlib/ -- las TOML)
    bmo-verify/   (gate: delega en bmo-abi::bef::validator)
    bmo-lower/    (L1: la puerta INVOKE -- console::*, task::*)
  tools/          <- generadores: linker, bef-bootstrap, hello-bex, fontgen
```
