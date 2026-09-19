# PLAN EL TROQUEL -- la geometria de los registros, estampada de un golpe

> Propuesta del dueno, **2026-09-09**, y la trajo mirando la jerarquia de caches:
>
> > *"pensaba que INTI mas cerca del registro, esa jerarquia, para que INTI
> > salte y asi facilitar. Podemos preparar la propuesta, pero ya no seria
> > compilador sino que ese es LIBRERIA para x86-64, basicamente como que es
> > intermedio para procesar en vez de demorar en RAM."*
>
> [!] **Esto es una propuesta y no toca nada todavia.** Lo que hay aqui es el
> mapa medido, el hueco que se ve al dibujarlo, y lo que costaria.

---

# 1. ★★★ EL NUMERO QUE ABRE LA PROPUESTA

Se conto cuantos sitios del arbol escriben bytes de x86-64 a mano, y cuantos lo
piden a la libreria que existe para eso (`sem-asm`):

```text
   emisor                     a mano    por sem-asm
   ------------------------------------------------
   lang/c                        321          18
   lang/cobol                    105          33
   lang/ada                       16           0
   inti/emisor-x86_64             51           2
   forge/bmo-lower               176           0
   ------------------------------------------------
   TOTAL                         669          53      -> el 7,3 %
```

★★ Y el ultimo de la lista es el que duele: **`bmo-lower` es la capa GENERICA
del pipeline** --su propia cabecera se llama a si misma *"L1: el descenso
generico"*-- y escribe 176 secuencias a mano sin pedirle ni una a `sem-asm`.

## ⚠ Y `sem-asm` lleva escrito para que existe desde el dia que nacio

Su primera linea, sin tocar:

> *"Reemplaza el hardcodeo de bytes DUPLICADO en `lang/c/src/codegen.rs` y
> `lang/cobol/src/codegen.rs` (ambos escriben 0x48/0xB8... a mano)."*

**Van ocho.** `constante_de` estaba completo y el emisor no preguntaba;
`ritmo()` se media y solo lo leia Ring 0; `FB_OP_BYTES` decia *"para un
`rep stosd`"* y nadie lo hacia. Esta es la misma frase otra vez:

> La pieza existe, dice para que existe, y quien la necesita no la usa.

---

# 2. PERO LO QUE PROPONE EL DUENO NO ES `sem-asm`

Y esa distincion es toda la propuesta. `sem-asm` traduce **un mnemonico a
bytes**: le dices `mov rax, 8` y te da `48 C7 C0 08 00 00 00`. Es una tabla.

Lo que falta es la pregunta de **antes**: *"este valor, donde vive?"*.

```text
   L2  ESPECIALIZADA    la semantica de cada lenguaje       existe, una por uno
   ??  EL HUECO         donde vive un VALOR: registro o     NO EXISTE
                        pila; cuando muere; con quien
                        comparte sitio
   L1  GENERICA         `bmo-lower`: el descenso al ABI     existe
   L0  SUPERFICIE       INVOKE / WAIT                       congelada
```

★ El diagrama de arriba es **el del propio `bmo-lower`**, con una fila anadida.
El hueco estaba dibujado y sin nombre.

---

# 3. POR QUE ESTO ES "MAS CERCA DEL REGISTRO"

La jerarquia que el dueno nombro, con el nivel que se le olvida a todo el mundo
puesto arriba:

```text
   registros    0 ciclos      son el operando. No hay viaje
   L1d         ~4-5 ciclos    32 KiB por nucleo    (cache.rs lo sabe)
   L2         ~13 ciclos      512 KiB por nucleo
   L3         ~46 ciclos      32 MiB, un solo CCX
   RAM       ~300+ ciclos
```

[!] Esas latencias son las **publicadas** de Zen 3, no medidas aqui. LEY 24 dice
que una cifra generica es de otro proyecto, y `bmo::ciclos()` mas cinco bucles
darian las de esta maquina.

