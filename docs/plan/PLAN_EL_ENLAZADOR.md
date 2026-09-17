# PLAN EL ENLAZADOR -- la pieza que madura a CINCO lenguajes a la vez

> Peticion del dueno, **2026-09-17**: *"los lenguajes de programacion como C,
> C++ y eso, TODO menos Rust [...] TODO tipo de AOT y madurar por completo"*.
>
> **Lo que afirma**: que dos ficheros fuente --de C, o uno de C y otro de COBOL--
> producen UN `.bex` que corre, sin pegarlos a mano.
>
> **Como se cae**: el `.bex` enlazado no pasa `bmo-verify`, o pasa y una llamada
> entre unidades salta a una direccion que no es la de la funcion.

---

## 0. Por que este plan y no "madurar cada lenguaje"

Se midio donde se atasca cada frontend, y **cinco planes distintos citan la
misma pieza como bloqueante sin que ninguno sea su dueno**:

| quien | lo que no puede hacer hoy | donde esta escrito |
|---|---|---|
| **C** | una libc que sea LA libc: hoy las cabeceras traen el cuerpo "a la fuerza" | `toolchain/forge/README.md` |
| **C++** | revivir: es su condicion 2 | `toolchain/lang/cpp/APARCADO.md`, seccion 4 |
| **COBOL** | `CALL`, `LINKAGE SECTION`, el batch | `toolchain/lang/cobol/PLAN_BANCA.md`, 6.1-6.3 |
| **Ada** | `package` con especificacion y cuerpo en ficheros distintos | `toolchain/lang/ada/PLAN_ADA.md` |
| **ports** | SQLite cabe (es una amalgama); OpenTTD, ScummVM, DOSBox no | `docs/identidad/QUE_DESBLOQUEA.md`, palanca 2 |

*** Madurar los cinco "por completo" uno a uno choca cinco veces contra la
misma pared. **La pared es una sola, y es un contrato de FORMATO** --la regla 2
de la casa: *contratos y formatos, nunca cerebros*--, no un IR ni un optimizador
compartido. Cada frontend sigue siendo entero y suyo; lo unico nuevo es que
puede escribir un OBJETO en vez de una imagen.

---

## 1. La lista de lenguajes, cerrada -- y por que no es "todo AOT"

La barrida de AOT ya se hizo, el **2026-07-30**, y el criterio es el de
`toolchain/lang/PROPOSITO.md`: un lenguaje entra por **para que existe**, no por
existir.

```text
   DENTRO     C          el byte donde tu dices
              COBOL      el decimal que no se redondea
              Ada        el fallo que se caza antes de correr
              INTI       el de BMO-X: sintaxis de Python, control de ASM, sin UB
              C++        APARCADO con dos condiciones escritas
   QUITADO    Python AOT 2026-09-17, decision del dueno: es INTI
                         (`docs/maestro/PYTHON_MAESTRO.md`, seccion 4b)
   APARTE     PL/I, Modula-2    proyectos aparte, no fases
   FUERA      GraalVM, NativeAOT, TinyGo, LDC, Zig, Nim, Crystal, Free Pascal
              -- traen runtime, GC o libc: sirven para LEER su arquitectura
              Fortran, RPG -- no aportan a banca o no tienen oraculo libre
```

[!] Recordatorio pedido por el propio dueno (regla 7): *"TODO tipo de AOT"*
contradice su regla 0 si se lee como "mas lenguajes". Leido como **"que los que
hay maduren por completo"**, es exactamente este plan.

---

## 2. Lo que YA esta, medido el 2026-09-17

```text
   el FORMATO de objeto, casi entero
      bef/symbols.rs       161   Local/Global/Weak, Function/Object/Section
      bef/relocations.rs   198   Abs64, Rel32, SeccionAbs64 con `symbol_idx`
      bef/imports.rs        91
      bef/exports.rs       115
   BMO C ya EMITE la seccion Symbols   `toolchain/lang/c/src/tests/simbolos.rs`
   el cargador ya APLICA relocaciones  C5 de PLAN_SEGURIDAD, 25-08
   bmo-verify                          lo llaman los frontends antes de escribir
```

Y lo que NO es lo que parece:

```text
   tools/bmo-linker      un REGISTRO de simbolos ELF, fuera del workspace
   bef/linker/           308 lineas de enlazador DINAMICO que no llama nadie,
                         contra la decision escrita "todo estatico"
                         (PLAN_LA_DEUDA, D1e)
   tools/bex-link        ELF de Rust -> imagen YA enlazada a base fija: dos de
                         esas no se concatenan
```

### El Camino B del 02-08 ya se gasto

`toolchain/forge/README.md` dejo la decision abierta entre **A** (enlazador de
verdad, semanas) y **B** (funciones sintetizadas, una sesion), con la pregunta
*"que llega antes, un programa ajeno grande (A) o DOOM (B)?"*.

**DOOM llego** -- se juega en el Ryzen desde el 12-09, en una sola unidad y con
`<bmo/monton.h>` resolviendo el `malloc`. O sea que el argumento a favor de B ya
cobro lo que tenia que cobrar. **Lo que queda en la mesa es A**, y la decision
sigue siendo del dueno: este plan escribe los escalones para cuando la tome.

