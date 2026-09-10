# PLAN EL NEUTRO VIGILADO -- que algo procese el DMA aunque la CPU no mire

> Propuesta del dueno, **2026-09-09**:
>
> > *"prepara la estrategia con DMA, eso BMO-X aunque la CPU no vigile pero si
> > algo DMA pueda ser procesada por algo. Prepara las propuestas y dime que
> > seguirian."*
>
> [!] **Esto es una propuesta y no toca ni una linea todavia.** Lo que trae es
> el inventario de lo que YA hay --que es mas de lo que parece--, el hueco
> exacto con su numero de linea, y cinco escalones con su precio cada uno.

---

# 1. *** LA PREGUNTA SON DOS, Y SOLO UNA TIENE UNA PIEZA DE HARDWARE

Todo el mundo --incluido este proyecto hasta hoy-- habla del DMA como una sola
pregunta. Son dos, y separarlas es la mitad del trabajo:

```text
   DONDE puede escribir un aparato      -> lo contesta una IOMMU
   CUANDO puede escribir                 -> NO lo contesta ninguna IOMMU
```

*** Un aparato con DMA en vuelo sobre un bufer **que ya se libero** escribe en
una direccion que la IOMMU considera **legitima**: el mapeo es valido, el
permiso existe, y el dato aterriza encima de otra cosa. La IOMMU dice que si
porque la pregunta que sabe contestar es *"puede este aparato tocar esta
pagina"*, y la respuesta correcta era *"ya no"*.

** Y esto no es teoria: el sintoma que [`EL_NEUTRO`](../identidad/EL_NEUTRO.md)
ya tiene apuntado --**la azul de la purga, el xHC muerto y el asignador colgado
A LA VEZ**-- tiene mucha mas cara de vida de un bufer que de permiso de una
pagina. Tres aparatos distintos rompiendose juntos es lo que pasa cuando el que
reparte memoria se equivoca, no cuando uno de ellos se porta mal.

> Comprar una IOMMU para arreglar un fallo de tiempos es poner una cerradura
> mejor en una puerta que se deja abierta a la hora equivocada.

Por eso este plan tiene los dos ejes, y el del *cuando* va **antes** que el
hardware.

---

# 2. ** LO QUE YA HAY, Y ES MAS DE LO QUE PARECE

Antes de proponer nada se conto lo que existe. Cinco piezas, y ninguna estaba
puesta ahi pensando en esto:

```text
   1. LA ETIQUETA        `Titular::Neutro` (mm/titular.rs:137). Los marcos que un
                         aparato usa para DMA se marcan como suyos, y AHCI, NIC
                         y xHCI lo hacen desde el 2026-09-07
   2. LA PREGUNTA        `phys::titular_de(fisica)` contesta de quien es un
                         marco. Existe, es O(1) y ya se usa
   3. EL PRECEDENTE      `vmm/roja.rs:224`, `es_tabla()`: usa esa etiqueta como
                         PUERTA -- se niega a caminar una direccion cuyo titular
                         dice otra cosa, y hasta razona su escape para
                         `Anonimo`. El molde ya esta escrito
   4. EL CENSO           `NEUTRO/CENSO.txt` lista quien alcanza la RAM por su
                         cuenta, con un guardian en el build desde el 07-09
   5. EL HARDWARE        `plat/placa.rs:261` YA LEE EL IVRS: cuantos IOMMU
                         declara el firmware y donde viven
```

*** O sea que BMO-X **ya tiene el inventario y las etiquetas**. Lo que no tiene
es **la comprobacion**.

---

# 3. ★★★ EL HUECO, CON SU LINEA

`platform/drivers/storage/ahci/src/comando.rs:293`:

```rust
prdt.add(0).write_volatile((buf_phys & 0xFFFF_FFFF) as u32);
prdt.add(1).write_volatile((buf_phys >> 32) as u32);
```