*** **BMO C nunca usa el nivel 0.** Es una maquina de pila: cada variable vive
en su hueco de `%rbp` y cada operacion viaja a memoria, aunque sea a L1. Los
`push`/`pop` que se quitaron el 09-09 eran exactamente eso.

Y no se puede "elegir L1 en vez de L2" --eso lo decide el silicio--. Lo unico
que se decide es **si el valor baja a la escalera o no**. Eso es esta libreria.

---

# 4. ★★★ LA CLAVE NO ES EL REGISTRO: ES EL ALIAS

Por que un compilador manda a memoria una variable que cabria en un registro?
Porque **no puede probar que un puntero no la pisa**. Con un `*p = x` en medio
tiene que asumir lo peor y volver a leerla.

```text
   adivinar el TIPO    -> emite `div` donde iba `idiv`      C5 de PLAN_EL_CODEGEN
   adivinar el ALIAS   -> tiene que escribir a memoria      ESTE plan
```

★★ **Las dos son la misma palabra del dueno: adivinar.** Y la segunda es
justamente la que aleja del registro.

Una libreria del sitio no adivina: **recibe lo que el lenguaje declara**.

```text
   entra   los valores, sus vidas, y que punteros NO se solapan
   sale    este en `rax`, este en `rdx`, este a la pila y por que
```

Y por eso el cliente natural es **INTI**: tiene cero UB comprobado en el Ryzen y
puede declarar en la gramatica lo que C solo puede deducir. Un lenguaje nuevo
puede exigir lo que un lenguaje de 1972 tiene que suponer.

---

# 5. ⚠ LA CORRECCION AL PLANTEAMIENTO, Y ES IMPORTANTE

El dueno lo dijo como *"INTI ya no seria compilador sino libreria"*. No:

```text
   INTI el LENGUAJE          se queda. Es "el C de BMO-X" y tiene su gramatica
   inti/emisor-x86_64        se queda. Depende de `bmo-inti-front`, o sea que
                             esta acoplado al arbol de INTI: no es generico
   EL TROQUEL               es NUEVA, vive en `forge/`, y no sale de INTI --
                             INTI es su primer CLIENTE, que no es lo mismo
```

Sacar el emisor de INTI y llamarlo libreria seria heredar su acoplamiento al
AST de INTI, y entonces C y COBOL no podrian usarla. El sitio de un valor no
depende del lenguaje: **por eso puede ser una libreria, y por eso tiene que
nacer sin dueno.**

---

# 6. ★★ Y ESTO RESUELVE LA OBJECION QUE YO MISMO ESCRIBI CONTRA `C4`

En [`PLAN_EL_CODEGEN`](PLAN_EL_CODEGEN.md), el escalon `C4` --mantener en
registro la variable de un bucle-- lleva escrito este sacrificio:

> ⚠ *"es donde un compilador deja de poder leerse de una sentada, que es la
> propiedad por la que existe este."*

*** Con la libreria **esa objecion desaparece**: la complejidad del analisis de
vivos vive FUERA, en una crate con su propio banco, y BMO C sigue cabiendo en
una tarde de lectura. El dueno resolvio sin saberlo la pega que bloqueaba `C4`.

Y hay un segundo premio: **una asignacion de registros escrita SEIS veces no se
escribe nunca.** Hoy hay seis sitios que emiten x86-64. Escrita una vez, la
heredan los cinco restantes el dia que la enlacen.

---

# 7. LA ESCALERA

- [ ] **S1 -- EL CONTRATO, EN PAPEL Y ANTES QUE EL CODIGO.** Que entra y que
      sale. Sin un lenguaje dentro: valores, vidas, alias, y la tabla de
      registros de la arquitectura.
      ★ Si el contrato no se puede escribir sin nombrar C ni INTI, es que no
      era una libreria.

- [ ] **S2 -- LA TABLA DE REGISTROS COMO DATO, no como codigo.** x86-64 nombra
      16; RISC-V nombra 32. Cual esta reservado, cual lo pisa una llamada.
      ★★ Y aqui esta el regalo para [`PLAN_EL_GUARDIAN`](PLAN_EL_GUARDIAN.md):
      el backend de RISC-V (G1.2) deja de ser "otro emisor" y pasa a ser **otra
      tabla**. Es la apuesta de esta casa --*tablas y no cerebros*-- aplicada al
      sitio donde mas se nota.

