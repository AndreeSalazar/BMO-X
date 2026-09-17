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
   el enlazador dinamico 308 lineas en bmo-abi que no llamaba nadie -- BORRADO
                         el 2026-09-17 (E0). Lo que sigue es lo que decia:
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

- [x] **E0 -- LA DECISION: ESTATICO.** Tomada por el dueno el 2026-09-17:
      *"si, estatico, borra lo dinamico y empieza"*. Un `.bex` lleva dentro todo
      lo que ejecuta, asi que la firma lo cubre entero y corre igual en cualquier
      BMO-X. El enlazador dinamico de `bmo-abi` se borro con epitafio en
      `platform/abi/bmo-abi/src/bef/mod.rs`, y `BefFlags::SHARED_LIBRARY` queda
      RETIRADA: el validador la rechaza.

- [x] **E1 -- EL CONTRATO DEL OBJETO. HECHO el 2026-09-17** en
      `platform/abi/bmo-abi/src/bef/objeto.rs`: la forma entera en la cabecera
      del fichero, `BefFlags::OBJECT` (bit 11), `SECTION_UNDEFINED` (0xFD) y
      `objeto::read`, que rechaza con motivo un objeto mal formado (seis pruebas,
      una hostil). El gate del kernel (`bmo-bex-gate`) dice
      *"es un OBJETO sin enlazar: pasalo por bmo-enlazar"*, y el validador acepta
      un objeto y rechaza el mismo fichero marcado como imagen.
      Dos fallos que salieron al escribirlo: el validador leia TODA seccion
      `Symbols` desplazada 8 bytes desde el 08-14 (y con una referencia sin
      alinear), y `SymbolTable::name_of` leia un prefijo de longitud que ningun
      productor escribe. Lo que decia la casilla: Que
      distingue un `.bo` (objeto) de un `.bex` (imagen): una bandera en la
      cabecera BEF, simbolos INDEFINIDOS (`section_idx` sin seccion), y
      relocaciones contra simbolo en vez de contra seccion. Va en
      `VALKYRIE-ABI/` o junto a `bef/header.rs`, y pide su numero de version.
      **Como se sabe**: `bmo-verify` rechaza un `.bo` como ejecutable, con motivo.

- [x] **E2 -- BMO C escribe un objeto. HECHO el 2026-09-17.** `-c` emite un
      `.bo`; el modo objeto vive en `toolchain/lang/c/src/codegen/objeto.rs` y su
      banco en `src/tests/objeto.rs` (7 filas). El parser guarda lo que tiraba
      --prototipos, `static` de fichero y `extern`-- en `ast::Enlace`, y **los 41
      ejecutables del arbol salen byte a byte iguales**: el modo imagen no
      cambio. Dos fallos que salieron: `&x` de un nombre que no existe emitia
      CERO en silencio (dos sitios), y una funcion con solo prototipo no podia
      usarse como valor. Lo que decia la casilla: una orden `-c` que
      emite `.bo` con sus globales como simbolos y cada llamada a una funcion que
      no esta en la unidad como relocacion `Rel32` contra un simbolo indefinido
      -- donde hoy dice *"aqui no hay enlazado: todo lo que se llama tiene que
      estar en esta unidad"*. **Como se sabe**: un banco en el anfitrion que lee el
      `.bo` y encuentra el simbolo indefinido y su relocacion.

- [x] **E3 -- `bmo-enlazar`. HECHO el 2026-09-17.** `toolchain/tools/bmo-enlazar`,
      7 filas que compilan C de verdad, enlazan y EJECUTAN: dos unidades que se
      llaman dan `42`, las cadenas y los globales cruzan de unidad, dos `static`
      con el mismo nombre no chocan, y los cuatro "no" (nadie lo define, definido
      dos veces, sin `main`, no es un objeto) dicen nombre y unidad. El mismo
      mandato da los mismos bytes. Lo que decia la casilla: N objetos -> un `.bex`: junta
      secciones, resuelve simbolos, aplica relocaciones y llama a `bmo-verify`
      ANTES de escribir. Y dice que NO con nombre: simbolo definido dos veces,
      simbolo que nadie define, relocacion que no cabe. Sin `Weak` al principio
      (una promesa menos que cumplir).
      **Como se sabe**: `dos.c` + `uno.c` -> `.bex` que corre en el emulador con la
      salida EXACTA, y el mismo `.bex` sale igual byte a byte en dos corridas.

