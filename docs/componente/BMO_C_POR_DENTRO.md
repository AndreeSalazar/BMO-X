# BMO C POR DENTRO -- el vehiculo, y los dos jueces que no se hablan

> Escrito el 2026-09-02, analizando `toolchain/lang/c` entero antes de tocar
> nada. El motivo es el rojo numero 2 del orden de trabajo: este compilador
> emite **una respuesta equivocada en silencio**, y hay cinco casillas con
> `#[ignore]` que lo reproducen en el anfitrion.

---

## 0. Que representa, y por que importa mas que los otros frontends

Desde la decision del **2026-08-09**, C dejo de ser un lenguaje mas y paso a ser
**el vehiculo**: *"Con C ese mismo seria como compositor y muchas cosas"*. COBOL
bajo a demostracion, Ada esta por escribir, INTI es otra cosa.

De ahi salen las tres consecuencias que hacen critico a este directorio:

```
   los 13 ejemplos de C        la prueba de que REX funciona
   DOOM (81 ficheros)          el instrumento que mide el sistema
   el escritorio y las apps    lo que se escribe de aqui en adelante
```

** Un fallo de codegen aqui **no es un fallo de DOOM**: es un fallo de todo
programa que compile cualquiera, hoy y despues. Por eso es rojo.

---

## 1. Los numeros, que dicen algo por si solos

```
   codigo      11.675 lineas en 30 ficheros
   pruebas     11.405 lineas en 46 ficheros
   ejemplos    15 programas de C en examples/
   banco       449 filas en `bmo.ps1`
```

★ **Casi uno a uno entre codigo y pruebas.** Eso no es celo: es la respuesta a
la enfermedad de este compilador, que no es caerse -- es acertar a veces. Un
banco que EJECUTA lo emitido (`bmo-lower` con `features = ["emulator"]`) es lo
unico que caza *"compila, corre, y el numero es plausible y falso"*.

---

## 2. La forma: el tubo, y las cuatro piezas de forge

```
   .c  ->  preprocessor  ->  lexer  ->  Parser  ->  Program (AST)  ->  Codegen  ->  bmo-verify  ->  .bex
```

`bmo-c-front` no se enlaza con nadie mas que con cuatro piezas de `forge`, y
cada una tiene su motivo escrito en el `Cargo.toml`:

| pieza | que aporta | por que no es un embudo |
|---|---|---|
| `bmo-abi` | BEF, reubicaciones, la superficie | es el contrato, no una libreria |
| `bmo-sem-asm` | el codificador de x86-64 | **elegido**, compartido con los otros frontends |
| `bmo-lower` | L1, la puerta (`INVOKE`/subsyscalls) | elegida; y en pruebas trae el emulador |
| `bmo-mods` | tablas y manifiestos (`$BMO_MODS`) | sustituye a dos lectores de TOML a mano |
| `bmo-verify` | **EL GATE** | ningun `.bex` se escribe sin pasar por aqui |

★ La nota del manifiesto merece quedarse: `bmo-verify` es *"el papel de
seguridad que tendria un IR central, pero como CONTRATO"* -- cada lenguaje
emite lo suyo y esto lo revisa. Es la razon de que no haya un IR unico y no
haga falta.

### El reparto de `codegen/` (2.320 lineas en `mod.rs` + diez modulos)

Cada submodulo lleva en su cabecera **la pregunta que contesta**, y dos de esas
frases son el mapa del fallo de hoy:

```
   indexing.rs      "el STRIDE es el numero que ha fallado mas veces que
                     ningun otro aqui"
   floats.rs        "el unico valor que no viaja en rax"
   format.rs        printf, la unica parte que emite un INTERPRETE
   sintetizadas.rs  nombre -> los bytes que lo implementan (no sabe que es C)
```

---

## 3. ** LA DECISION QUE LO EXPLICA TODO: el offset viaja EN EL AST

Esta es la pieza sin la que nada de lo de abajo se entiende. En el AST:

```rust
   Arrow(Box<Expr>, String, u32, TypeSpec)
   //                       ^^^ el OFFSET DE CAMPO, ya resuelto
```

O sea que **el parser tiene que hacer de comprobador de tipos**. Su propio
fichero lo dice sin adornos (`parser/types.rs`):

> *"the parser of C cannot stay a parser [...] the `Expr::Field` node **carries
> the byte offset inside it**, so by the time the tree is built the parser has
> already had to answer 'what type is `a`, what does `b` point at, and how far
> in is `c`'. That is a type checker's job living inside a parser."*