- [ ] **S3 -- EL PRIMER CLIENTE: INTI.** Porque puede declarar el alias. Y
      porque si se estrena en C hay que arrastrar 321 sitios a mano el primer
      dia.

- [ ] **S4 -- BMO C, y solo el bucle caliente.** No los 321 sitios: los de
      `emitir/valor.rs`, que es donde estan los 157 ciclos por escritura.

- [ ] **S5 -- `bmo-lower` deja de escribir bytes a mano.** Los 176. Es la capa
      generica: que la generica no use las librerias genericas es la deuda mas
      barata de pagar y la mas fea de tener.

---

# 8. LAS TRES PREGUNTAS, Y DE AHI SALE EL NOMBRE

El dueno lo ordeno mejor que este documento: *"el compilador procesa, pero si
quieres mas, esto se encarga para que INTI procese; y si hablamos de x86-64 es
esto en libreria, porque **INTI es agnostico**"*.

Escrito como tres preguntas, cada una con su dueno y ninguno sabiendo el
trabajo de los otros dos:

```text
   INTI        decide QUE hay que calcular       semantica     AGNOSTICO
   el TROQUEL  decide DONDE encaja cada valor      arquitectura  x86-64 / RISC-V
   sem-asm     decide QUE BYTES son              codificacion  x86-64 / RISC-V
```

★★ Y eso convierte esta capa en **la frontera de abajo del toolchain**, con la
misma forma que las tres de la raiz (`V-ABI` arriba, `PERFIL` abajo, `NEUTRO` al
lado): es donde el software agnostico deja de serlo. LEY 24 dice que el hardware
se PERFILA y el software es agnostico -- **aqui esta la linea exacta donde se
cruza**, y hasta hoy no tenia sitio.

## ★★★ EL NOMBRE: **TROQUEL**, y lo puso el dueno

Un troquel estampa una forma sobre metal **de un golpe**. No amasa: da una
decision seca y sale la pieza. Y el vocabulario entero viene puesto:

```text
   el DIBUJO del troquel    la arquitectura: x86-64, o RISC-V
   la MATRIZ                el espacio de los registros -- 16 huecos, o 32
   la REBARBA               *** lo que escapo por el borde y no cupo dentro
```

*** **`rebarba` es el hallazgo.** El termino tecnico de un valor que no cabe en
un registro es `spill`, y una rebarba no es *"lo que se cayo"*: es, por
definicion, **el material que escapo por el borde del troquel**. Cae encima de
la palabra tecnica sin forzar nada. Un valor con rebarba va a la pila igual que
el metal sobrante.

### ⚠ Dos correcciones al argumento con el que llego el nombre

El caso a favor traia dos frases que hay que arreglar antes de que se repitan:

```text
   "la fisica del golpe instantaneo (0,1 ns)"
      -> NO. El troquel actua en tiempo de COMPILACION, no de ejecucion.
         Pero la version correcta es MAS fuerte: el programa paga CERO. La
         decision se tomo una vez, en la maquina del que compila, y el `.bex`
         ya nace con los valores en su sitio. No hay 0,1 ns: no hay
         nanosegundos en absoluto

   "determinista y SIN MARGEN DE ERROR"
      -> Al reves, y esto importa. Es de las piezas con MAS margen de error
         del sistema: un registro mal asignado corrompe en silencio y el
         sintoma sale lejisimos. En la escala de `fases.py` seria
         **[aparece] DENTRO, carril ROJO**. No es un pero: es lo que dice que
         NECESITA SU BANCO antes de que nadie confie en ella
```

### Los que se descartaron, y por que

