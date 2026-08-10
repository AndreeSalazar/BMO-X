# VERIFICACION EN METAL -- la sesion del 2026-08-09

> Que teclear, en que orden, **que tiene que salir**, y que mandar de vuelta
> cuando no salga eso. Escrito antes de flashear, para que la sesion delante de
> la maquina no sea "a ver que pasa".
>
> La hoja del 08-08 esta en `VERIFICACION_METAL.md` y sigue valiendo para lo que
> quedara pendiente de aquella.

## La regla de esta hoja

Cada prueba dice **que afirma** y **como se cae**. Una prueba que solo puede
salir bien no prueba nada: si no se sabe de antemano que aspecto tiene el fallo,
cualquier cosa que aparezca en pantalla se lee como exito.

Y el orden no es caprichoso: **primero lo que no puede romper el arranque**, y
al final lo que si.

---

## 0. Antes de tocar la maquina

Ya comprobado en el anfitrion, para que no se busque aqui:

```
build.ps1 -BuildOnly            verde entero
operaciones: 37 opcodes         ninguno repetido (el guardia del build)
353 filas de bmo-c-front        verde, CERO ignoradas
693 del workspace               verde
```

** Y lo que **NO** esta comprobado y hay que tener presente: esta tanda toca el
**camino de carga de cualquier `.bex`** (`MAX_BEX` de 1 a 4 MiB) y **la emision
de `malloc`** en todo programa que lea ficheros. No son cambios de una
caracteristica nueva y aislada: si algo deja de arrancar, empezar por ahi.

---

## 1. Que arranque -- y esta es la prueba de `MAX_BEX`

Encender y esperar al escritorio. **Nada nuevo que teclear.**

| Sale | Significa |
|---|---|
| el escritorio, como siempre | ✅ el bufer de 4 MiB se reservo y el `.bss` cabe en el hueco de 16 MiB |
| se queda en el panel del kernel | el compositor no cargo -- F11 y buscar `no cabe en el buffer` |
| no arranca nada | **es `MAX_BEX`**: vuelta atras con `git checkout HEAD~N -- Ultra_kernel_x86-64/` |

Lo unico que cambio es el tamano de un array en `.bss`. El `.bin` del kernel
**no crecio** (909.696 B antes, 914.128 B despues -- esos 4.432 son el codigo
nuevo de `MI_PAQUETE`, no el bufer). Si eso rompiera el arranque seria porque el
hueco de 16 MiB que reserva el cargador UEFI no es lo que creemos, y esa es
informacion que vale mas que la prueba.

---

## 2. `run c/leer.bex` -- la mina del orden de los bloques

**Afirma**: que `fread` funciona **por construccion** y no por casualidad.

Antes se publicaba el bloque del ULTIMO `malloc`; ahora el PRIMERO. `leer_C.c`
abria y luego pedia memoria, o sea que acertaba por el orden. Ahora tiene que
acertar igual haciendo lo mismo.

Tiene que salir lo de siempre: las dos lecturas de `datos/salida.txt`
coincidiendo. **Si sale igual que antes, la prueba ha pasado** -- lo que se
comprueba es que NO cambio.

Si sale `fread trajo 0 bytes`, el arreglo rompio el caso que funcionaba: es el
calculo de `desde = dst - base` en `<bmo/archivo.h>`.

---

## 3. ** `run c/caja.bex` -- LA APP QUE ES UN SOLO FICHERO

**Afirma la cadena entera del paquete**, y es lo nuevo de esta sesion.

Este `.bex` lleva sus datos DENTRO (`bmo-pack` se los mete en el build) y los
lee **sin escribir ninguna ruta**: le pide al kernel su propia imagen.

```
caja: 2 recursos
[hola desde dentro de la caja] 28
numeros: 1 2 3 4 5 6 7 8
CAJA: las cuatro pruebas pasan
```

Cada linea afirma una cosa distinta, y por eso son cuatro:

| Linea | Que prueba |
|---|---|
| `caja: 2 recursos` | el kernel recordo de donde salio el proceso, abrio esa imagen, y el indice se leyo |
| `[hola...] 28` | un recurso de texto sale entero y con su longitud |
| `numeros: 1..8` | y uno binario sale **con los bytes correctos** -- un recurso del tamano bueno y el contenido de otro sitio se ve igual desde fuera |
| las cuatro pruebas | lo que no esta contesta cero, no basura |

