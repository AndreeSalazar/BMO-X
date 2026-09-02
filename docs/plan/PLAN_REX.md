# REX -- la puerta de los terceros, ORDENADA

> La ley esta en [`META-SDK_HARD.md`](../../META-SDK_HARD.md) y el indice en
> [`tables/bmo/README.md`](../../toolchain/forge/sem-asm/tables/bmo/README.md).
> **Esto es otra cosa: es el ORDEN.** Donde va lo proximo, que NO entra nunca, y
> que forma tiene una cabecera para que la numero 11 se parezca a la numero 1.

Se escribe el 2026-09-01, al dia siguiente de que el semaforo (L6g) llegara a
REX, y por un motivo concreto que dijo el dueno: *"no quiero fideos
desordenados"*. Sin esto, las cinco cabeceras que faltan entran una a una segun
vayan haciendo falta, y en tres semanas REX es un cajon.

---

## Lo que YA es verdad, para no reconstruirlo

| | |
|---|---|
| **Dos puertas y solo dos** | `INVOKE` y `WAIT`. Congeladas desde el 2026-08-10; el numero 1 sigue RESERVADO |
| **REX existe y tiene nombre** | 10 cabeceras publicas, 18 ficheros, 3.015 lineas. Bautizado el 18-08 |
| **La cabecera trae el cuerpo** | No hay `libbmo.so` porque no hay enlazado dinamico |
| **Se puede tapar sin bifurcar** | `$BMO_MODS` -> `mods/` -> `tables/`. Gana el primero que tenga el fichero |
| **El semaforo** | 18 de 18 con `[carril]`, `[cuesta]` y `[riesgo]`. ROJO 9, AMARILLO 5, VERDE 4 |
| **Cuatro partidas por dentro** | `bmo/`, `archivo/`, `monton/`, `superficie/`, con fachada. `#include` intacto |
| **El juez** | `contrato.py` R11 y R12. `--autoprueba`, 50 casos |
| **Los numeros COINCIDEN hoy** | 74 parejas C <-> `bmo-abi`, cero desacuerdos. Medido el 01-09 |

---

## ** LA REGLA QUE EVITA EL CAJON: lo que NO entra en REX

Esto va primero a proposito. Una lista de lo que falta invita a meterlo todo;
lo unico que para eso es una frontera escrita **antes** de tener la prisa.

> **REX es la superficie de una APP, no la del sistema.** Si la operacion solo
> tiene sentido para el escritorio, el explorador o un panel del kernel, **no es
> de REX aunque este en el ABI.**

El ABI declara 337 constantes. De ellas, **134 no son superficie de app** y no
van a tener cabecera:

| familia | cuantas | de quien es |
|---|---|---|
| `DISCO_*` mascaras | 57 | los paneles. Un programa lee el disco por `BMO_INFO_DISCO_*`, que SI esta |
| `ES_*` | 39 | ESTRATOS y su explorador -- un navegador de ficheros, no una app cualquiera |
| `CABINA_*` | 17 | la cabina del kernel |
| `USB_SALUD_*` | 11 | el panel de salud del USB |
| `AUTOPSIA_*`, `KLOG_*`, `SYSCALL_*` | 10 | instrumentos de Ring 0 |

(57 + 39 + 17 + 11 + 10 = **134**. Las cuentas se dejan a la vista porque un
total sin sumandos es un numero que nadie puede discutir.)

[!] **Y no es "todavia no": es que no.** Meterlas ensancharia la puerta de los
terceros por gusto, y cada fila de esa puerta es una promesa que hay que
mantener. Si algun dia una app de verdad las necesita, entra por la puerta
normal: se escribe aqui POR QUE, y deja de estar en esta lista.

---

## El mapa: donde va cada cosa

**Las rutas de inclusion son PLANAS y se quedan planas.** `<bmo/archivo.h>`,
`<bmo/pantalla.h>`. Agrupar por carpetas --`<bmo/io/archivo.h>`-- costaria
`PUERTA`: rompe fuentes que ya existen, incluido DOOM. Asi que **el orden vive
en este documento y en el README, no en el arbol de directorios**; lo unico que
el arbol dice es el CARRIL (L6g), que es otra pregunta.

Seis familias. Todo lo que venga cae en una, y si no cae en ninguna hay que
discutir si es de REX:

```
   LA PUERTA      bmo.h  bloque.h              las dos llamadas y el contrato de bloque
   MEMORIA        monton.h                     malloc/free/realloc sobre UN bloque
   FICHEROS       archivo.h  paquete.h         el disco, y los datos dentro del .bex
   PINTAR         superficie.h  [pantalla.h]   en una ventana, o la pantalla entera
   ENTRADA        entrada.h  scroll.h          teclado, raton, y la vista sobre un historial
   SONIDO         sonido.h  musica.h           el derecho a hacer ruido, y las notas
```

** Lo que ese mapa hace visible de un vistazo: **PINTAR esta coja**. Hay
cabecera para dibujar en una ventana y no la hay para tomar la pantalla, que es
el caso mas viejo de los dos.

---

## Lo que falta, MEDIDO

31 constantes en siete familias de app, hoy sin cabecera -- **mas las dos de
`SOLTAR`**, que no se cuentan aqui porque viven en la familia `TASK` y no en
ninguna de las siete:

| familia | cuantas | que desbloquea | paso |
|---|---|---|---|
| `FB_OP_*` | 4 | la pantalla completa | **1** |
| `TAREA_OP_*` | 4 | lanzar un hijo **y esperarlo** | 5 |
| `PRESTADO_OP_*` | 4 | memoria que te presta otro | -- |
| `LIENZO_*` | 5 | formatos de pixel | -- |
| `RED_OP_*` | 6 | `ARMAR`, `SONDEAR` | -- |
| `PLACA_OP_*` | 4 | ECAM, IOMMU, tablas | -- |
| `CONSOLA_OP_*` | 4 | la consola como objeto | -- |

---

# ** LA RECETA -- que es una cabecera de REX

Escrita para que la numero 11 se parezca a la numero 1. Ocho casillas, y ninguna
es opcional:

```
   1  cabecera de doc que dice QUE resuelve y, sobre todo, QUE NO
   2  [carril] [cuesta] [riesgo] con el PORQUE de cada uno   (R11)
   3  guarda de inclusion BMO_<NOMBRE>_H
   4  TRAE EL CUERPO: nada que declare y espere a un enlazador
   5  sus numeros salen del ABI, y no se escriben a mano dos veces (R13)
   6  un ejemplo que compile en toolchain/lang/c/examples/
   7  una fila en tables/bmo/README.md, con su color
   8  si tiene DOS masas, carpeta + fachada             (L6g, R12)
```

[!] La casilla 6 tiene UN incumplimiento conocido y viejo: `entrada.h` no tiene
ejemplo. Lo dice el README desde el 19-08. No se tapa aqui -- se arregla en el
paso 1, que es cuando por fin habra algo que ensenar.

---

# ~~PASO 1~~ -- `<bmo/pantalla.h>` ✅ HECHO (2026-09-01)

## Por que es la primera

No porque falte: porque **ya se esta usando sin existir**. Los cuatro `FB_OP_*`
no aparecen en `tables/` ni una vez, asi que cada programa que toma la pantalla
se los inventa:

```c
   doomgeneric_bmo.c:198   #define FB_BASE   0x01
   raycaster_C.c:75        #define FB_BASE   0x01
```

**Dos copias de un numero del kernel, en ficheros donde ningun guardian mira.**
REX da la llave --`PANTALLA_RECLAMAR` si esta-- y no da la puerta.

## Y lo segundo, que el semaforo destapo el 01-09

```
   BMO_OP_SONIDO_SOLTAR      esta en REX
   BMO_OP_PANTALLA_SOLTAR    NO existe en REX      TASK_OP 0x1D
   BMO_OP_ENTRADA_SOLTAR     NO existe en REX      TASK_OP 0x1E
```

Desde C se puede reclamar la pantalla y el teclado **y no se pueden devolver**.
El sonido si. Y la ironia es que `entrada.h` se etiqueto `[cuesta] APARATO`
porque *"el teclado secuestrado"* es el precedente de esa clase en L6e: la
cabecera que cuesta un aparato es justo la que no sabe soltarlo.

[!] **Las dos operaciones EXISTEN y estan cableadas** en el kernel
(`syscall/mod.rs:257`, `op_aparato::pantalla_soltar`). Esta cabecera no promete
nada nuevo: publica lo que ya se puede hacer. Comprobado antes de escribirla,
que es la regla 1 del proyecto.