- [ ] **E4 -- EL METAL.** Ese `.bex` enlazado, en el Ryzen. Hoy corre en el
      emulador; lo que falta es una foto. **La condicion 2 de C++ esta cumplida
      en el anfitrion desde el 17-09** (`APARCADO.md`, 7.2), y se cierra del todo
      con esta casilla.

- [x] **E2b -- LOS CUERPOS QUE TRAEN LAS CABECERAS. HECHO el 2026-09-17.**
      Las cabeceras de BMO traen la implementacion dentro --no habia enlazado,
      asi que la cabecera ERA la implementacion-- y con dos unidades que incluyan
      `<string.h>` eso son dos `strncpy` definidas. Ahora cada unidad se queda su
      **copia privada**, que es lo que hace `static inline` en la libc de verdad:
      el nombre no sale de la unidad y no hay choque. La regla la pone C y no hay
      que explicarla: `<...>` es del sistema, `"..."` es tuyo. El preprocesador
      anota en `rangos_sistema` que lineas del texto expandido vinieron de una
      cabecera del sistema, y la decision se toma DESPUES de parsear
      (`politica_libc`): el parser no tiene por que saber que existe un
      enlazador. **Como se sabe**: `dos_unidades_que_incluyen_la_misma_cabecera_no_chocan`.

- [~] **E5 -- la libc como biblioteca. HECHA, y todavia NO paga.** 2026-09-17:
      `bmo-c-front --libc` compila los cuerpos de las seis cabeceras que tienen
      cuerpo UNA vez a `libc.bo`, y `-c --libc-aparte` compila una unidad que los
      deja fuera (su firma se queda como prototipo, el nombre sale indefinido).
      Enlazado da lo mismo que con la copia:
      `la_libc_aparte_hace_lo_mismo_que_la_copiada`.

      **Y el numero dice que falta la otra mitad.** Medido con un programa que
      solo usa `strncpy`:

      ```text
         la unidad con su copia        5.242 B        el programa    5.100 B
         la unidad SIN la libc         1.698 B  (3x menos)
         libc.bo (los cuerpos)        29.306 B
         el programa + libc entera                                  28.197 B  (5,5x MAS)
      ```

      La unidad encogia y el programa CRECIA, que es literalmente el "como se
      cae" que esta casilla tenia escrito. **Arreglado el mismo dia por E5b**:
      con la poda puesta el programa queda en 2.132 B, 88 bytes por encima de
      llevarse la copia. El numero se dio la vuelta en la fila que lo exigia, no
      en una impresion.

      [!] **Se hizo antes que E4, que la seccion 4 llama un error**, por orden
      del dueno (17-09). El aviso sigue en pie y por eso E5 queda en `[~]` y no
      en `[x]`: la libc aparte no la ha ejecutado ningun CPU. E4 es su condicion.

