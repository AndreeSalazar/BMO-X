# VERIFICACION EN METAL -- la sesion del 2026-08-08

> Que teclear, en que orden, **que tiene que salir**, y que mandar de vuelta
> cuando no salga eso. Escrito antes de flashear, para que la sesion delante de
> la maquina no sea "a ver que pasa".

## La regla de esta hoja

Cada prueba dice **que afirma** y **como se cae**. Una prueba que solo puede
salir bien no prueba nada -- si no se sabe de antemano que aspecto tiene el
fallo, cualquier cosa que aparezca en pantalla se lee como exito.

Y el orden no es caprichoso: **lo que no toca nada va primero**, lo que no se
deshace va al final.

---

## 0. Antes de tocar la maquina

Ya comprobado en el anfitrion, para que no se busque aqui:

```
build.ps1 -BuildOnly            verde entero
306 tests de bmo-c-front        verde
los 27 .bex                     BYTE A BYTE IDENTICOS a los de la sesion anterior
```

Esa ultima linea es la importante: **ningun programa que ya corria cambia**. Lo
que cambia es el kernel (contadores de cerrojo, `OP_INFO`) y el compilador de C
--pero el compilador no ha reescrito ninguno de los binarios que se van a
probar--.

---

## 1. `info` -- el renglon nuevo, y es el que tiene que decir CERO

```
info
```

**Que afirma**: que los cerrojos del kernel no se los ha disputado nadie.

Tiene que salir, en la seccion `procesador`:

```
    smp           solo el BSP   (`smp all` levanta los demas)
    cerrojos      0 choques   (lo correcto: nadie pelea)
```

* 🟢 **`0 choques`** es el unico resultado bueno, y no es "poco": es cero por
  construccion. Con un nucleo nadie puede encontrar un cerrojo tomado.
* 🔴 Si sale un numero **en rojo**, no es una cifra de rendimiento: significa
  que alguien entro en el kernel desde otro sitio. Manda la foto y el numero de
  `espera mayor`.

> **Foto**: la seccion `procesador` entera.

---

## 2. F11 -- el arranque contado por el kernel

**Que afirma**: que el relato del arranque llega a Ring 3, y que no hay
`warn`/`fault` nuevos.

Se abre con **F11** y se recorre con RePag/AvPag.

* 🟢 Lineas de `info`, ninguna de `fault`.
* 🔴 Cualquier `[X]` o `fault`: **copia el texto literal**. Los mensajes de
  CABINA llevan el modulo y un numero; ese numero suele ser la respuesta entera.

---

## 3. `run c/leer.bex` -- la cadena de ficheros

```
run c/leer.bex
```

**Que afirma**: `fopen` -> `fread` -> `fseek` -> **relee y compara**. Que las
dos lecturas coincidan es lo que separa *"leyo el fichero"* de *"escribio algo
en mi buffer"*.

* 🟢 Dice que las dos lecturas son iguales.
* 🔴 Si difieren, manda las dos.

---

## 4. ★ EL RAYCASTER -- la prueba que estaba esperando

```
run c/ray.bex
```

### Por que esta prueba es distinta de las demas

Durante meses este programa **dibujo un laberinto que no era el suyo**. El
global `char *mapa = "..."` guarda la DIRECCION del literal, el codegen no sabia
ponerla y **rellenaba de ceros sin decir nada**, asi que `pared()` leia el mapa
desde la direccion 0 -- o sea, desde el `push rbp` de la primera funcion. **Las
paredes eran el codigo maquina del propio programa.**

Y no se noto, porque un raycaster que dibuja paredes desde bytes cualesquiera
**sigue dibujando paredes**. Salia un laberinto plausible. Lo destapo un test de
globales, no una foto.

### Lo que ya esta comprobado en el binario

`ray.bex` (8.728 bytes) lleva:

```
  rodata   448 B   <- el literal del mapa empieza aqui, en rodata+0x0
  data       8 B   <- el puntero `mapa`, y nada mas
  relocs    24 B   <- UNA relocation:
                      SeccionAbs64  en data+0x0  <-  rodata+0x0
```

Traducido: *"escribe en el puntero la direccion del mapa"*. **El lado del
compilador esta probado.** Lo que ninguna maquina ha ejecutado todavia es el
cargador de Ring 0 aplicandola.

### El laberinto que TIENE que salir

`@` es donde arranca (3.5, 3.5) mirando al **este**:

```
        x=0123456789012345
  y= 0  ################
  y= 1  #..............#
  y= 2  #.####....####.#
  y= 3  #.#@.........#.#
  y= 4  #.#.###.###.##.#
  y= 5  #.#...#.#...#..#
  y= 6  #.###.#.#.###.##
  y= 7  #...#.#...#....#
  y= 8  ###.#.###.#.####
  y= 9  #...#...#.#....#
  y=10  #.#####.#.####.#
  y=11  #.....#...#....#
  y=12  #####.#####.####
  y=13  #.....#........#
  y=14  #.....#........#
  y=15  ################
```

★ **La prediccion que hace falsable esta prueba**: mirando al este desde x=3, la
fila 3 esta libre hasta **x=13**. Son **nueve celdas y media de pasillo recto**,
asi que en el primer fotograma la pared del centro tiene que verse **LEJOS Y
BAJA**, con pasillos abriendose a los lados.