## Contenido

| | |
|---|---|
| `FB_OP_BASE` 0x01, `DIMS` 0x02, `STRIDE` 0x03, `BYTES` 0x04 | del ABI, no a mano |
| `bmo_pantalla_reclamar()` / `_soltar()` | y la de entrada, que va con ella |
| `bmo_pantalla_ancho/alto/paso(cap)` | desempaquetar `DIMS` y `STRIDE` es aritmetica que hoy repite cada app |
| carril | **ROJO** -- lo que devuelve es una direccion donde se escribe sin red |

## Casillas

```
   [x] 1.1  <bmo/pantalla.h>, 238 lineas, carril ROJO. R11 la acepta
   [x] 1.2  raycaster_C.c: CUATRO numeros fuera, no tres
   [x] 1.3  examples/pantalla_C.c -- y con el, entrada.h deja de ser la
            unica cabecera sin ejemplo, hueco abierto desde el 19-08
   [x] 1.4  fila en el README, y el indice pasa a ONCE piezas
   [x] 1.5  DOOM: los tres suyos fuera. Compila, 891.704 bytes
```

[!] 1.5 va en `BMO-externo`, que no es repo git. Se hace y se dice; no aparece
en ningun commit.

## ★★ Lo que salio por el camino, y no estaba en la lista

**1. `raycaster_C.c` tenia CUATRO numeros copiados, no tres.** El cuarto era
`#define ENT_TECLA 0x03` -- y `<bmo/entrada.h>`, que lo publica desde siempre,
**ya estaba incluida nueve lineas mas arriba**. O sea que no todos los numeros
copiados vienen de que falte la cabecera: uno venia de no mirarla. Eso es
exactamente lo que el paso 3 (R14) va a cazar.

**2. ★★ `unity.py` compilaba el fichero equivocado, y decia que si.**

Construia `bmo_unity.c` --el agregado de los 81 ficheros de DOOM, que no tiene
`main` porque el punto de entrada vive en la capa de plataforma-- en vez de
`doomgeneric_bmo.c`, que es quien INCLUYE al agregado. El frontend contestaba

```text
   error: no hay funcion 'main': un programa de Ring 3 necesita punto de entrada
```

y el guion lo imprimia como una linea mas y **salia con codigo 0**.

> Un guion que falla y sale con cero no es un guion roto: es un guion que
> MIENTE.

Por eso `doom-port/out/doom.bex` llevaba **desde el 14-08 sin tocarse** --19
dias-- y nadie lo noto: el binario que se desplegaba se construia a mano con la
orden que la cabecera de `doomgeneric_bmo.c` documenta. Arreglado: compila la
entrada correcta y propaga el codigo de salida.

**3. En REX una cabecera se paga por INCLUIRLA, no por usarla.** Medido:

```text
   incluir <bmo/pantalla.h> y no llamar a nada   +1.795 B
   scroll_C, que no llama a bmo_entrada_soltar      +51 B
```

La cabecera trae el cuerpo --no hay `libbmo.so`-- y no hay enlazador detras que
pode lo que nadie llama. ★ **Se descubrio porque lo escribi al reves**: la
primera version de `pantalla.h` decia *"quien no lo usa no lo paga"* citando a
`<bmo/bloque.h>`, y ahi la regla es otra --el CODEGEN no emite las globales si
nadie las declara--. Se midio, salio falso, y la frase esta corregida en la
cabecera. Es una propiedad de REX entera y no estaba escrita en ninguna parte.

---

# ~~PASO 2~~ -- R13, el espejo ✅ HECHO (2026-09-01)

## El problema, con su cita

`paquete.h` lo confiesa por escrito:

> *"los numeros del formato viven en `bmo_abi::bef` y aqui se repiten porque C
> no puede importar de Rust; si algun dia dejan de coincidir, la fila de pruebas
> que empaqueta con la herramienta y lee con esta cabecera es la que lo dice."*

Eso es un `ESPEJO` con un juez que solo cubre UNA fila. Las otras 73 parejas no
las mira nadie. Y es el patron numero 47 del proyecto: **una tabla con dos
lectores, y crecer por uno la rompe para el otro** -- el episodio de
`intrinsics.toml` del 22-08 costo cinco frontends.

