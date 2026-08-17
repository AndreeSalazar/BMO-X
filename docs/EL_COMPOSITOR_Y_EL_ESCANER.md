# EL COMPOSITOR CONTRA EL ESCANER

> Capitulo de componente, como `EL_TECLADO_EXIGE.md` y `LA_PUERTA_POR_DENTRO.md`.
>
> Nace de una pregunta del dueno el **2026-08-17**:
>
> > *"la CPU arranca normal porque BMO-X procesa normal, pero la GPU se
> > desincroniza... si metes MUCHOS elementos en un frame mientras la CPU va en
> > su logica, puede pasar eso, no? No siempre V-Sync, sino por motivos."*
>
> La intuicion es correcta. El nombre no: **hoy no hay GPU**. Pero el fenomeno
> que describe **ya existe**, tiene numero, y es peor de lo que suena.

---

## 0. Los dos relojes que nadie sincroniza

En esta maquina hay dos cosas mirando la misma memoria, y **ninguna sabe de la
otra**:

```
   EL COMPOSITOR   pinta en su lienzo y vuelca la caja sucia cuando termina.
                   Su ritmo lo pone el trabajo: un frame gordo tarda mas.

   EL ESCANER      lee el framebuffer 60 veces por segundo, de arriba abajo,
                   pase lo que pase. Su ritmo lo pone el monitor.
```

★ **BMO-X no tiene V-Sync, ni espera de VBlank, ni una sola linea que pregunte
por donde va el haz.** No es un descuido pendiente de arreglar: es que nunca ha
existido. El volcado (`Pantalla::vaciar`) ocurre cuando el compositor acaba, y
donde caiga.

---

## 1. La aritmetica, que es la respuesta entera

```
   [CALCULADO]  un frame completo   1920 x 1080 x 4 B  =  8,3 MB
   [MEDIDO]     el blit al framebuffer                 ~300 MB/s
   ---------------------------------------------------------------
                volcar la pantalla entera              =  27,6 ms
   [SPEC]       un frame de video a 60 Hz              =  16,7 ms
```

> ★★ **Un volcado completo dura MAS que un frame de video.** No es que "a veces
> se desincronice": es que si se volcara la pantalla entera, **el escaner
> alcanzaria al volcado siempre**, por construccion. Y ademas techaria el
> escritorio en ~36 fps sin pintar nada.

[!] Los ~300 MB/s salen de la tanda de DOOM a 1600x1000, no de una medida hecha
para esto. Es el orden de magnitud correcto --el framebuffer esta al otro lado
del PCIe y en write-combining-- pero **si algun dia se va a tocar este camino, lo
primero es medirlo aqui**, no reusar el numero de otro sitio.

### Lo que hoy lo sostiene: no se vuelca la pantalla

`userland/src/sin_gpu/sucio.rs` --la carpeta se llama asi, y ya lo decia todo--
lleva hasta **8 cajas** de lo realmente tocado y vuelca solo eso:

```
   [CALCULADO]  el cursor      32 x 32 x 4  =    4 KB  ->   13 us
                la terminal   760 x 428 x 4 =  1,3 MB  ->  4,3 ms
                media pantalla                4,2 MB  ->   14 ms
   -------------------------------------------------------------------
                lo que CABE en un frame de video       =  5,0 MB = 60%
```

**Ese 60% es el umbral de tu pregunta.** Mientras lo tocado en un frame se quede
por debajo, el volcado cabe entre dos barridos del escaner y no pasa nada. Por
encima, el volcado se cruza con el haz **con seguridad**, y lo que se ve es la
mitad de arriba del frame nuevo y la mitad de abajo del viejo.

### Y por que fusionar cajas es una CUENTA y no un gusto

`desperdicio(a, b)` mide los pixeles que se copiarian de mas al juntar dos
cajas, y solo las junta si sale por debajo de `COSTE_DE_UNA_CAJA = 4096`. Con las
ocho llenas degenera en una caja grande, o sea en el comportamiento de antes:
**no puede empeorar**, solo dejar de mejorar.

---

## 2. Tu escenario, paso a paso

> *"si metes MUCHOS elementos en un frame para la GPU mientras la CPU va en su
> logica"*

Traducido a lo que hay:

```
   1. muchos elementos          -> muchas cajas sucias, repartidas
   2. mas de ocho, o dispersas  -> se fusionan en una envolvente grande
   3. la envolvente pasa del 60% de la pantalla
   4. el volcado ya no cabe en 16,7 ms
   5. el escaner lo pilla a medias   <- lo que tu llamas "desincronizado"
```

★ **El paso 2 es el que decide, y es contraintuitivo**: no es la cantidad de
pixeles pintados, es **lo separados que estan**. Tocar la esquina de arriba y la
de abajo con dos puntos de un pixel da una envolvente de pantalla completa. Por
eso el arreglo del 12-08 fue trocear en varias cajas y no "pintar menos".