* 🟢 Pared del fondo pequena, en el centro, y el laberinto se corresponde con el
  mapa de arriba al andar (W/A/S/D para moverse, Q/E para girar).
* 🔴 **Una pared pegada a la cara al arrancar** = el puntero sigue valiendo 0 y
  se esta dibujando el codigo otra vez. Es el fallo exacto que esta prueba
  busca.
* 🔴 El programa muere al arrancar = la relocation dejo una direccion invalida.
  Entonces el kernel **recupera la pantalla e imprime las ultimas cuatro lineas
  que escribio el proceso**: esa es la foto que hace falta.

> **Foto**: el primer fotograma tal cual sale, antes de moverse. Y si puedes,
> otra despues de andar hacia el este hasta el fondo.

Se sale con **ESC**.

---

## 5. Ctrl+Alt+ESC -- el rescate, probado A PROPOSITO

Volver a entrar (`run c/ray.bex`) y, **sin pulsar ESC**, pulsar
**Ctrl+Alt+ESC**.

**Que afirma**: que un programa que tiene la pantalla Y la entrada **no puede
quedarse la maquina**. La comprobacion vive en `dev::usb`, en el punto unico por
el que pasan todas las teclas, donde ningun proceso la puede desactivar.

* 🟢 El escritorio vuelve solo, y en F11 aparece
  `entrada RESCATADA por el teclado` con el pid.
* 🔴 No vuelve: **es lo mas grave de toda la lista**. Boton de reinicio y
  cuentamelo -- esa es la unica prueba de la que el sistema no se recupera solo.

Esta prueba se hace **queriendo** porque el dia que haga falta de verdad no va a
haber tiempo de averiguar si funciona.

---

## 6. SMP -- del censo inofensivo al reparto

En este orden exacto:

```
smp                 censa y NO TOCA NADA
info                los cerrojos siguen en 0?
smp prueba          reparte con lo que haya despierto
info                y ahora?
smp all             despierta a los once
smp prueba          el reparto de verdad
info                *** el mas importante de los cuatro ***
smp parar           los obreros a hlt
```

**Que afirma cada uno**:

* `smp` a secas contesta el censo y **no manda un solo INIT+SIPI**. Es el caso
  por defecto a proposito: despertar un nucleo es la unica operacion del sistema
  que no se deshace sin reiniciar.
* `smp all` tiene que decir **`12 de 12`**. Si dice menos, la MADT lista mas
  nucleos de los que contestan: manda el numero **y la mascara** que salga en
  CABINA.
* `smp prueba` da la aceleracion. **El numero bonito no es el resultado**: es un
  bucle de cuenta pura, o sea el caso mas favorable que existe, y por eso es un
  techo y no una promesa.
* ★ **`info` despues de `smp prueba` es la prueba de verdad.** `cerrojos` tiene
  que seguir diciendo **0**. Doce nucleos calculando a la vez es el momento en
  que un choque aparece si va a aparecer. Si sube, el reparto se metio en el
  kernel y el nombre del cerrojo sale en CABINA.

⚠ Con los doce en pie, **once nucleos giran al 100 %** -- un obrero que espera
gira, no duerme. Por eso `smp parar` va al final y por eso la orden lo dice.

> **Foto**: `smp all` con el `12 de 12`, y el `info` de despues.

---

## 7. `estratos sellar` -- y el reinicio es parte de la prueba

**Ultimo de la lista porque es el unico que escribe en un disco.**

```
estratos sellar
F12                 mirar la generacion
reboot
F12                 mirar la generacion OTRA VEZ
```

★ **Solo el ultimo paso prueba algo.** Ver la generacion subir antes de
reiniciar dice que el numero cambio en RAM; **verla subida DESPUES del reinicio
es lo unico que dice que llego al plato** y no se quedo en la cache del SSD.

* 🟢 La generacion despues del reinicio es la de antes + 1.
* 🔴 Vuelve a la de antes: el commit no cruzo. Manda las dos capturas de F12 y
  lo que diga CABINA entre medias -- el camino escribe una linea antes y otra
  despues, pase lo que pase.

---

# Que mandarme, y en que forma

Lo que convierte una foto en un arreglo:

1. **La foto de la pantalla**, no una descripcion. "Se veia raro" no se puede
   depurar; un fotograma si.
2. **El texto literal de la linea de CABINA o de F11**, con su numero. Los
   mensajes llevan modulo y valor: `[X] lanzar: NoSeEncuentra 0` y
   `[!] smp: faltan nucleos por contestar 3` dicen cosas distintas y el numero
   es la mitad del mensaje.
3. **En que paso de esta hoja iba**, y si los anteriores habian salido verdes.
4. Si algo se colgo: **que fue lo ultimo que aparecio en pantalla**. Lo que se
   quedo a medio pintar dice en que fase murio.

★ Y lo que mas vale de todo: **un fallo con su foto vale mas que un exito sin
ella**. La lista de arriba esta ordenada para que, si algo se rompe, se rompa
teniendo delante el paso anterior en verde -- que es lo que acota donde mirar.