## La forma

Una tabla `REX_ESPEJO.txt` al lado de `LINEA_BASE.txt`, con el mismo patron que
ya funciona para los kinds:

```
   FB_OP_BASE            FB_BASE                  la pantalla
   ARCH_OP_LEER_EN       BMO_ARCH_LEER_EN         el camino rapido de fread
```

** **El emparejamiento lo escribe una persona y la comprobacion la hace la
maquina.** Los dos lados se llaman distinto A PROPOSITO --uno habla ingles de
kernel, el otro espanol de app-- asi que un juez que dedujera el par estaria
adivinando. Tablas y no cerebros.

## Casillas

```
   [x] 2.1  borrador: **90 parejas, no 74**. El paso 1 anadio los cuatro
            FB_OP_* y los dos SOLTAR, y cada uno trajo su pareja
   [x] 2.2  revisadas a mano las 90. **CERO discrepancias hoy**
   [x] 2.3  R13 en contrato.py + SEIS autopruebas
   [x] 2.4  sellado en 90, en `toolchain/tools/contrato/REX_ESPEJO.txt`
```

## ★ Y se comprobo que MUERDE, no que compila

Se cambio a mano `BMO_FB_STRIDE` de `0x03` a `0x33` en el arbol de verdad:

```text
   [X] R13 el espejo de REX: el espejo sello FB_OP_STRIDE = BMO_FB_STRIDE =
       0x03, y hoy el ABI dice 0x03 y REX dice 0x33. Cambiar un numero de la
       puerta rompe binarios que YA existen
```

y se deshizo. Una regla que solo se ha visto pasar no se ha visto.

## ★★ Lo que salio por el camino

**1. `INFO_TSC_HZ` y `INFO_TXT_EXT_NOMBRE` valen los DOS `0x05`** (y
`CPU_HILOS` / `TXT_EXT_NOTA`, los dos `0x06`). Parece un choque y no lo es: los
campos de texto entran por `TASK_OP_INFO_TEXTO` (0x14), que es **otra
operacion**, asi que viven en otro espacio de numeracion. Queda escrito en la
nota de esas cuatro filas para que nadie lo *arregle*.

**2. ★★ Los trinquetes de L6e y L6f llevaban desfasados.** Al correr
`--sellar` salieron sus suelos reales:

```text
   [cuesta]   suelo 7  ->  24 ficheros lo declaran
   [riesgo]   suelo 3  ->  20
```

No es un resellado cosmetico. **Un suelo por debajo de la realidad es un
trinquete que no trinca**: con el suelo en 7, diecisiete ficheros de Ring 0
podian perder su etiqueta y el guardian habria dicho que todo bien. Se quedaron
atras cuando Ring 0 gano sus carriles y nadie volvio a sellar.

> Un trinquete que no se resella deja de ser un trinquete y pasa a ser un
> numero viejo con autoridad.

---

# ~~PASO 3~~ -- R14, y de regalo R15 ✅ HECHO (2026-09-01)

Un `.c` no puede llevar un `#define` de una constante que ya vive en el ABI.

**Hoy fallaria dos veces**, y las dos son `FB_BASE`. Por eso este paso va
DESPUES del 1: una regla que prohibe algo sin dar la salida es una regla que se
apaga en una semana.

```
   [x] 3.1  R14 sobre toolchain/lang/c/examples/, con cinco autopruebas
   [x] 3.2  alcance DECIDIDO: solo el arbol propio
```

[!] 3.2 se cierra como decia el plan: un tercero **tiene derecho** a redefinir
un numero --es lo que `$BMO_MODS` promete-- asi que R14 no mira `mods/`. Mira
lo que el proyecto PUBLICA como ejemplo, que es lo que la gente copia.

## ★★ Que comprueba R14 exactamente, y por que no lo obvio

*"El nombre parece del kernel"* seria adivinar, y un guardian que adivina da
permiso con autoridad. Lo que R14 mira es concreto y se ve:

> **el segundo argumento de `bmo_valor`/`bmo_codigo` es un macro que el propio
> fichero define como un numero.**

Ese macro ES una copia de un numero del kernel que nadie compara con el
original. Un `#define` que apunta a un nombre de REX no es pecado: es un alias
legible y el numero sigue viniendo de un sitio solo.