Ahi se le da a un aparato una direccion fisica **sin preguntarle a
`titular_de`**. Dos lineas mas arriba hay una comprobacion --`if buf_phys & 1 !=
0`, o sea la alineacion-- asi que el sitio donde comprobar ya existe y solo
comprueba una cosa de las cinco que se pueden comprobar.

Lo mismo en `xhci/src/lib.rs:217-221` con los TRB, y en `net`.

```text
   lo que se comprueba hoy   que la direccion sea par
   lo que NO                 de quien es el marco
                             que addr+len no se salga del marco
                             que no envuelva a 32 bits
                             que el aparato sea el dueno, y no OTRO aparato
```

** Y no hace falta hardware para ninguna de las cuatro: son aritmetica y una
lectura de tabla.

---

# 4. LOS CINCO ESCALONES

De mas barato a mas caro. **Cada uno vale por si solo** -- si el proyecto se
para en el segundo, lo hecho sigue sirviendo.

## N-A. EL CENSO SE ALIMENTA SOLO

`NEUTRO/CENSO.txt` se escribe **a mano**, y su propio fichero lo declara como
deuda. Pero hay que ser exacto, porque **la mitad ya esta pagada**:

```text
   R5a  codigo <-> censo     [x] HECHO el 07-09. `censo-neutro` corre en el
                             build, comprueba cuatro cosas y sabe decir que NO
   R5b  censo  <-> maquina   [ ] ABIERTO. Hay que ARRANCAR para verla, y el
                             build no puede ver el bus PCI: corre en el anfitrion
```

*** O sea que hoy se comprueba que **el censo cuadra con el CODIGO**, y no que
cuadre con **la MAQUINA**. `dev/portero.rs` recorre el PCI en el arranque y ya
sabe quien hay; no alimenta el censo, asi que un aparato que este en la placa y
no en la lista **no lo ve nadie**. Este escalon es R5b, ni mas ni menos.

```text
   [cuesta]  DATO -- una lista que miente sobre quien alcanza la RAM
   [juez]    el guardian del censo, que ya corre en el build
```

**[!] SU SACRIFICIO (L3):** un aparato que aparezca a mitad de ejecucion --una
tarjeta en caliente-- no estara en un censo tomado al arrancar. Se acepta: hoy
esta maquina no tiene hot-plug, y **decirlo es mejor que fingir que la lista es
continua**.

## N-B. ★★ EL JUEZ DE DESCRIPTORES -- `bmo-dma-juicio`

**El escalon con mejor relacion entre lo que da y lo que cuesta.** Una funcion
**pura** por la que pasa toda direccion fisica antes de entrar en un descriptor
de aparato:

```text
   el marco es `Titular::Neutro`?              si no, es memoria de otro
   es del APARATO que va a escribir?          si no, un aparato pisa a otro
   addr + len se queda dentro del marco?      el desbordamiento clasico
   la alineacion es la que pide el aparato?   ya se comprueba a medias
   addr + len envuelve a 32 bits?             el PRDT es de 32+32
```

Cinco preguntas, cinco comparaciones. Y **se prueba en el anfitrion**, con
filas que saben ponerse rojas -- que es como esta casa prueba todo lo demas
(`bmo-fisica-juicio` y `bmo-mmio-juicio` ya existen y son exactamente esto para
otras dos preguntas).

```text
   [cuesta]  MAQUINA -- por herencia: no toca hardware, pero quien lo llama
             DECIDE con su respuesta si programa el aparato
   [riesgo]  ESPEJO -- la misma pregunta la haran tres drivers, y si cada uno
             la escribe a su manera se separan. Por eso es UN crate
   [juez]    bmo-dma-juicio, con sus filas en el banco
```

**[!] SU SACRIFICIO (L3):** un aparato que legitimamente necesite escribir en
un marco que no sea suyo --si alguna vez lo hay-- dejara de funcionar hasta que
alguien le de una fila. Ese es el precio de un vocabulario cerrado, y es el
mismo que la casa ya paga con `COSTES` y `RIESGOS`.

