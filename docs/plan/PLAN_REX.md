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

# PASO 2 -- R13, el espejo de los numeros

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
   [ ] 2.1  generar el borrador con el emparejador de familias (74 parejas)
   [ ] 2.2  revisarlo A MANO: una herramienta no sabe si dos nombres son la
            misma cosa. Es la misma nota que ya lleva `--sellar`
   [ ] 2.3  R13 en contrato.py + autoprueba (un par que discrepa, un par nuevo
            sin sellar, y el suelo que baja)
   [ ] 2.4  sellar el suelo en 74
```

---

# PASO 3 -- R14, ninguna app define un numero del kernel

Un `.c` no puede llevar un `#define` de una constante que ya vive en el ABI.

**Hoy fallaria dos veces**, y las dos son `FB_BASE`. Por eso este paso va
DESPUES del 1: una regla que prohibe algo sin dar la salida es una regla que se
apaga en una semana.

```
   [ ] 3.1  R14 sobre toolchain/lang/c/examples/
   [ ] 3.2  decidir el alcance: solo examples/, o tambien mods/ de terceros
```

[!] 3.2 no es obvio y no se decide aqui. Un tercero **tiene derecho** a
redefinir un numero -- es lo que `$BMO_MODS` promete. La regla es para el
arbol propio.

---

# PASO 4 -- R15, la cobertura es un numero y solo sube

Cuantas operaciones del contrato tienen funcion en REX. Hoy **74 de 337**, y de
lo que es de app faltan 31 en siete familias.

Convierte *"REX no tiene X"* de sensacion en cifra con trinquete. Y el
denominador no son las 337: son las 203 que quedan tras quitar las 134 que la
frontera de arriba excluye -- **un porcentaje contra un denominador inflado es
una forma elegante de mentirse**.

```
   [ ] 4.1  la frontera, como tabla legible (no como comentario)
   [ ] 4.2  R15 con trinquete en COBERTURA.txt
```

---

# PASO 5 -- `<bmo/tarea.h>`: lanzar un hijo Y ESPERARLO

`TAREA_OP_CERRAR`, `VIVE`, `DELANTE`, `TID`. Hoy `BMO_OP_EJECUTAR` lanza y
**no hay forma de saber si el hijo sigue vivo** desde C.

Es literalmente la I/O en segundo plano que lleva pendiente desde el 14-08 --el
shell de Ring 0 quedandose con el teclado hasta que el hijo lo reclama-- y es el
unico sitio de REX donde `WAIT` seria la respuesta natural en vez de un bucle
que cede. Las dos puertas, usadas como se disenaron.

---

## Lo que NO bloquea esto, y se dice para que no se cuele

- **`tables/` en su raiz tiene siete `.h` sueltos** (`stdio.h`, `stdlib.h`,
  `string.h`, `strings.h`, `ctype.h`, `math.h`, `stdarg.h`) al lado de seis
  carpetas. Eso tambien son fideos, y **son 1.250 lineas mas** que aqui no se
  tocan: el alcance de este plan es `tables/bmo/`. Cuando le toque, el metodo ya
  esta probado.
- **`semantic/`** son intrinsecos del metal, no superficie de app. Otra puerta.
- **Enlace de COBOL y Ada a REX.** No existe y no lo pide nadie todavia.