[!] Y hay un segundo camino al mismo sitio que no es el volcado: **si la logica
de la CPU tarda mas de 16,7 ms, el frame entero llega tarde** y el escaner
repite el anterior. Eso no es tearing, es un tiron -- y se distingue a ojo: el
tearing corta la imagen en horizontal, el tiron la congela.

---

## 3. Que haria falta para arreglarlo de verdad

```
   VBlank    preguntarle al hardware por donde va el haz y volcar en el hueco.
             Hoy NO existe: no hay registro de video leido en ninguna parte,
             y el framebuffer viene del firmware (GOP), sin driver de video.
   triple    un tercer bufer para no esperar. Cuesta otros 8 MB de RAM.
   bufer     Con 15 GiB no es el problema; el problema es el ancho de banda,
             que no cambia.
```

★★ **Y el diseno que absorbe esto ya esta elegido**, sin que se eligiera para
esto: `docs/PLAN_DIRECTOR.md` -- *una app dibuja en SU memoria y se compone en un
marco*. Ese es exactamente el reparto productor/consumidor que hace que la
desincronizacion no rompa: cada app va a su ritmo en su superficie, y **el
compositor decide cuando componer**. El dia que llegue una GPU, eso es una cola
de swapchain con otro nombre.

---

## 4. Cuando llegue la GPU: que cambia y que no

```
   CAMBIA     el ancho de banda. Hoy la CPU escribe 8 MB por PCIe a ~300 MB/s;
              una GPU escribe en su VRAM a cientos de GB/s. El cuello de la
              seccion 1 DESAPARECE.
   NO CAMBIA  la sincronizacion. Sigue habiendo un escaner a 60 Hz que no
              pregunta. Con GPU el problema se llama swapchain y se resuelve
              igual: VBlank, o triple bufer, o los dos.
```

[!] O sea que **la GPU no arregla lo que tu describes: lo hace mas facil de
ocultar.** Con 8 MB en 0,02 ms en vez de 27 ms, casi cualquier volcado cabe en el
hueco -- pero si no se pregunta por el hueco, sigue habiendo un dia en que no.

Vulkan esta **APARCADO con plan escrito**, no descartado. Cuando se retome, esta
seccion es su primera pagina.

---

## 5. ★ La ventana en vivo que pide el dueno

> *"eso seria en la F5 o no se? Es para leer TODO lo que pasa en consumo en
> tiempo real... puedes crear una interfaz y vivir en la pantalla, aunque eso
> requiere atajos propios."*

**Se puede, y la mitad ya esta.** F7 (CPU) y F8 (memoria) son exactamente esa
clase de ventana: se repintan solas cada 15 fotogramas y viven en la pantalla.
Falta la tercera, la del PINTADO, y **F5, F6 y F9 estan libres** (F7 F8 F10 F11
F12 ya tienen dueno).

Lo que tendria que ensenar, y de donde sale cada dato:

```
   cajas por frame        `sucio.rs::cajas()`        ya existe
   pixeles volcados       `sucio.rs::pixeles()`      ya existe
   MB/s al framebuffer    pixeles x 4 x fps          cuenta
   % de la pantalla       contra ancho x alto        cuenta
   el PEOR frame          maximo desde que se abrio  falta guardarlo
   fotogramas por segundo el bucle ya los cuenta     ya existe
```

### ⚠ Y la trampa, que es la misma de esta semana

**Una ventana que se repinta para medir el repintado se mide a si misma.** Cada
refresco del panel ensucia su propia caja, la suma a los MB volcados, y el numero
que ensena incluye el coste de ensenarlo. Es el hermano exacto de la ventana
sucia que acaba de morder al metro de la puerta.

Las dos salidas honestas, y hay que elegir una **antes** de escribir el panel:

```
   a) excluir la caja del propio panel de la cuenta, y decirlo en la ventana
   b) contarla y DECLARARLO -- "incluye este panel: N KB por refresco"
```

La (b) es mas barata y mas dificil de estropear. La (a) miente menos, pero
alguien tiene que acordarse de mantener la exclusion el dia que el panel cambie
de sitio.

---

## 6. Lo que este documento NO afirma

- **Que los ~300 MB/s sean el numero de esta maquina para este uso.** Vienen de
  DOOM a otra resolucion. El orden de magnitud manda; la cifra exacta, no.
- **Que haya tearing hoy.** No se ha visto ni se ha buscado: el troceado en cajas
  lo hace improbable en el uso normal del escritorio. Lo que dice este documento
  es **cuando dejaria de serlo**, y por que.
- **Que el compositor vaya lento.** A 60 fps con cajas pequenas sobra tiempo. El
  problema aparece con el area, no con la frecuencia.

---

*Ver `docs/PLAN_DIRECTOR.md` para las superficies, `LIENZO.md` para el modelo de
dibujo, y `bmo-doom-rendimiento` para de donde sale el numero del blit.*