★ **Y esto es lo que contesta la pregunta del dueno**: *"aunque la CPU no
vigile, que algo lo procese"*. Ese algo es el juez, y procesa **antes** de que
el aparato exista para el problema.

## N-C. LA VENTANA, Y EL REBOTE

Que todo bufer de DMA salga de **una region declarada**. Entonces "esta
direccion es legitima" son dos comparaciones contra los limites de la ventana,
y un aparato que se vuelva loco corrompe **la ventana** y no el kernel ni otro
proceso.

Es lo que hacen los sistemas sin IOMMU, y tiene nombre desde hace veinte anios
(*bounce buffer*).

```text
   [cuesta]  TAREA -- lo que se pierde es velocidad, no correccion
```

**[!] SU SACRIFICIO (L3), y es CARO y hay que decirlo:** lo que no nazca dentro
de la ventana hay que **copiarlo** a ella y copiarlo de vuelta. Eso es un
`memcpy` por cada I/O que hoy no existe. Para el disco puede compensar; **para
el camino de la mano al pixel probablemente no**, y ese es el que manda.

*** Por eso N-C va DESPUES de N-B y no antes: el juez no cuesta ancho de banda,
la ventana si. Y si el juez basta, la ventana no se construye.

## N-D. ★★★ EL RELOJ DEL BUFER -- el eje del "CUANDO"

**Es el escalon que ninguna IOMMU sustituye, y el que mas se parece al fallo
que esta casa tiene apuntado.**

Un marco con DMA en vuelo **no se puede liberar, ni desmapear, ni reasignar**
hasta que el aparato diga que termino. Hoy nada lo impide: `Titular::Neutro`
dice de quien es el marco, no si **hay algo volando hacia el**.

```text
   lo que falta   un estado EN VUELO en el marco, puesto al programar el
                  descriptor y quitado al consumir la interrupcion de fin
   lo que compra  que `reap`, la purga y el asignador no puedan tirar de un
                  marco que un aparato todavia esta escribiendo
```

** Y encaja con lo que ya hay: `Titular` es un `u8` por marco con su cuenta en
O(1); un bit mas es una casilla mas del mismo mapa.

```text
   [cuesta]  MAQUINA -- un marco liberado a destiempo es corrupcion silenciosa
   [riesgo]  AJENO -- lo que se lee para bajar la bandera viene del aparato
   [juez]    la cuenta: marcos EN VUELO al apagar tiene que ser CERO, igual
             que `soltados` en el censo del neutro (N3)
```

**[!] SU SACRIFICIO (L3):** un aparato que se cuelgue sin dar su interrupcion
de fin deja marcos en vuelo **para siempre**, y eso es una fuga. Hace falta un
plazo, y un plazo es una decision --*cuanto se espera antes de dar por muerto a
un aparato*-- que no se puede tomar sin medir. **Esa es la parte dificil de
este escalon, y no es el bit.**

## N-E. AMD-Vi -- la unica que de verdad OBLIGA

Los cuatro de arriba son **disciplina**: el kernel se comprueba a si mismo
antes de programar un aparato. Un fallo en el propio kernel se los salta todos.
La IOMMU es lo unico que pone la comprobacion **fuera del alcance del
software**: el aparato tiene sus propias tablas de pagina y el silicio las
hace cumplir.

```text
   lo que hay ya   `plat/placa.rs:261` lee el IVRS: cuantos hay y donde
   lo que falta    construir y mantener un segundo juego de tablas de pagina,
                   su cache, y su invalidacion
```

```text
   [cuesta]  MAQUINA
   [riesgo]  ARRANQUE -- si esto se programa mal, la maquina no arranca
```

**[!] SUS TRES SACRIFICIOS (L3):**

```text
   1. ES UN PROYECTO DEL TAMANO DEL VMM, no un `if`. Tablas, cache de
      traducciones del aparato, invalidacion, y una cola de comandos
   2. LA VENTANA DEL ARRANQUE. La IOMMU tiene que estar programada ANTES de
      que ningun aparato haga DMA, o sea antes de AHCI y del xHC. Y entre que
      el firmware suelta la maquina y BMO-X la programa hay un hueco en el que
      nadie vigila. Ese hueco no se cierra: se acorta y se DECLARA
   3. CUESTA LATENCIA. Cada acceso del aparato pasa por una traduccion, y su
      cache falla igual que un TLB. No es gratis ni siquiera cuando funciona
```