---

## 3. Los escalones -- lo que no toca nada va primero

- [ ] **E0 -- LA DECISION.** Estatico solamente, o tambien dinamico. La
      recomendacion es **estatico**, por lo ya escrito (`PYTHON_MAESTRO.md`
      seccion 6: *"no hay enlazado dinamico y es una decision"*) y porque un
      `.bex` firmado tiene que llevar dentro todo lo que ejecuta. Si se decide
      estatico, `platform/abi/bmo-abi/src/bef/linker/` se borra con epitafio.

- [ ] **E1 -- EL CONTRATO DEL OBJETO, en papel antes que en codigo.** Que
      distingue un `.bo` (objeto) de un `.bex` (imagen): una bandera en la
      cabecera BEF, simbolos INDEFINIDOS (`section_idx` sin seccion), y
      relocaciones contra simbolo en vez de contra seccion. Va en
      `VALKYRIE-ABI/` o junto a `bef/header.rs`, y pide su numero de version.
      **Como se sabe**: `bmo-verify` rechaza un `.bo` como ejecutable, con motivo.

- [ ] **E2 -- BMO C escribe un objeto.** `toolchain/lang/c`: una orden `-c` que
      emite `.bo` con sus globales como simbolos y cada llamada a una funcion que
      no esta en la unidad como relocacion `Rel32` contra un simbolo indefinido
      -- donde hoy dice *"aqui no hay enlazado: todo lo que se llama tiene que
      estar en esta unidad"*. **Como se sabe**: un banco en el anfitrion que lee el
      `.bo` y encuentra el simbolo indefinido y su relocacion.

- [ ] **E3 -- `toolchain/tools/bmo-enlazar`.** N objetos -> un `.bex`: junta
      secciones, resuelve simbolos, aplica relocaciones y llama a `bmo-verify`
      ANTES de escribir. Y dice que NO con nombre: simbolo definido dos veces,
      simbolo que nadie define, relocacion que no cabe. Sin `Weak` al principio
      (una promesa menos que cumplir).
      **Como se sabe**: `dos.c` + `uno.c` -> `.bex` que corre en el emulador con la
      salida EXACTA, y el mismo `.bex` sale igual byte a byte en dos corridas.

- [ ] **E4 -- EL METAL.** Ese `.bex` en el Ryzen. **Es la condicion 2 de C++
      cumplida**, y se dice en `toolchain/lang/cpp/APARCADO.md` ese mismo dia.

- [ ] **E5 -- la libc como biblioteca.** Las cabeceras de
      `toolchain/forge/sem-asm/tables/standards/C/` dejan de traer el cuerpo: se
      compilan UNA vez a `libc.bo` y se enlazan. **Como se cae**: un `.bex` crece
      en vez de encoger -- entonces el enlazador no tira lo que no se usa, y eso
      es E5b, no un detalle.

- [ ] **E6 -- COBOL `CALL` estatico.** `toolchain/lang/cobol/PLAN_BANCA.md`, 6.2 y
      6.3, sobre E3. Aqui se prueba que el contrato es de FORMATO: un `.bo` de
      COBOL y uno de C en el mismo `.bex`, cada uno con su convencion de llamada
      declarada y ninguno sabiendo del otro.

- [ ] **E7 -- Ada `package` en dos ficheros.** `toolchain/lang/ada/PLAN_ADA.md`,
      escalon A6. El orden de elaboracion (RM 10.2.1) lo decide el enlazador por
      las dependencias `with`, y un ciclo es un error con los dos nombres.

- [ ] **E8 -- INTI decide.** INTI compila hoy un fichero con sus modulos dentro;
      si quiere objetos, los pide aqui. No es obligatorio: el formato es
      opcional para quien no lo necesite (regla 2).

---

## 4. Lo que seria un error

- **Un IR comun "para que el enlazador lo tenga facil".** Es el cerebro que la
  regla 2 prohibe. El enlazador lee BEF, y nada mas.
- **Empezar por el dinamico** porque `bef/linker/` ya existe. Existe y no lo
  llama nadie, y va contra una decision tomada.
- **E5 antes que E4.** Mover la libc a una biblioteca sin un enlace probado en
  metal es apilar sobre un camino que nadie ha visto funcionar -- la misma frase
  con la que se aparco C++.
- **Contar esto como "madurar C++".** C++ sigue aparcado hasta que su condicion
  1 (SSE ejecutado en el emulador) se compruebe tambien. Nota del 17-09: el
  emulador ya decodifica `movsd` (`toolchain/forge/bmo-lower/src/emu/mod.rs`), asi
  que la condicion 1 **puede** estar cumplida sin que nadie lo haya dicho -- se
  comprueba, no se supone.

---

Ver [`PLAN_LA_DEUDA.md`](PLAN_LA_DEUDA.md) (D1e, el enlazador dinamico muerto),
[`QUE_DESBLOQUEA.md`](../identidad/QUE_DESBLOQUEA.md) (por que la palanca 2 va
antes que C++) y [`toolchain/lang/PROPOSITO.md`](../../toolchain/lang/PROPOSITO.md)
(la prueba de fuego de cada lenguaje).