**Como se cae, y cada forma apunta a un sitio:**

| Sale | Donde mirar |
|---|---|
| `caja: no es un paquete` | F11: `[paquete] la ruta no cabe` -> el nombre pasa de 64 bytes. Sin linea -> `MI_PAQUETE` devolvio 0: o el kernel no recordo, o `archivo::open` fallo |
| `caja: 0 recursos` | el `.bex` se desplego **sin empaquetar**: `bmo-pack --listar` sobre el fichero del disco lo dice |
| `numeros:` vacio o con otros bytes | `ARCH_OP_LEER_EN` trae bytes del sitio equivocado -- el offset del indice es RELATIVO a la seccion y hay que sumarle la base |
| `CAJA: pasan 3 de 4` | dice cual falta por eliminacion; las tres primeras se ven en pantalla |

★ **Y una comprobacion que vale la pena hacer desde Windows**: enchufar el
Kingston y mirar `c\caja.bex`. Windows ve **un fichero** de 8.744 bytes, no un
binario y una carpeta de datos. Eso es el objetivo entero del formato.

---

## 4. F10 -- el volumen del audifono, con la oreja

**Afirma**: que la ventana de sonido dice la verdad sobre lo que hay enchufado.

Con el audifono USB puesto, abrir **F10**. Lo primero es la linea de arriba:

```
aparato   altavoz del PC + audifono USB
el volumen manda sobre el audifono USB de verdad
```

** Si dice solo `altavoz del PC`, el bit `DEVICE_USB` no se encendio y el
audifono no se localizo. **F11 dice por que, y ahora cada causa tiene su
linea:**

| En CABINA | Que pasa |
|---|---|
| `audifono USB con volumen` | lo encontro; el numero es el slot |
| `el descriptor no cabe entero` | mide mas de 512 B y el Feature Unit puede quedar fuera |
| `aparato de audio SIN control de volumen` | esta, pero no deja |
| `ningun aparato de audio en los slots 1..8` | no lo vio. **Linea nueva**: antes esto se daba callando |
| `el aparato no dijo su rango: se supone` | no contesto `GET_MIN`/`GET_MAX` |
| `el aparato rechazo el volumen` | STALL, y ya se probaron todos los canales |
| `el maestro no valia: el volumen va por canal` | ✅ funciono, por la segunda vuelta |
| `el aparato guardo OTRO volumen` | acepto la peticion y guardo otra cosa |

Luego, **con algo sonando**, las flechas:

- subir y bajar tiene que **oirse**, y el 50% tiene que sonar a media fuerza
  (la curva es logaritmica a proposito: un mapeo lineal en dB salta de mudo a
  ensordecedor);
- ★ **bajar a 0 tiene que CALLAR DEL TODO.** Es el arreglo que mas importa:
  antes se mandaba `0x8000`, que esta fuera del rango que declara el aparato, y
  un aparato que valida contesta STALL -- barra a 0 y **los cascos sonando
  igual**. Ahora el 0% va por el control de MUTE;
- y **volver a subir tiene que devolver el sonido**. Sin quitar el mute, el
  volumen llega, se acepta, y no se oye: el sintoma que hace pensar que el
  camino entero esta roto.

---

## 5. Que todo lo demas siga igual

La red de seguridad. Cinco programas de los que ya corrian:

```
run cobol/1/hola.bex        run c/holac.bex       run apps/extracto.bex
run c/memc.bex              run c/sonido.bex
```

**Ninguno tiene que cambiar.** Lo que se comprueba es que el `MAX_BEX` nuevo y
la emision nueva de `malloc` no tocaron a nadie.

---

# Que mandarme, y en que forma

`datos/salida.txt` del Kingston, que es lo que sustituye a las fotos. Empieza
por el eco del comando, asi que basta con lanzar las pruebas en orden y pasarme
el fichero entero.

De F10 y F11 hace falta foto, porque no pasan por la consola.

Y de lo del audifono, **lo que oiste**: eso no lo dice ningun fichero.