[!] Y se lee bajo la ley de la casa sin discutir: el IVRS es una **tabla ACPI
estatica**, no AML. *"Tablas estaticas SI, AML NUNCA."*

---

# 5. [!] LO QUE NI CON LOS CINCO SE ARREGLA

Un plan que no diga donde acaba es propaganda.

```text
   el microcodigo del CPU   `NEUTRO/CENSO.txt` ya lo declara IMPOSIBLE: no
                            pide memoria a este asignador, ya esta dentro
   el modo SMM del firmware igual: no la pide, se la toma
   DMA entre dos aparatos   PCIe permite que dos aparatos hablen sin pasar por
                            la RAM. Una IOMMU puede prohibirlo, pero hay que
                            configurarlo Y la placa tiene que dejar
   un aparato con firma      un aparato que hace exactamente lo que le pidieron,
                            en un momento en que no debia. Eso es N-D, y N-D es
                            software: si el bit se pone mal, la IOMMU no salva
```

> El objetivo no es un sistema donde el DMA no pueda hacer dano. Es uno donde,
> cuando lo haga, **se sepa cual y cuando** -- que es lo que hoy no pasa.

---

# 6. LOS PASOS

- [x] **N0 y N1 -- HECHOS EL 2026-09-09, y son LA MISMA PIEZA.**

  ** Al escribirlo se vio que N0, tal como estaba redactado --*"hacer que todo
  pase por una funcion"*--, **es una regla que hay que acordarse de cumplir**.
  El decimoquinto sitio entra igual; solo que ademas hay una funcion que nadie
  le obligo a llamar.

  *** La forma que SI se cumple sola es un **TIPO**. `bmo-dma-juicio::Prenda`
  envuelve un `u64` y su campo es privado: **la unica forma de obtener uno es
  `juzgar`**, y los drivers son crates distintos, asi que no pueden construirlo.

  ```text
     el embudo lo cuenta el COMPILADOR, no un grep
     rodearlo no es "olvidarse": es un error de tipos
     y el numero de constructores es 1 POR CONSTRUCCION
  ```

  ** Y el juez no comparte vocabulario con nadie: recibe un `bool` en vez de
  `Titular::Neutro`, y un `u16` que solo compara consigo mismo en vez de un enum
  de aparatos. **No tiene `[riesgo] ESPEJO` porque no hay copia.**

  Seis preguntas, 16 filas, la mitad para decir que NO. La sexta salio
  escribiendo las pruebas: `bytes == 0` pasa las otras cinco.

  Se verifica: `cargo test -p bmo-dma-juicio` -- 16 filas, y `Prenda(0)` desde
  fuera del crate no compila.

- [x] **N0-original -- el enunciado, conservado.** Antes de escribir ningun
  juez, hacer que **toda** direccion fisica que va a un aparato pase por UNA
  funcion. Hoy son tres sitios en tres crates (`ahci/comando.rs:293`,
  `xhci/lib.rs:217`, y `net`), cada uno con sus propias comprobaciones o
  ninguna. Sin el embudo, un juez es un juez al que se puede rodear. Se
  verifica: un censo dice cuantos sitios escriben una direccion fisica en un
  descriptor, y ese numero tiene que ser **1**.