- [x] **E5b -- TIRAR LO QUE NADIE LLAMA. HECHO el 2026-09-17**, en
      `toolchain/tools/bmo-enlazar/src/tirar.rs`. Se marca desde `main`, se
      conserva lo alcanzable y lo demas no se copia. **Y el numero de E5 se dio
      la vuelta**, con las cuatro esquinas medidas sobre el programa que solo
      usa `strncpy`:

      ```text
                                sin poda     con poda
         la copia privada        5.100 B      2.044 B   (-60,0 %)
         la libc APARTE         28.197 B      2.132 B   (-92,4 %)
      ```

      O sea: enlazar contra la libc entera ya no cuesta 5,5 veces mas, cuesta
      **88 bytes** mas -- y esos 88 no son cuerpos, son su `rodata`, que no se
      poda. La fila que exigia lo contrario esta reescrita afirmandolo
      (`enlazar_contra_la_libc_entera_ya_no_cuesta_mas`).

      **Lo que hubo que arreglar primero, y no estaba en el plan**: una llamada
      a una funcion de la MISMA unidad la cerraba el compilador, porque hasta
      ahora el enlazador copiaba cada unidad entera y esa distancia seguia
      valiendo. Tirando funciones, las de al lado se mueven -- y una distancia
      ya escrita apuntaria a media instruccion SIN fallar al enlazar. Ahora un
      objeto lleva ABIERTAS tambien sus referencias internas, que es lo que
      `objeto.rs` decia de `lea [rip+cadena]` desde E2.

      **Como se comprueba que no es una trampa**: un podador que tire de mas
      deja un programa que enlaza, pasa el gate y salta al vacio. Por eso las
      filas exigen las dos mitades juntas --que tire `isspace` y NO tire
      `strncpy`-- y que la salida sea la misma que sin podar. Mutado, caen dos:
      quitar la raiz del puntero a funcion tumba una fila, y no seguir las
      aristas tumba tres.

- [ ] **E5c -- EL ARBOL ENTERO POR EL CAMINO DEL OBJETO.** Los 41 ejecutables
      se compilan en modo IMAGEN, que NO poda: lo de arriba no les llega. Pasar
      los mismos fuentes por `-c` + `bmo-enlazar` ya se midio el 17-09:

      ```text
         hola_C       2.791 ->  2.791 B    0,0 %   (no le sobra nada)
         scroll_C     6.971 ->  5.885 B  -15,6 %
         raycaster   35.386 -> 24.825 B  -29,8 %
         cubo_C      46.015 -> 28.561 B  -37,9 %
      ```

      **No se hace sin que lo decida el dueno**, y no por prudencia: cambia los
      41 ejecutables, o sea que se pierde la prueba de byte-identico con la que
      se han verificado los tres ultimos cambios de fondo. Se paga una vez, con
      la vista puesta.
      **Como se sabe**: `cubo.bex` baja a la altura de los 28.561 B y el
      escritorio sigue pintando su icono.

- [ ] **E5d -- EL `bss` NO SE SABE NOMBRAR.** Salio al hacer E5b: una reloc del
      objeto nombra su seccion destino con TRES codigos --codigo, datos,
      rodata-- y `bss` no es ninguno. Un puntero guardado en un dato que apunte
      a un global sin inicializar de OTRA unidad (`extern int tabla[100];`) no
      se puede expresar. Hoy eso **para el enlace con su nombre**
      (`BssNoSeSabeNombrar`) en vez de escribir una direccion de `rodata`, que
      es lo que hacia la cuenta anterior sin decirlo.
      **Como se cierra**: un cuarto codigo, y el cargador del kernel sabiendo
      aplicarlo -- toca Ring 0, y por eso es casilla y no un apano.

- [x] **E5e -- C++ COMPILA POR SEPARADO. HECHO el 2026-09-17**, y costo cuatro
      lineas: C++ baja al arbol de BMO C y usa SU codegen, asi que quien decide
      si las referencias salen abiertas o cerradas es el de C. **La compilacion
      separada de C++ la pago E2 sin saberlo.** `bmo-cpp-front -c` escribe un
      `.bo`, y la fila `dos_unidades_de_cpp_se_llaman_y_el_programa_corre`
      compila dos unidades, las enlaza y las EJECUTA.
      Con esto cae la ultima condicion de `toolchain/lang/cpp/APARCADO.md` 7.4
      que no era el metal.