```text
   Motor de Sintesis Bare-Metal   `sintesis` ya esta cogida: en hardware es
                                  HDL -> puertas logicas, y quien viene de ese
                                  mundo es justo el publico de un proyecto
                                  bare-metal. Y `motor` vende de mas, que es lo
                                  que esta casa no hace: `huella`, `testigo`,
                                  `aforo`, `compas` -- ninguno promete mas de
                                  lo que hay, y por eso se creen
   bmo-molde                      aguantaba, pero `derrame` es mas vago que
                                  `rebarba`, y un molde sugiere algo lento y
                                  viscoso. Un troquel da UNA decision
   bmo-sitio                      nombraba la respuesta, no el trabajo
```

[!] Y aun asi el nombre lo pone el dueno. `CUPO` se rechazo por lo que recordaba
en Peru, y esa clase de cosa no la puede saber quien escribe el plan.

---

# 9. ★★ COMO SE VERA -- y lo primero es lo que NO va a cambiar

Pregunta del dueno: *"si entra en BMO-X, cambia el comportamiento? Para ver los
cambios en general, ya sabes, DOOM y otros"*. Si, pero no en todas partes, y la
mitad de la respuesta es la mitad que no cambia.

```text
   NO CAMBIA NADA          el escritorio y el kernel son RUST, compilados por
                           `rustc`. El troquel no los toca ni de lejos
   CAMBIA                  todo `.bex`: DOOM, los 13 ejemplos de C, los 12 de
                           COBOL, el de Ada y las sondas de INTI
```

[!] Y eso hay que tenerlo delante para no atribuirle al troquel un `cuerpo 2` o
un `latido 300/s` que son de otro compilador.

## Los tres jueces, y ya existen los tres

```text
   1. el banco de BMO C        500 filas que EJECUTAN. Es el primero que
                               habla, en 0,3 s, y ya cazo cinco de quinientas
                               el 09-09 con un plegado de mas
   2. el tamano de los .bex    el build imprime los 25. Menos instrucciones
                               es menos bytes, y se ve sin arrancar la maquina
   3. `expansion N us`         el `[perf]` de DOOM. **Este es EL juez**: es
                               donde vivian los 157 ciclos por escritura
```

★ Del 09-09 quedan **28 instrucciones y dos accesos a la pila** en ese bucle. De
las 28, unas catorce son ir y venir a los huecos de `%rbp` -- exactamente lo que
el troquel quita. Pero **cuanto de eso son los 157 ciclos no se sabe**, porque
el metal no ha hablado desde el cambio del compilador.

## ★★★ Y la primera version puede ser TONTA y ganar casi todo

Esto es lo que abarata el plan entero, y conviene decirlo antes de que alguien
se asuste con "analisis de alias":

> **Una variable local cuya direccion nunca se toma no la puede pisar ningun
> puntero.** No hay nada que analizar: es una propiedad que se ve mirando si
> aparece un `&` sobre ella.

Y eso cubre justo lo que importa: los contadores de bucle, los indices, los
punteros que caminan. En el bucle de la expansion son `j` y `d8`, y **ninguno
de los dos tiene su direccion tomada**.

```text
   version tonta    los locales sin `&` viven en registros. Cero analisis
   version lista    alias declarado por el lenguaje -> INTI. Eso es S3
```

*** La tonta se puede escribir sin que INTI declare nada, y **beneficia a C
igual** -- que hoy tiene un suelo de cero, porque una maquina de pila no usa un
registro JAMAS. Pasar de "nunca" a "cuando es obviamente seguro" es el salto
grande; declarar el alias es el ultimo tramo, no el primero.

## Lo que faltaria para verlo de un vistazo

El build ya imprime el tamano de los 25 programas, pero **nadie los compara con
los de ayer**. Una linea base de tamanos --como `avisos` y como `fases`-- haria
que cualquier cambio del toolchain ensenara de golpe su efecto sobre los 25.

[!] Con un aviso: **el tamano no es la velocidad**. Van juntos en este caso
concreto --pasar de pila a registros quita instrucciones-- y no en general. Seria
un REPORTERO, no un trinquete: los programas pueden crecer con razon.

---

# 9. LO QUE ESTE PLAN NO PROMETE