- [x] **N4 -- EL BIT EN VUELO. HECHO el 2026-09-09.**

  ** Vive en el **nibble alto del byte del titular**, y no hizo falta ni una
  tabla nueva: `Titular` nunca paso de 8, asi que los cuatro bits de arriba
  llevaban libres desde el primer dia -- y ese byte ya se leia y escribia en un
  solo sitio (`marcar`), que es lo que lo hace seguro.

  ```text
     bits 0..3   el titular    0..8
     bits 4..7   EL APARATO    0 = nada en vuelo, 1..15 = quien
  ```

  Cuatro funciones (`en_vuelo`, `aterrizo`, `en_vuelo_de`, `vuelos`), los cuatro
  aparatos numerados **por las filas de `NEUTRO/CENSO.txt`**, y tres cuentas:

  ```text
     vivos     lo que hay ahora. Al apagar, CERO
     pisados   ** CERO. Un marco que cambia de titular con DMA dentro
     choques   ** CERO. Dos aparatos pidiendo el mismo bufer
  ```

  ★ **`pisados` se detecta desde el lado del MARCADO**, no en el camino de
  devolucion. Es la misma tecnica que este fichero invento para `NEUTROS_SOLTADOS`
  y por el mismo motivo: ese camino es rojo, es donde vive la azul del 07-09, y
  R4 dice que no se toca hasta reproducirla. **Se puede saber que una regla se
  rompio sin ponerse delante de ella.**

  [!] Y NO IMPIDE NADA: cuenta y dice. Negarse a marcar desde el camino rojo con
  un dato que aun no se ha ganado la confianza dejaria la maquina sin memoria --
  peor que el fallo que evita. Primero el numero; la barrera, cuando el numero
  lleve arranques diciendo cero.

  ** Cableado en el AHCI, en `mandar_lectura`, que es el embudo de las DOS
  lecturas. Se verifica: `run c/ciclos.bex` no basta -- hace falta que CABINA
  ensene `vuelos()`. Esa fila es N4b.

- [x] **N4b -- HECHO. Las tres cuentas se ven en CABINA**, pegadas a `neutro=` porque son la misma pregunta en dos tiempos: `vuelo=V:P:C`. Y `pisados`/`choques` mandan sobre el color por delante de la RAM baja, por la misma razon que `soltados`. Lo que falta ahora es UN ARRANQUE que diga si `vivos` llega a cero al apagar.

- [x] **N4-original -- el enunciado, conservado.**

  *** El orden de este plan estaba MAL, y lo demostro intentar cablear el juez.
  `dev/disk/transfer.rs:64` --el camino DIRECTO de una lectura-- le da al disco
  la direccion fisica del **bufer del que llamo**, que no es del aparato y no
  tiene por que serlo. **El juez con una sola regla habria rechazado una lectura
  legitima y dejado el disco sin funcionar.**

  Asi que `Marco` gano un segundo caso --`en_vuelo_para: Option<u16>`-- y ese
  campo **no lo puede rellenar nadie hoy**: saber que un marco esta prestado a
  un aparato Y AHORA es exactamente el bit en vuelo.

  ```text
     N-B (el DONDE) necesita un dato de N-D (el CUANDO)
     -> los dos ejes NO eran independientes, y esto lo demuestra
  ```

  Lo que hace falta: un estado mas en `Titular`, puesto al programar el
  descriptor y quitado al consumir la interrupcion de fin. Se verifica: la
  cuenta de marcos en vuelo al apagar es **CERO**, igual que `soltados` en N3
  del censo del neutro.

- [x] **N2 -- HECHO el 2026-09-09, despues de N4 como el propio plan corrigio.**

  El juez se pregunta **PAGINA A PAGINA**, y eso no es celo: es lo unico que
  hace que el juicio no sea circular. El `Marco` sale del ASIGNADOR --titular,
  quien lo tiene en vuelo, y que una pagina mide una pagina-- y no de lo que
  diga el que pide.

  ** Y `Peticion` gano un campo mas al cablearlo: `prestando`. `en_vuelo_para`
  es un HECHO sobre el marco; lo que justifica el prestamo no es un hecho, es
  que **alguien con derecho lo cede**.

  [!] **Y solo se RECHAZA uno de los seis vetos: `DeOtroAparato`.** Esto es el
  camino del disco, o sea el del arranque: una regla mal afinada aqui no da un
  aviso, deja la maquina sin poder leer su propio sistema. Los otros cinco se
  cuentan en `DMA_VETOS` y se dicen. Es la forma de `vmm::es_tabla`, que deja
  pasar `Anonimo` por el mismo motivo.
  Hoy el juez existe y **no lo llama nadie**. Ponerlo en el camino pide cambiar
  la firma de los que escriben descriptores para que pidan `Prenda` en vez de
  `u64`, y eso toca `platform/drivers/{ahci,xhci}` y quien los llama en Ring 0
  --que es el que sabe `titular_de`--.

  [!] **Ese es el trabajo de verdad, y no se ha hecho.** El crate sin cablear
  vale igual --las 16 filas son un contrato escrito y probado-- pero **no
  vigila nada todavia**, y decirlo importa mas que tenerlo. Se verifica: una
  direccion fuera de un marco `Neutro` no llega al aparato, y CABINA lo dice.