Y la consecuencia esta escrita, tambien, en la funcion que resuelve el tipo:

> *"Devuelve None si no es resoluble (y el offset caera a 0 -- visible en
> tests)."*

** **Ese `unwrap_or(0)` es el fallo de hoy.** No es un descuido: esta declarado.
Lo que nadie comprobo es CUANTAS formas de C caen en el `None`.

---

## 4. ** LOS DOS JUECES DEL MISMO NUMERO (patron 55, otra vez)

Hay **dos** funciones que contestan *"a que apunta esta expresion"*, en dos
ficheros, y **no saben lo mismo**:

```
   parser/types.rs     resolve_expr_type()    <- decide el OFFSET que se graba
   codegen/indexing.rs pointee_type()         <- decide el STRIDE al emitir
```

| forma de C | el parser | el codegen |
|---|---|---|
| `Expr::Var` sobre `Array` (decae) | **NO** | SI |
| `Expr::Add` / `Expr::Sub` (aritmetica) | **NO** | SI |
| `Expr::AddrOf` (`&x`) | SI | **NO** |
| `p++` sigue siendo puntero | NO | SI |

★★ **Y el mas debil es el que graba el numero en el arbol.** El codegen sabe
mas, y llega tarde: cuando el `Expr::Arrow` le llega, el `u32` ya esta dentro y
el codegen no lo revisa. Es exactamente la forma del patron 55 --dos jueces de
la misma magnitud con dos techos, y manda el flojo-- con el agravante de que
aqui el flojo escribe y el fuerte ni se entera.

⚠ **Y el motivo escrito para dejar la aritmetica fuera no cubre el caso que
rompio.** El comentario dice:

> *"Se anaden solo estos y no la aritmetica: el tipo de `a + b` pide las
> conversiones usuales de C, y equivocarse aqui no da un error, da un `memset`
> de la medida equivocada."*

Eso es cierto para `entero + entero`. Para **`puntero +/- entero` C no tiene
ninguna pregunta que hacer**: el tipo del resultado ES el del puntero. Las
conversiones usuales solo aplican a dos operandos aritmeticos. O sea que la
prudencia se aplico al unico caso que no la necesitaba, y ese es el que se usa
para recorrer cualquier tabla.

---

## 5. Las cinco casillas rojas, y sus DOS causas

`src/tests/sonda_resta_de_punteros.rs`, 441 lineas. Se corren con:

```
   cargo test -p bmo-c-front sonda_resta_de_punteros -- --ignored
```

### CAUSA A -- el parser no sabe el tipo de una expresion aritmetica

Tres casillas, y el par verde/rojo lo demuestra sin lugar a duda:

```
   p = tope - 1;  p->next = &arr[0];     VERDE   la variable tiene tipo declarado
   (tope - 1)->next = &arr[0];           ROJA    en linea, se pierde el tipo
```

La cuarta casilla dice **donde** cae, que es lo que nombra al culpable: da
`1 0`, o sea que escribio en `prev` (offset 0) en vez de `next` (offset 8).
**No es una direccion mal calculada: es un offset de campo que vale cero.**

Ruta exacta: `(tope - 1)` es `Expr::Sub` -> `resolve_expr_type` no tiene brazo
-> cae en `_ => None` -> `resolve_arrow_expr_offset(...).unwrap_or(0)`.

[!] Y el mismo `None` estropea una segunda cosa por otra puerta:
`field_type_via_pointer(...).unwrap_or(TypeSpec::Long)` -- o sea que ademas de
escribir en el sitio equivocado, escribe **ocho bytes**.

**Esto es lo que mata a DOOM.** `(tope-1)->next = &centinela` no cierra la
lista, el ultimo elemento conserva `next = ds+1`, el recorrido se va del array,
y `R_SortVisSprites` acaba leyendo `ds->scale` con `ds` a NULO.

### CAUSA B -- el codegen no reconoce `&x` como puntero

Una casilla: `(int)(arr - &arr[5])` da **-679168** en vez de **-5**.

`pointee_type` no tiene brazo para `Expr::AddrOf`, asi que `pointer_scale(&arr[5])`
contesta `None`. Y entonces el brazo de `Expr::Sub` no ve *"puntero menos
puntero"* sino *"puntero menos ENTERO"*, y toma la rama que **multiplica el
segundo operando por 80**:

```rust
   (Some(scale), None) => { let scaled = Expr::Mul(b, Int(scale)); ... }
```