### ★ Y se probo contra el codigo de VERDAD, no contra un test sintetico

R14 corrio sobre el `raycaster_C.c` de `d5bd4abe` --el de antes del paso 1-- y
canto los **cinco** usos reales: `FB_BASE` (dos veces), `FB_DIMS`, `FB_STRIDE`
y `ENT_TECLA`. La regla caza el bug que la motivo, sobre el codigo que lo tenia.

### [!] Y un literal desnudo INFORMA, no falla

`sonda_C.c` llama a `0x7777` y a `0xFFFFFFFF` **a proposito**: su trabajo es
comprobar que el kernel dice que no a lo que no existe. Una regla que le grita
a la sonda de seguridad por hacer su trabajo es una regla que se acaba
apagando. Salen como `[i]`, y ahi se ve el cuarto --`0x1C`-- que **no** es una
sonda: es una operacion viva que REX no publica.

## ★★ R15, que no estaba en el plan: EL ABI REPITE UN NUMERO

Aparecio mirando ese `0x1C`. En `bmo-abi`:

```text
   TASK_OP_TOMAR           0x1C     viva, la implementa el kernel
   TASK_OP_LIENZO_REFLEJO  0x1C     nadie la implementa
```

Misma familia, mismo numero, ninguna nota. `KIND_LIENZO` fue un diseno
**retirado** --salio del kernel cuando el prestamo se hizo generico, y lo
cuentan `obj/loan.rs` y `docs/identidad/LIENZO.md`-- asi que es **una constante
muerta okupando un numero vivo**: quien la escriba invocara `TOMAR`.

** Y este proyecto ya lo pago una vez. Lo dejo escrito el kernel al elegir el
opcode de `PANTALLA_SOLTAR`:

> *"0x1D elegido tras listar los opcodes ORDENADOS, que es la regla desde que
> `MEMORIA_PEDIR` se puso en `0x12` --ya ocupado por `REINICIAR`-- y pedir
> memoria habria reiniciado la maquina."*

`R5` vigila eso en el KERNEL. **Nadie lo vigilaba en el ABI**, que es la lista
que lee quien escribe una app. R15 lo vigila ahora; en todo el ABI hay
exactamente UN choque, y es ese.

### ★ BORRADAS el 2026-09-02, a peticion del dueno

Ocho constantes fuera: `TASK_OP_LIENZO_REFLEJO`, `LIENZO_FMT_*`,
`LIENZO_OP_*`, `LIENZO_UNICO` y `LIENZO_FILAS_RESERVADAS_ARRIBA`. **La lista de
tolerados esta a CERO**, que es su estado correcto -- duro exactamente un dia.

[!] Y **no se reserva el numero**, al reves que con `BMO_CHANNEL_KICK`. Alli la
regla era *"el numero no se recicla"* porque estaba libre; aqui ya estaba
reciclado: el `0x1C` es de `TOMAR` y lo lleva usando desde siempre. La lapida
queda al lado de `TASK_OP_TOMAR`, que es donde hace falta leerla.

** Y R15 gano una exigencia mas: **una tolerancia que ya no hace falta tambien
se dice**. Una deuda saldada que sigue escrita miente igual que una oculta --
la proxima persona la lee y cree que el choque sigue ahi.

### ★★ Y el borrado destapo otra cosa: un byte NUL dentro de un comentario

`surface/tarea.rs` tenia un `0x00` literal en la linea 431, dentro de una frase
que hablaba *de* el: *"en una ruta un `\0` no puede aparecer"*. Alguien
quiso escribir `\0` y metio el byte.

**Consecuencia: `grep` trataba el fichero como BINARIO y lo saltaba en
silencio.** Sin `-a`, todas las busquedas sobre el ABI se dejaban fuera el
fichero de las operaciones de TAREA -- que son 49. Asi es como un fichero se
vuelve invisible para las herramientas sin que nadie lo decida.

Arreglado. Y deja un hueco anotado abajo.

### Lo que se busco despues, y NO estaba

Se barrio el ABI entero buscando mas operaciones muertas: 3 candidatas
(`PRESTADO_OP_BASE`, `_BYTES`, `_SOLTAR`) y **las tres son falsa alarma** -- el
kernel las implementa con otro nombre (`loan::OP_BASE`, `OP_BYTES`, `OP_SOLTAR`)
y los mismos valores. **No quedan constantes muertas en el ABI.**