```text
   [ ] no promete "todo en registros". x86-64 NOMBRA 16, y el fichero fisico
       de ~180 no se puede direccionar. Lo alcanzable es el bucle caliente
   [ ] no promete un numero. Del 09-09 quedan 28 instrucciones en el bucle de
       la expansion y DOS accesos a la pila: puede que ya no den los 157
       ciclos. **Eso lo dice el metal y todavia no ha hablado**
   [ ] no es un compilador optimizador. Es UNA decision --donde vive un valor--
       sacada a su sitio. El resto de `PLAN_EL_CODEGEN` sigue donde estaba
   [ ] y no empieza sin S1. Una libreria que nace pegada a un lenguaje se
       queda pegada, y entonces son seis emisores y una excusa
```

> Hoy hay seis sitios que escriben x86-64 y una libreria de codificacion que
> usa el 7% de ellos. **El problema no es que falte una capa: es que las que hay
> no se usan.** Este plan solo vale si la nueva nace con clientes, y por eso el
> primer escalon es un contrato y no un `cargo new`.

---

# 10. LO QUE DIJO EL METRO EL 18-09, Y LO QUE QUEDA

El metro del emisor (`toolchain/tools/metro`: 25 programas, trinquete en el
build, `--desglose` por clase y `--caliente` por direccion) hizo en un dia lo
que la seccion 8 pedia y mas: la linea base de tamanos existe (`tamano.py`,
41 ejecutables) y ADEMAS la de instrucciones. Con el se eligieron seis pasos,
cada uno por el desensamblado del bucle caliente y no por corazonada, y C paso
de 451.306 a 225.124 instrucciones (**-50 %**) con las 25 salidas identicas:
inmediato, comparacion fundida en el salto, troquel POR VARIABLE + operar en
sitio, sombras renombradas (un fallo), operando derecho sin pila, y tres
operandos (`lea`, `imul`, `cmp rN`). El bucle de `blit`: 29 -> 10 por vuelta.

** Y el reparto quedo PLANO: aritmetica 25 %, memoria 16 %, registro 16 %,
salto 14 %, pila 11 %. Ya no hay un cuello: hay un emisor de acumulador. Lo
que queda son ganancias de un digito, y por la ley de optimizacion solo se
tocan si el metro las pide:

- [ ] **T1 -- el recorte del `int` en registro.** MEDIDO el 19-09: el
      `movsxd rN, rNd` tras cada `add rN, imm` es el 6,2 % del banco de C, y
      es TODO `int` con signo. Quitarlo obliga a que el registro deje de
      guardar el valor extendido (la invariante de `emit_guardar_en_registro`)
      o a asumir que el desborde de `int` no ocurre -- y el banco protege el
      desborde (`int x = 2147483647; x + 1 == -2147483648`). Se deja con su
      numero; no se toca sin cambiar la regla, y eso es decision del dueno
- [x] **T2 -- leer la matriz sin `mov rax, rN`**: `t[i] = v` con `v` en la
      matriz escribe `mov [rdx], r12d` directo (`c833ace2`); `and`/`or`/`xor`
      con el izquierdo en la matriz siguen con `mov` + `op` (no hay forma de
      tres operandos)
- [x] **T3 -- `IndexPtr` sin pila** (`c833ace2`): con el indice en la matriz,
      `lea rax, [rax + r12*s]`; con indice sin pila y escala > 1, la base a
      rdx. La escala 1 con indice general sigue por la pila, a proposito (+1 B)
- [x] **T4 -- el metro ve la memoria** (`8b775d35`): tercer numero, `accesos`
      (pila + marco + memoria), trinquete como los otros dos; `lea` ya no
      cuenta como acceso
- [ ] **T5 -- DOOM pasa por el metro.** Hoy esta fuera del arbol y del banco;
      encogio un 12,7 % desde el 18-09 y NADIE lo ha visto correr. Hoja del
      metal del 18-09, seccion 3b

** Y lo que salio del censo POR PATRON (19-09, `--caliente` volcando todas
las cuentas y sumando por bytes de opcode), que no estaba en esta lista:

- [x] el prologo cargaba cada parametro en rax "por si algun dia" (4,9 %), y
      guardaba los cuatro registros de la matriz aunque usara uno (`8b775d35`)
- [x] el bucle ROTADO: la condicion abajo y el `jmp` de vuelta (6,1 %)
      desaparece; y al rotar salio un FALLO: `break` en `do ... while` no
      rompia (`8b775d35`)
- [x] `x = constante` directo al marco o a la matriz, recortada al compilar
- [x] los argumentos de un intrinseco directos a su registro: el envoltorio
      de la puerta pasa de 31 a 13 instrucciones (`c833ace2`)

Cierre de la manana del 19-09: 451.306 -> 197.052 instrucciones (**-56 %**
desde el 18-09), accesos 40.737, y el reparto sigue plano: `jcc` 9 %, el
recorte de T1 6 %, `lea [rip]` de los globales 5 %, `push` de argumentos de
llamada 4 %, `mov rax, rN` 4 %. Lo siguiente grande no era una instruccion:
era la convencion de llamada, y es la seccion 11.

[!] Lo que la seccion 3 llamaba S1-S5 sigue en pie como forma (la libreria de
codificacion con contrato). Lo del 18-09 se hizo SIN ella, en `decidir/` +
`operando.rs` + `en_sitio.rs`, y funciono porque cada decision es pura y el
banco la juzga. `emit_lea` es el primer codificador general que nace con
clientes: es el embrion de S2.

---

# 11. LA CONVENCION DE LLAMADA HIBRIDA (19-09), Y LO QUE LE FALTA

Hasta el 19-09 BMO C pasaba TODOS los argumentos por la pila y el prologo
los copiaba uno a uno a su hueco. Eso era el 4 % de `push` y otro tanto de
`mov` que el censo por patron veia y ninguna instruccion suelta arreglaba. El
commit `0e84c825` lo cambia, y la forma importa mas que el numero porque es
la que INTI va a copiar (seccion 12):

```text
   escalares y punteros    rdi  rsi  rdx  rcx  r8  r9      en ese orden
   el septimo en adelante  la pila, de derecha a izquierda
   structs y flotantes     la pila, siempre (no hay xmm en la convencion)
   una VARIADICA           TODO por la pila: `va_arg` es `*ap++` y no cambia
   su direccion            NO compila: por un puntero nadie sabria
```

Y las tres piezas que hacen que la convencion GANE en vez de solo cambiar
de sitio el coste:

- **los argumentos van directos** (`argumentos.rs`): constante, variable o
  direccion de funcion se cargan en su registro sin pasar por rax; una
  expresion sin pila hace `mov`; solo lo que llama a alguien aparca en la
  pila. El de rcx va el ultimo porque rcx es el scratch del operando derecho
- **el REENVIO** (`decidir/reenvio.rs`): `return destino(params, constantes)`
  no tiene marco. Baraja los registros (un ciclo pasa por r11) y salta con
  `jmp`, o pone los bytes del intrinseco y `ret`. `bmo_valor` 31 -> 13
- **la RESIDENCIA** (`registros::pisa_argumentos`): si el cuerpo no llama, no
  copia structs y no pone a cero, rdi y rsi se quedan donde llegaron --
  recortados a su tipo en la entrada, nunca si se toma su direccion

Y de rebote: `p->c = v`, `*p = v`, `v.c = x` escriben con la direccion en
rdx y el desplazamiento dentro del `mov` (7 -> 3), y 17 emisores que solo
sabian de r12-r15 aprendieron REX entero.

El metro: 197.052 -> **183.875** instrucciones (-6,7 %), accesos 40.737 ->
**29.358** (-28 %), codigo 128,6 -> 124,4 KB; `sonda_C.c` 5.872 -> 964
accesos (-84 %). DOOM 763.672 -> 742.168 B. Las 25 salidas identicas. Desde
el 18-09: **451.306 -> 183.875, -59 %**.

** Y lo que le falta, medido antes de tocarlo:

- [ ] **C1 -- el metal.** Todo esto compila y corre en el emulador, y ningun
      CPU lo ha ejecutado (LEY 24). La foto es DOOM en el Ryzen con la hoja
      del 18-09, seccion 3b: si el `[perf]` sale y se juega, la convencion
      esta viva. Va ANTES que C2-C4: no se apila mas encima de lo que el
      metal no ha visto
- [ ] **C2 -- residencia en r8 y r9.** Hoy solo rdi y rsi se quedan; rdx y
      rcx son scratch y no pueden, pero r8/r9 no se le deben a nadie y el
      quinto y sexto parametro se vuelcan sin motivo. Pide un `--caliente`
      que diga cuantas funciones hoja tienen 5-6 parametros
- [ ] **C3 -- el troquel y la residencia se pelean.** Un parametro con 6+
      usos ponderados va a la matriz (`mov r12, rdi` + push/pop de r12)
      cuando ya estaba en rdi gratis. Con residencia posible, el troquel
      deberia dejarlo en paz. Medir cuantos casos son
- [ ] **C4 -- reenvio con argumentos de pila.** `return f(a, b, c, d, e, g,
      h)` con 7 no es reenvio; y tampoco `return f(a, s)` con un struct. Son
      pocos; se hacen si el censo los cuenta
- [ ] **C5 -- INTI pasa por el metro.** Los 25 programas son C, C++, COBOL y
      Ada: INTI tiene seis `.ibx` en el build y ningun trinquete. Es lo
      primero de la seccion 12, porque sin metro cada paso de INTI se elige
      por corazonada

---

# 12. EL 50/50 DE INTI: el frontend MAESTRO y el REGISTRO

> Eddi, 19-09: *"el INTI eso vamos a ver luego, pero aisla como se hace en
> 50/50 el frontend MAESTRO y el REGISTRO de INTI, que INTI tiene que hablar
> con la CPU literalmente por registro por algo."*