★ Eso explica la observacion que la casilla ya traia escrita y que descartaba
la teoria facil: *"ni siquiera son bytes: -400 seria 'se olvido de dividir', y
-679168 no es eso"*. No se olvido de dividir: **multiplico**.

### La quinta

`la_lista_circular_con_centinela_local_se_recorre_entera` es la reproduccion
completa de la muerte de DOOM (`8 101` donde toca `8 8`). No es una causa
aparte: es la CAUSA A vista de lejos, y es la que se pone verde sola cuando A
se cierre. **Sirve de juez, no de tarea.**

---

## 6. Lo que este compilador NO hace, y esta bien que no lo haga

- **Ring 0.** `TargetProfile::Ring0Kernel` existe y **rechaza diciendo por que**.
  Se conserva la variante a proposito: *"la pregunta se la va a hacer alguien
  otra vez, y un rechazo con su motivo contesta mejor que un enum donde la
  opcion no aparece"*.
- **Compilacion separada.** Solo unity build. Es lo que bloquea SDL, y lo que
  hace imposible traer un editor visual de fuera.
- **Enlazado dinamico.** No hay `libbmo.so`; por eso una cabecera de REX trae el
  cuerpo y se paga por incluirla ([[PLAN_REX]]).
- **libc completa.** Lo justo, y declarado: *"C es neutro -- su trabajo es no
  estorbar"*.

---

## 7. EL 2.0, PASO 1 -- HECHO el 2026-09-02

El arreglo **no fue anadir dos brazos**: fue que las dos funciones dejaran de
existir por separado.

```
   src/tipos.rs            EL JUEZ UNICO. `tipo_de` y `apunta_a`, mas un
                           `trait Ambito` de UNA sola pregunta: el tipo de un
                           nombre. Todo lo demas sale del arbol, y por eso los
                           dos consumidores obtienen la MISMA respuesta sin
                           compartir tablas.
   parser/types.rs         `resolve_expr_type` -> delega
   codegen/indexing.rs     `pointee_type`      -> delega
```

Las dos causas se cierran en el mismo sitio y de una vez:

| era | ahora |
|---|---|
| `(tope - 1)->next` grababa offset **0** (escribia en `prev`) | brazo `Add`/`Sub`: `puntero +/- entero` conserva el tipo del puntero |
| `arr - &arr[5]` daba **-679168** | brazo `AddrOf`: `&x` ES una direccion, asi que el codegen ve "puntero menos puntero" y divide en vez de multiplicar |

★ Y una tercera decision que no estaba en ninguna de las dos: **`p - q` NO es un
puntero, es un indice.** Decir que sigue siendo puntero haria que
`(p - q)->campo` resolviera un offset con toda confianza sobre algo que ya no
apunta a ningun sitio.

### Lo que se ELIMINO

```
   src/ir_emit.rs   282 lineas MUERTAS. Emitia un `bmo_abi::ir::IrModule`
                    "para que cualquier backend (x86-64, ARM64, RISC-V) lo
                    consuma", y en todo el repo NADIE consumia ese IR: los
                    unicos ficheros que lo mencionaban eran el mismo y la
                    linea de `lib.rs` que lo exponia.
```

### Y llega a DOOM, COMPROBADO -- no deducido

`r_things.c:803` es la construccion **literal**, sin parecido ni analogia:

```c
   788:  count = vissprite_p - vissprites;     <- puntero menos puntero
   803:  (vissprite_p-1)->next = &unsorted;    <- LA LINEA
```

Antes, ese `->next` resolvia offset **0**: escribia en `prev`. O sea que el
ultimo elemento nunca apuntaba al centinela, el recorrido de la linea 813 se
iba del array, y `ds->scale` acababa leyendose con `ds` a NULO -- que es el
`#PF NULO en 0+0x2c -> R_SortVisSprites+0x2c6` del metal, entero.

[!] `doom.bex` **mide lo mismo que antes (894.929 B) y NO es el mismo fichero**:

```
   antes   B72EAC9AA44337C9DBAC696249072284CEF4D9D4E6B978F8D81F67DDB89B896D
   ahora   E2B9AAC1203BBB008D32DF67E65D82304D00BAE67507D5057388DF296B690DEA
```

*** El tamano identico es la trampa: lo que cambia es un byte inmediato, no una
instruccion. **Comparar tamanos habria dicho "no se reconstruyo".** El unico
juez de si un binario cambio es su hash.

[!] Y esto NO dice que DOOM se juegue. Dice que la causa medida en el anfitrion
esta cerrada y que el binario la lleva. Lo que falta es el metal.