---

# ~~PASO 4~~ -- R16, la cobertura ✅ HECHO (2026-09-02)

Cuantas operaciones del contrato tienen funcion en REX. Hoy **74 de 337**, y de
lo que es de app faltan 31 en siete familias.

Convierte *"REX no tiene X"* de sensacion en cifra con trinquete. Y el
denominador no son las 337: son las 203 que quedan tras quitar las 134 que la
frontera de arriba excluye -- **un porcentaje contra un denominador inflado es
una forma elegante de mentirse**.

```
   [x] 4.1  FRONTERA_REX.txt -- ocho prefijos, cada uno con de quien es
   [x] 4.2  R16 con trinquete en COBERTURA.txt, sellado en 93
```

**REX cubre 93 de las 194 constantes del ABI que son de app -- el 47%.** La
frontera deja fuera 137, y por eso el denominador es 194 y no 331: un
porcentaje contra el total contaria como pendiente cosas que nunca van a estar.

Probado que muerde: se subio el suelo a 94 a mano y R16 lo dijo; deshecho.

## ★★ Y el paso 4 encontro un bug DENTRO de R13

Midiendo la cobertura salieron valores que no cuadraban. El extractor de
constantes cazaba `(0x[0-9A-Fa-f]+|\d+)` y **paraba ahi**:

```text
   pub const DEVICE_HDA: u64 = 1 << 1;                  leia 1, vale 2
   pub const CURRENT_TASK: u64 = 0xFFFF_FFFF_FFFF_FFFE;  leia 0xFFFF
```

**Dos truncamientos en silencio, dentro del juez que existe para cazar numeros
que no coinciden.** Si `CURRENT_TASK` hubiera tenido pareja, R13 habria cantado
un choque falso -- y peor: un truncamiento que POR CASUALIDAD coincida da un
`clean` que nadie puede distinguir de uno de verdad.

> Un extractor que trunca no lee de menos: lee MAL, y lo hace en voz de dato.

Arreglado: se captura la expresion entera y se **evalua o se descarta**. Lo que
no se sabe leer exacto no se empareja, y sale a la vista --hoy 13 constantes--
en vez de colarse con un valor adivinado. Seis autopruebas nuevas, una por cada
forma que le costo.

Con los valores ya correctos aparecieron **tres parejas que antes eran
imposibles**: `CURRENT_TASK`, `DEVICE_SPEAKER` y `DEVICE_HDA`. El espejo pasa
de 90 a 93, y el gate de revision de R13 hizo lo suyo: paro el build hasta que
una persona las miro.

---

# PASO 5 -- REESCRITO: **el zero copy, y el streaming a ritmo de quien lee**

La version anterior decia *"`<bmo/tarea.h>`: lanzar un hijo y esperarlo... es
la I/O en segundo plano"*. **Esa etiqueta era mia y era mala**, y el dueno la
cuestiono con la pregunta correcta:

> *"I/O no se si me beneficia porque mi BMO-X es zero copy, es MAS en tiempo
> real... si hay alternativa MEJOR que I/O, mejor."*

Tiene razon, y lo mejor es que **la alternativa ya existe en el kernel**: lo
que falta es publicarla.

## Lo que el ABI ya tiene, y REX no cuenta

### 1. Streaming a ritmo de QUIEN LEE (no I/O asincrona)

`TASK_OP_ARCHIVO_ASINC` + `ARCH_OP_LISTO`, y lo dice el propio ABI:

> *"Este vuelve en cuanto sabe que el archivo esta ahi. Los bytes llegan a
> trozos, y **preguntar por el archivo es lo que lo trae**: cada `ARCH_OP_LISTO`
> avanza un trozo y vuelve a Ring 3, asi que entre trozo y trozo el planificador
> puede dar el turno a otro."*

** Eso NO es I/O en segundo plano. No hay cola de terminaciones, ni callbacks,
ni hilos, ni buffers en vuelo que no controlas. **El que consume marca el
ritmo**, y el trabajo ocurre dentro de su propia llamada. Es exactamente la
forma que pide un sistema sin hilos de Ring 3 y con la latencia acotada.

`ARCH_OP_LISTO` contesta `(entero << 63) | bytes que ya llegaron` -- *"cuanto
hay"* y *"queda mas"* en la misma respuesta, juntas a proposito.