Esta seccion no es codigo ni casillas de hoy: es la forma, aislada, para
que cuando toque no haya que reinventarla. Y la forma ya existe dos veces
en el arbol: en BMO C desde el 19-09 (`decidir/` no sabe un byte;
`operando.rs`, `argumentos.rs` y `frame.rs` no saben un motivo) y en INTI
desde el 19-08 (`marco.rs` dice *"la IR habla de `Local(3)` y `Temporal(7)`:
indices sin sitio"*, y `inti.toml` da los registros POR ROL). Lo que falta
es escribir la frontera con las palabras justas y ver que cada pieza de la
seccion 11 cae en un lado y solo en uno.

## 12.1 La frase que parte en dos

```text
   EL FRONTEND MAESTRO dice HECHOS sobre el arbol.
   EL REGISTRO dice NOMBRES sobre la maquina.
   Y lo que cruza es un hecho hacia abajo -- nunca un nombre hacia arriba.
```

Un hecho es algo que se comprueba mirando SOLO el fuente: *este parametro
es un escalar*, *a esta local le toman la direccion*, *esta funcion llama a
alguien*, *este valor nace aqui y muere aqui*, *esta funcion solo reexpide
sus parametros*, *este nombre se usa 14 veces y 12 dentro de un bucle*. Un
nombre es *rdi*, *r12*, *el cuarto va en r10 porque `syscall` machaca rcx*.
El frontend de INTI tiene PROHIBIDO por test nombrar una maquina; el
emisor no tiene por que recorrer el arbol. La frontera es exactamente esa.

## 12.2 Cada pieza de la seccion 11, a su lado

| pieza (BMO C, 19-09) | el HECHO (frontend MAESTRO) | el NOMBRE (REGISTRO x86-64) |
|---|---|---|
| clasificar (`decidir/llamada.rs`) | `de_registro[i]`: el i-esimo es escalar o puntero; `variadica` | `[registros] argumento orden 1..6`; el resto a la pila |
| argumentos directos (`argumentos.rs`) | `sin_pila(e)`: evaluar `e` no llama a nadie ni aparca | `mov reg, imm` / `mov reg, [rbp+off]` / push+pop |
| recibir (`frame.rs`) | `contar(cuerpo, p) == 0`: el parametro no se nombra | el hueco negativo, o nada |
| el troquel (`decidir/registros.rs`) | el PESO de cada nombre (usos x profundidad de bucle), `tomadas` | `[reparto] preservados_en_uso`, umbral 6 |
| la residencia (`pisa_argumentos`) | `pisa`: el cuerpo llama, copia agregados o pone a cero | cuales son `argumento` y no scratch (rdi, rsi; no rdx, rcx) |
| el reenvio (`decidir/reenvio.rs`) | `reenvia`: el cuerpo es `devuelve f(params, constantes)`; el orden de las fuentes | `bailar` sobre `argumento orden`, el temporal r11 (`libre`), `jmp` |
| el recorte de entrada | el ANCHO del tipo del parametro (`disposicion` ya lo sabe) | `movsxd` / `movzx` / nada |

Ni una fila tiene un registro a la izquierda ni un `Stmt` a la derecha. Eso
es el 50/50: no *la mitad de las lineas en cada crate*, sino **que cada
decision tenga UN dueno** y que se pueda probar en su lado sin el otro
(`clasificar` tiene tres tests sin emisor; `bailar` cuatro; `marco.rs` se
prueba con `RESPALDO` sin la tabla).

## 12.3 Como se hace en INTI, en orden

Hoy `FuncionIr` trae `parametros`, `locales`, `medidas_locales`,
`temporales`, `instrucciones` -- cuantos, que miden, y el cuerpo. Los
HECHOS de la tabla de arriba se calculan desde ahi, en el frontend, y
viajan como campos de la IR. `marco.rs` los lee y reparte por rol.

- [ ] **I0 -- INTI en el metro** (= C5). Seis `.ibx`, tres numeros y las
      salidas fijas. Sin esto no hay I1
- [ ] **I1 -- los hechos en la IR.** `FuncionIr` gana `pisa: bool`,
      `tomadas: Vec<Local>`, `peso: Vec<u32>` (por local), y
      `reenvia: Option<Reenvio>`. Calculados en `ir/`, probados en `ir/`
      sin un emisor. El test que prohibe nombrar la maquina sigue en pie y
      es el que vigila que ningun campo nuevo sea un numero de registro
- [ ] **I2 -- las LOCALES en registro** (ESTADO.md 2.3, *"lo que sigue en
      el marco"*). `marco.rs` reparte `preservados_en_uso` entre las locales
      con mas `peso` que no esten en `tomadas`, con el umbral de C (6) hasta
      que el metro de INTI diga otro. Es el troquel, y el asignador lineal
      de temporales no cambia
- [ ] **I3 -- la residencia.** Con `pisa == false`, los parametros 1-2 no
      bajan al marco: `marco.local(Local(i))` contesta `Sitio::Registro(
      argumento[i])`. Hoy el prologo baja los seis SIEMPRE (`funcion.rs`:
      *"lo primero que hace toda funcion es bajarlos"*); esa linea es la que
      cambia y ninguna otra
- [ ] **I4 -- el reenvio.** `reenvia` en la IR; en el emisor, `bailar` es
      la misma funcion que en C (`decidir/reenvio.rs` no depende de nada de
      C: es una barajada de u8 y puede vivir en `sem-asm` junto a la tabla)
- [ ] **I5 -- la tabla dice el scratch.** `inti.toml` sabe `trabajo`,
      `argumento`, `libre`, `preservado`; la residencia necesita saber que
      rdx y rcx son argumento Y scratch (la division cae en rdx, el operando
      derecho en rcx). Es una nota en la tabla (`scratch = true`), no un
      `if` en el emisor

** Y lo que NO se copia de C: C recorta el `int` en cada operacion (T1, 6 %)
porque su invariante es *"el registro guarda el valor extendido"*; INTI
comprueba el desborde por regla (Regla 1, `jo`) y su entero es de 64 bits,
asi que ese coste no existe alli. La forma se copia; el precio no.

