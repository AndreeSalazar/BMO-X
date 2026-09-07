# GPU NVIDIA MAESTRO -- traducir en vez de reconstruir?

> Escrito el **2026-09-07**. Sigue el metodo de `SMP_MAESTRO.md`: **que copiar
> del mundo y que seria un error copiar**, antes de una sola linea de codigo.
>
> La pregunta la trajo el dueno con su motivo economico dicho de frente:
>
> > *"en vez de reconstruir todo PERFIL a Nvidia, sino TRADUCIR a mi BMO-X, eso
> > es posible? Para no tener que gastar mucho esfuerzo... con RTX 3060 12G por
> > mi economia no podre comprar la GPU AMD"*
>
> Su hermano mayor es `platform/drivers/gpu/rdna4/PLAN_VULKAN.md`, que da por
> supuesta una RX 9060 XT. **Este documento no lo reemplaza: le pregunta cuanto
> de el sobrevive si la tarjeta es otra.** La respuesta corta es: casi todo,
> menos lo unico que parecia el trabajo.

---

# 0. ★★ LA RESPUESTA, ANTES DE LOS DETALLES

```text
   traducir el DRIVER de AMD a Nvidia          IMPOSIBLE, y no por lo legal
   traducir el PERFIL, el plan y la FORMA      YA ESTA HECHO. Es casi todo
   necesitar la GPU para lo que te frena HOY   NO. Tu cuello es el blit
```

**La palabra "traducir" tiene dos significados aqui y solo uno funciona.**

---

# 1. Lo legal: tienes razon, y NO es lo que te bloquea

El dueno trajo el dato bien: **tres a cero**. Los casos son reales y esa es la
cuenta.

| caso | ano | quien gano |
|---|---|---|
| Sega v. Accolade | 1992 | **Accolade** -- copiar para entender es uso legitimo |
| Sony v. Connectix | 2000 | **Connectix** -- emular la BIOS de PSX, legal |
| Sony v. Bleem | 2000 | **Bleem** -- capturas comparativas, uso legitimo |

★ **Y en Europa es todavia mas claro**, que es lo que aplica aqui. La Directiva
2009/24/CE, articulo 6, permite **explicitamente** descompilar para conseguir
interoperabilidad. No es una defensa que haya que ganar en un juicio como el
*fair use* americano: es un derecho escrito, con sus tres condiciones (que lo
haga alguien con licencia de uso, que la informacion no este ya disponible, y
que se limite a las partes necesarias).

## ⚠ Pero esto NO es lo que te frena, y creerlo cuesta meses

**Para Nvidia casi no hay que hacer ingenieria inversa.** Desde mayo de 2022
Nvidia publica el codigo del lado del kernel:

```text
   open-gpu-kernel-modules     MIT / GPLv2, FUENTE PUBLICADA
   soporta Turing en adelante  -> tu RTX 3060 (GA106, Ampere) ENTRA
   trae cabeceras de registros -> no es una caja negra
```

Asi que el permiso legal que estabas buscando **ya no hace falta pedirlo para lo
principal**: no hay que descompilar lo que viene con licencia MIT.

> El muro no es un abogado. Es un microcontrolador con firma.

---

# 2. Por que el DRIVER no se puede traducir

Esta es la parte que hay que decir sin adornos, porque es la que ahorra el
esfuerzo mal gastado.

**RDNA y Ampere no comparten nada donde importa.** No es como traducir de un
dialecto a otro; es que no hay dos textos que emparejar:

```text
   el juego de instrucciones del sombreador    distinto entero
   el formato de los paquetes de comandos      distinto entero
   los registros de configuracion              distintos, y sin correspondencia
   el motor de display                         distinto entero
   como se arranca un canal de trabajo         distinto entero
```

** No existe una capa intermedia porque **no hay nada que interpretar**: un
paquete PM4 de AMD y un metodo de un canal de Nvidia no son dos formas de decir
lo mismo, son dos maquinas distintas. Escribir un traductor seria escribir los
dos drivers y ademas el traductor.

---

# 3. ★★ Lo que SI se traduce, y es casi todo lo que ya tienes escrito

Aqui esta la buena noticia, y es grande. **Lo que costo escribir de
`PLAN_VULKAN.md` no eran los registros: era el pensamiento.** Y ese es
independiente del fabricante.

| lo que ya esta escrito | sobrevive al cambio de tarjeta? |
|---|---|
| **Meta A vs Meta B** (acelerar el compositor / correr juegos) | ★ **entera**. Es la separacion mas valiosa del documento |
| La ruta B1: **Vulkan por software** | ★ **entera, y ni se entera**: no toca la GPU |
| Que Vulkan abarata y que no (las tres partes) | entera |
| Que un juego pide una LISTA de caracteristicas, no una version | entera |
| Que Vulkan es el 30% de lo que toca un juego | entera |
| ⚠ **El muro del firmware firmado** | entera -- y es el mismo muro, ver abajo |
| Los nombres de registros de RDNA | **no**, y son la parte pequena |

★ Y lo mismo el resto de la casa: la enumeracion de PCI, el juez de la cesion de
MMIO (`bmo-mmio-juicio`), el veto `PisaRam`, la forma de un anillo de comandos
con su timbre, el perfilado por LEY 24. **Nada de eso sabe de que fabricante es
la tarjeta.**