### 2. El TIEMPO, que es lo que hace falta para ser de tiempo real

`LATIDO_OP_CUENTA`: *"el testigo que se le pasa a `WAIT`. Solo sube y no se
reinicia nunca: un contador que da la vuelta convierte 'espera al siguiente' en
'espera para siempre'"*. Es la pareja natural de la segunda puerta, y REX no la
publica.

### 3. ★★ Y el hueco mas gordo: **el zero copy esta publicado A MEDIAS**

```text
   MEM_OP_OFRECER     SI esta en REX   (superficie/roja.h)   prestar
   TASK_OP_TOMAR      NO               tomar lo prestado
   PRESTADO_OP_*      NO               medirlo, ver si el dueno vive, soltarlo
```

**Una app de C puede PRESTAR memoria y no puede RECIBIRLA.** Justo el eje que
el dueno dice que le importa, y esta cortado por la mitad. `superficie.h` usa
`OFRECER` porque una ventana ofrece sus pixeles al DIRECTOR; el camino de
vuelta --recibir un bloque de otro sin copiarlo-- no tiene cabecera.

## Lo que se propone entonces

```
   [x] 5a  <bmo/prestado.h>   TOMAR + PRESTADO_OP_*   HECHO 2026-09-02
   [ ] 5b  <bmo/latido.h>     LATIDO + WAIT           el tiempo, y la 2a puerta
   [ ] 5c  <bmo/corriente.h>  ARCHIVO_ASINC + LISTO   leer a ritmo de quien lee
```

## ✅ 5a HECHO -- y la cobertura pasa del 47% al 50%

`prestado.h`, 284 lineas, carril ROJO. Cinco parejas nuevas en el espejo
(98 en total) y `examples/prestado_C.c`.

### ★★ Lo que el ejemplo destapo, y no estaba en el plan

**El kernel prohibe prestarse a uno mismo** --`if destino == owner { return
false; }` en `obj/loan.rs`-- asi que un solo `.bex` no puede ser las dos
puntas. El ejemplo hace las dos cosas que SI caben solas (prestar al padre,
tomar lo que haya) y **dice que no puede ensenar el ciclo entero** en vez de
fingir uno.

Y tirando de ahi salio el hueco de verdad:

> Para prestarle a un HIJO hace falta su TID, y el unico TID que un programa
> de C puede conseguir hoy es el de su PADRE. `TAREA_OP_TID` existe en el ABI
> y **no esta en REX**.

** O sea que la punta corta del zero copy no era tomar --eso ya esta-- sino
**saber a quien prestar**. Eso es `<bmo/tarea.h>`, que este plan habia bajado
de prioridad por una razon distinta y equivocada. Sube: no como *control de
procesos*, sino como **el direccionamiento que le falta al prestamo**.

Y `<bmo/tarea.h>` --`CERRAR`, `VIVE`, `DELANTE`, `TID`-- parecia bajar de
prioridad: control de procesos, util para un shell, sin tocar el zero copy.
**Escribir 5a lo desmintio** -- ver abajo. `TID` es el direccionamiento del
prestamo, asi que la mitad de esa cabecera es zero copy con otro nombre.

---

## Lo que NO bloquea esto, y se dice para que no se cuele

- **`tables/` en su raiz tiene siete `.h` sueltos** (`stdio.h`, `stdlib.h`,
  `string.h`, `strings.h`, `ctype.h`, `math.h`, `stdarg.h`) al lado de seis
  carpetas. Eso tambien son fideos, y **son 1.250 lineas mas** que aqui no se
  tocan: el alcance de este plan es `tables/bmo/`. Cuando le toque, el metodo ya
  esta probado.
- **`semantic/`** son intrinsecos del metal, no superficie de app. Otra puerta.
- **Enlace de COBOL y Ada a REX.** No existe y no lo pide nadie todavia.
- **`ascii-sweep` no mira los bytes de CONTROL.** Vigila lo de arriba (>127) y
  no lo de abajo: un `NUL` en un comentario paso su `--check` mientras hacia
  que `grep` tratara el fichero como binario. Es pequeno --unas quince lineas--
  y no bloquea nada de este plan, pero un fichero invisible para `grep` es un
  fichero que las herramientas dejan de auditar sin decirlo.