- [ ] **E9 -- LO QUE HAY QUE HACER ANTES DE `main`.** Salio al hacer E5e y es el
      techo de C++ en varias unidades: las tablas de clase se rellenan al entrar
      en `main`, asi que una unidad con clases y sin `main` no tiene donde
      rellenarlas. Hoy eso **para con su nombre** al compilar, en vez de
      entregar un `.bex` que salta a una tabla de ceros en el metal. Lo que
      falta se llama inicializacion estatica entre unidades.
      **Como se sabe**: una unidad con una clase virtual y sin `main`, enlazada
      con otra que si lo tiene, llama al metodo virtual y acierta.

- [ ] **E10 -- `extern "C"`.** C++ DECORA los nombres con la firma
      (`cobrar#i.i`), que es lo que hace posible sobrecargar -- y por eso un
      `.bo` de C que pida `cobrar` no lo encuentra. Es el caso de "librerias en
      C++ para programas en C" que `APARCADO.md` daba como motivo para existir,
      asi que sin esto ese motivo sigue sin cumplirse.
      **Como se sabe**: la fila de dos unidades, pero con una en C.

- [ ] **E6 -- COBOL `CALL` estatico.** `toolchain/lang/cobol/PLAN_BANCA.md`, 6.2 y
      6.3, sobre E3. Aqui se prueba que el contrato es de FORMATO: un `.bo` de
      COBOL y uno de C en el mismo `.bex`, cada uno con su convencion de llamada
      declarada y ninguno sabiendo del otro.

      [!] **Y esto NO sale a cuatro lineas como C++**, medido el 17-09: C++ baja
      al arbol de BMO C y hereda su emisor, pero COBOL tiene el suyo --2.947
      lineas en `codegen.rs`-- y **no escribe ni un simbolo ni una reloc**: hoy
      cierra todas sus referencias porque nunca hubo otra cosa. Lo que
      le falta es lo mismo que aprendio BMO C en E2: dejar abierto lo que no es
      suyo y publicar lo que si.

- [ ] **E7 -- Ada `package` en dos ficheros.** `toolchain/lang/ada/PLAN_ADA.md`,
      escalon A6. El orden de elaboracion (RM 10.2.1) lo decide el enlazador por
      las dependencias `with`, y un ciclo es un error con los dos nombres.

      [!] Mismo aviso que E6: `ada/src/codegen.rs` son 602 lineas propias, sin
      simbolos ni relocaciones. Y ademas Ada pide algo que C no: la
      ELABORACION es codigo que corre antes del programa -- o sea la misma
      pregunta que E9 le hizo a C++ con las tablas de clase, y conviene que la
      conteste UNA vez para los dos.

- [ ] **E8 -- INTI decide.** INTI compila hoy un fichero con sus modulos dentro;
      si quiere objetos, los pide aqui. No es obligatorio: el formato es
      opcional para quien no lo necesite (regla 2).

---

## 3b. Quien sabe escribir un objeto hoy (17-09)

```text
   BMO C      SI    `-c`        E2
   BMO C++    SI    `-c`        E5e -- gratis: usa el codegen de C
   COBOL      NO                E6 -- codegen propio, 2.947 lineas, 0 relocs
   Ada        NO                E7 -- codegen propio, 602 lineas, 0 relocs
   INTI       NO                E8 -- y es OPCIONAL a proposito (regla 2)
```

Las dos columnas de la derecha son la respuesta a "por que C++ fue barato y los
otros no": no es el lenguaje, es **de quien es el emisor**.

---

## 4. Lo que seria un error

- **Un IR comun "para que el enlazador lo tenga facil".** Es el cerebro que la
  regla 2 prohibe. El enlazador lee BEF, y nada mas.
- **Empezar por el dinamico** porque ya habia uno escrito. Existia y no lo
  llama nadie, y va contra una decision tomada.
- **E5 antes que E4.** Mover la libc a una biblioteca sin un enlace probado en
  metal es apilar sobre un camino que nadie ha visto funcionar -- la misma frase
  con la que se aparco C++.
  ** 17-09: se hizo igual, por orden del dueno. El aviso no se borra ni se
  rebaja: E5 queda en `[~]`, y lo que lo cierra sigue siendo la foto del Ryzen.
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