- [x] **N2-original -- MOVIDO ARRIBA, delante de N3.** Decia *"es una linea"*,
  y lo es -- pero solo despues de N4. Ver su casilla.

- [x] **N3 -- HECHO el 2026-09-09. El censo contra la maquina, por un BIT.**

  La pregunta del censo no es *cuantos aparatos hay*: es **quien puede escribir
  en la RAM por su cuenta**. Y PCI la contesta con el bit 2 del registro
  Command --BUS MASTER ENABLE--, que es la tercera condicion de `FRONTERA.txt`
  **leida del silicio en vez de escrita a mano**.

  ** Y lo que puede descubrir no es solo un olvido nuestro: un BME encendido
  que este kernel no encendio es **la ventana del arranque** que
  `IOMMU_MAESTRO.md` describe, medida en vez de supuesta.

  [!] `APARATOS_CENSADOS` es una COPIA del censo, y lleva juez a proposito:
  `censo-neutro` comprueba en cada build que diga lo mismo que las filas, y
  esta probado que sabe decir que no. Es la leccion de R19 aplicada **el mismo
  dia que se aprendio**.

  Se verifica: en el arranque, `maestros del bus, y el censo los conoce a
  todos`. Si sale el aviso, hay un aparato con DMA que nadie censo.

- [x] **N4 -- MOVIDO ARRIBA el 2026-09-09.** Estaba aqui, detras de N2 y N3,
  y el intento de cablear el juez demostro que va DELANTE. Ver su casilla.

- [ ] **N5 -- EL PLAZO DEL APARATO MUERTO.** Cuanto se espera antes de dar por
  perdido un DMA en vuelo. **No se elige: se mide** -- el peor tiempo de
  respuesta real de AHCI y del xHC en esta placa, con margen. LEY 24.

- [ ] **N6 -- LA VENTANA Y EL REBOTE** (N-C), *si* N1..N5 no bastan. Se decide
  con un numero, no con una opinion: cuantas veces el juez ha dicho que no en
  un arranque normal. Si la respuesta es cero, la ventana no hace falta.

- [ ] **N7 -- AMD-Vi** (N-E). El ultimo, y con su propio plan cuando llegue.
  No se empieza sin N4 hecho: **una IOMMU sobre un sistema que no sabe cuando
  un bufer esta en vuelo arregla el eje equivocado**.

---

# 7. QUE SEGUIRIA, EN UNA FRASE CADA UNO

```text
   N0  hacer que solo haya UN sitio por donde salir
   N1  escribir el que dice que no
   N2  ponerlo en el camino
   N3  que la lista de aparatos deje de escribirse a mano
   N4  saber CUANDO un marco esta ocupado, que es el eje que falta
   N5  y cuanto se espera antes de rendirse
   N6  encerrarlos en una ventana, si hace falta
   N7  y solo entonces, el hardware
```

** Los cuatro primeros **no tocan hardware, no cuestan ancho de banda y se
prueban en el anfitrion**. Ese es el argumento entero para hacerlos antes: son
baratos, son comprobables, y **si el fallo que esta casa tiene apuntado es de
tiempos, lo caza N4 y no N7**.