> El PERFIL no se reconstruye porque el PERFIL no era de AMD. Era de BMO-X.

---

# 4. La asimetria real AMD / Nvidia, y donde esta el muro de cada uno

| | AMD RDNA | Nvidia Ampere |
|---|---|---|
| Especificaciones publicas | **si**, documentos completos de registros e ISA | **no** |
| Codigo del kernel publicado | si (amdgpu, GPL) | **si** (`open-gpu-kernel-modules`, MIT/GPL) |
| Cabeceras de registros | en los PDF | **dentro de ese codigo** |
| El muro | **el PSP** y su firmware firmado | **el GSP** y su firmware firmado |
| Cuanto pasa por el muro | parte | ★ **casi todo, y crece con cada generacion** |

## ⚠ El GSP, dicho claro

Desde Turing, Nvidia mete un procesador propio dentro de la GPU --el **GSP**, GPU
System Processor-- y le pasa el trabajo de gestionar el chip. El modulo abierto
de Nvidia es, en buena parte, **un mensajero que habla con ese firmware**.

```text
   el firmware del GSP viene FIRMADO
   no se puede sustituir por uno propio
   se puede redistribuir, pero no auditar ni cambiar
```

★ **Y esto no es peor que AMD, es la misma clase de muro** -- `PLAN_VULKAN.md`
ya lo tiene escrito como *"el muro real: el PSP y el firmware"*. La diferencia
es de grado: en Nvidia pasa mas cosa por el, y en AMD hay mas documentacion
publica para lo que queda fuera.

---

# 5. ★★ Y AHORA LA PREGUNTA QUE DE VERDAD DECIDE: para que quieres la GPU?

Esto es lo que hay que contestar antes que nada de lo anterior, y esta medido en
esta casa, no estimado:

```text
   [MEDIDO]  DOOM a 640x400 ya va a tope
   [MEDIDO]  a 1600x1000 el deficit ENTERO es el blit: ~300 MB/s al framebuffer
   [DATO]    no hay V-Sync ni VBlank. Volcar la pantalla son 27,6 ms
             contra los 16,7 de un fotograma
```

** Ninguna de esas tres cosas necesita sombreadores, ni CUDA, ni Vulkan. Lo que
te falta de una GPU, HOY, son tres piezas mucho mas pequenas:

```text
   1. un MOTOR DE COPIA (DMA) que mueva los pixeles en vez del CPU
   2. la INTERRUPCION DE VBLANK, para que el compositor deje de pintar a ciegas
   3. cambiar de modo de video, y ni eso: el framebuffer del UEFI ya sirve
```

★ Eso es **exactamente la Meta A** de `PLAN_VULKAN.md`, que aquel documento ya
califica de *"alcanzable y bien planificada"* y del tamano *"del driver de
AHCI"*. **La Meta A no exige AMD.** Exige un motor de copia, y eso lo tiene
cualquier GPU de los ultimos veinte anos.

---

# 6. Lo que NO se sabe, y hay que averiguar antes de prometer nada

Se escribe aparte porque es la unica parte de este documento que no es un hecho
comprobado. **Un maestro que mezcla lo que sabe con lo que supone es el que
manda a alguien a perder tres meses.**

```text
   [SIN COMPROBAR]  se puede mover un motor de copia de Ampere SOLO por MMIO,
                    sin el firmware del GSP?  <- ESTA es la pregunta que decide
   [SIN COMPROBAR]  el cambio de modo de video, igual
   [SIN COMPROBAR]  cuanto del modulo abierto es utilizable fuera de Linux --
                    depende de las APIs del kernel de Linux por todas partes
```

★ **La primera es la unica que importa.** Si la respuesta es *"si"*, la Meta A
sobre la 3060 es un proyecto del tamano del AHCI y merece la pena. Si es *"no,
todo pasa por el GSP"*, entonces la Meta A sobre Nvidia cuesta arrancar y hablar
con un firmware firmado, que es otro proyecto entero -- y en ese caso la
respuesta correcta **no es comprar otra tarjeta**: es la ruta B1 y seguir con el
framebuffer del UEFI.

---

# 7. El orden, y el primer paso ya esta dado

```text
   [x] 0. que BMO-X SEPA que la tarjeta esta ahi
          hecho el 07-09: `dev/portero.rs` censa el bus y dice
          "hay una GRAFICA NVIDIA y BMO-X no tiene codigo para ella"
   [ ] 1. leer su identidad: BAR0 y el registro de arranque que da el chip
          una lectura de MMIO. Confirma GA106 sin tocar nada
   [ ] 2. contestar la pregunta de la seccion 6 LEYENDO el modulo abierto
          es fuente publicada: se lee, no se descompila
   [ ] 3. y SOLO entonces decidir entre Meta A sobre Nvidia, o B1
```

## ⚠ Y el paso que NO va aqui

**Comprar la AMD no es el paso 4.** Si la Meta A resulta cara sobre Nvidia, lo
que gana no es otra tarjeta: es que el cuello de hoy --el blit-- tiene otras
respuestas mas baratas que ya estan escritas en
`EL_COMPOSITOR_Y_EL_ESCANER.md`, y ninguna cuesta dinero.

> La tarjeta que tienes no bloquea nada de lo que estas construyendo. Bloquea
> una meta que ya estaba aparcada por escrito, y por otras razones.