### El resultado, medido

```
   antes   449 pasan  +  5 con #[ignore] reproduciendo bugs abiertos
   ahora   454 pasan  +  0 ignoradas
```

Las cinco casillas **cambian de oficio**: dejan de reproducir fallos y pasan a
ser el guardian de los dos que cerraron. Y su cabecera dice donde mirar si
alguna vuelve a ponerse roja -- *no el brazo que falta, sino si alguien ha
vuelto a contestar esta pregunta por su cuenta.*

---

## 8. EL 2.0, PASO 2 -- EL COTEJO (2026-09-02)

El plan era fusionar las cinco parejas de disposicion que quedaban del censo
del punto 4. **Al leer el arbol, el plan estaba mal**, y lo dice una cabecera
que ya estaba escrita en `codegen/mod.rs`:

> *"Que el codegen la recalcule en vez de recibirla del parser **no es
> duplicacion**: es lo que hace que un frontend distinto (C++) que ya calculo
> offsets para sus nodos `Field` no pueda imponer una disposicion propia sin
> que se note."*

*** **El argumento es bueno.** Fusionar habria borrado una defensa a proposito
por confundirla con un descuido.

### Pero la implementacion no cumplia lo que la cabecera prometia

Se calculaban DOS disposiciones y **no se comparaba ninguna**.

> Dos cuentas que nadie contrasta no son una comprobacion doble: son dos
> oportunidades de equivocarse.

Y ya habia pasado: el 13-08 divergieron, y lo destapo un bug -- no un guardian.

### Lo que entra

```
   ast/program.rs           `Program.disposiciones` -- lo que el FRONTEND dice
                            que mide cada agregado y donde cae cada campo
   parser/mod.rs            lo COPIA de las tablas que ya lleno (no recalcula:
                            eso seria fabricar un tercer juez)
   codegen/disposicion.rs   colocar + `cotejar_disposicion`, en su fichero
```

** Y el cotejo se prueba por los dos lados. Que un guardian no se queje puede
significar dos cosas muy distintas --que todo cuadra, o que no mira--, asi que
`tests/cotejo_de_disposicion.rs` fija las cuatro:

```
   coinciden -> compila            (sin esta, un cotejo que rechaza SIEMPRE
                                    tambien pasaria por guardian)
   un offset movido un byte -> NO, y el mensaje nombra agregado Y campo
   un tamano que no cuadra  -> NO, y dice que lo que falla es el tamano
   `disposiciones` vacio    -> compila: ese frontend no declara la suya
```

Es la misma exigencia que el contrato le hace a sus diecisiete reglas: **saber
decir que NO.**

### L6a mordio, y tenia razon

Anadir el cotejo hizo crecer `codegen/mod.rs` 34 lineas de codigo, y ese fichero
esta en la lista del trinquete: **solo puede encoger**. La respuesta no fue
levantar el techo sino repartir, que es para lo que la regla existe -- colocar
un agregado y comprobar esa colocacion son **un solo concepto**, y estaban entre
una funcion sin vecinos y ninguna parte.

```
   codegen/mod.rs        1398 -> 1352 lineas de CODIGO   (techo sellado abajo)
   codegen/disposicion.rs                84
```

[!] Al sellar, `Ultra_kernel_x86-64/build.ps1` **salio de la lista**: mide 559
lineas (525 de codigo) desde que se partio en `build/`, y ese reparto nunca se
habia sellado. No es un techo que se afloje: es uno que ya no hace falta.

---

## 9. LO QUE FALTA DEL 2.0

Con el cotejo, las cinco parejas de disposicion **dejan de ser un riesgo**: si
divergen, ahora se dice. Lo que queda es el paso que devuelve el parser a ser un
parser:

```
   vaciar las 8 variantes de `Expr` que cargan datos resueltos
   (Subscript, AssignSubscript, IndexPtr, AssignIndexPtr,
    Field, Arrow, AssignField, AssignArrow -- de 50 en total)
```

Es lo que en un compilador de C tipico separa el front end de la pasada
semantica: el nodo NOMBRA el campo y el offset lo pone despues quien tiene la
tabla. Hoy `tipos.rs` ya es ese "despues" para los TIPOS; falta que lo sea
tambien para los OFFSETS.

[!] No se hizo hoy a proposito: toca `Program`, el codegen entero y las 47
casillas que construyen expresiones a mano. Va en su propio paso, con el banco
delante -- no colgando de otro cambio.
