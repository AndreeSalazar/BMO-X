# QUE FALTA PARA COMPLETAR EL NEUTRO

> El dueno lo pidio asi: *"investiga MAS, que faltarian requisitos poner en
> NEUTRO carpeta que pide para poder completar y facilitar"*.
>
> Formato de `plan/`: cada casilla con **que la bloquea** y **como se sabe que
> quedo hecha**. Lo marcado con `[x]` esta hecho **y se dice desde donde se
> comprueba** -- una casilla sin forma de comprobarla es una casilla marcada por
> optimismo.

---

## R1 -- [x] LA ETIQUETA: que un marco de aparato se pueda NOMBRAR

**Bloqueaba:** nada. `mm/duenno.rs` ya tenia ocho clases y la novena era una
linea.

**Hecho el 2026-09-07.** `Duenno::Neutro` existe, y los cuatro sitios que piden
DMA la usan: dos del disco, uno de la red, uno del USB.

```text
   antes   los marcos de DMA salian `Anonimo` = SIN OPINION
   ahora   salen `NEUTRO (un aparato)`
```

**Como se comprueba:** `duenno::neutros()` es mayor que cero tras arrancar.

---

## R2 -- [x] LA AZUL LO DICE, Y SALIO GRATIS

**Bloqueaba:** nada, y esto es lo bonito: **no hizo falta tocar la pantalla
azul**. `plat/faults/amarilla.rs` ya preguntaba `duenno_de(fisica)` y pintaba
`q.nombre()`; lo unico que le faltaba era que existiera un nombre que decir.

```text
   antes   "y NINGUNA tabla reclama ese marco: contabilidad rota"
   ahora   "ese marco se pidio como: NEUTRO (un aparato)"
```

★ **Y eso es media respuesta a la pista 1.5** de `docs/metal/PRUEBA_EN_METAL_0907.md`:
la proxima azul dira si el marco en disputa era de un aparato o no, que es
exactamente la pregunta que la pista hacia.

**Como se comprueba:** reproducir la purga y leer la azul. Sigue sin ejecutarse.

---

## R3 -- [x] EL NUMERO EN EL PANEL, y salio con un vigilante dentro

**Hecho el 2026-09-07.** `neutro=vivos:soltados` en la fila `sys`, al lado de
`mem=`, porque es un hecho de MEMORIA y no de USB.

Lo que se pidio era una fila. Lo que aparecio al escribirla es otra cosa:

```text
   la version vieja de `neutros()` RECORRIA la tabla -- cuatro millones de
   entradas-- y eso no se puede poner en un panel que se repinta
```

Asi que la cuenta se llevo a `marcar`, el unico sitio por el que un marco cambia
de dueno. Y ahi aparecio lo que no se buscaba: **si la cuenta puede subir, puede
bajar** -- y un marco de aparato que deja de serlo es **N3 rota**.

★★ `soltados` la vigila **desde el lado del marcado, sin tocar el camino de
devolucion de marcos**. Que es justo lo que R4 dice que no se puede tocar
todavia.

> Se puede saber que una regla se rompio sin ponerse delante de ella.

**Como se comprueba:** arrancar y mirar la fila `sys`. `vivos` distinto de cero
y quieto; `soltados` en cero. Si `soltados` sube, la fila entera se pone en rojo
-- gana sobre el aviso de RAM baja, porque quedarse sin memoria es incomodo y un
marco de aparato suelto es corrupcion esperando turno.

---

## R4 -- [ ] ⚠ QUE N3 SE HAGA CUMPLIR: un marco neutro que se devuelve

> ⭐ **Y desde R3 ya se DETECTA**, aunque no se impida. `soltados` cuenta las
> veces que pasa. Lo que falta aqui es **rehusarlo**, que es otra cosa y toca el
> camino ROJO.

**Bloquea:** hay que decidir **quien** rehusa, y no es obvio.

Hoy `free_frame_de` sabe rehusar --devuelve `NoEsTuyo(tiene, quien)`-- pero **el
camino de la purga usa `free_frame` a secas**, sin declarar. Y esa era la
tercera fila de la regla de `duenno`: *"alguno NO declara -> SIN OPINION.
Adelante, y callado"*.

```text
   quien declara hoy    el desmontaje de tablas de paginas (`Duenno::Tabla`)
   quien NO declara     la purga, y las hojas de Ring 3
```

** Asi que N3 esta escrita y **nadie la REHUSA todavia**. Desde R3 si se VIGILA
--`soltados` cuenta las veces-- y son dos cosas distintas que conviene no
mezclar:

```text
   vigilar   saber que paso. HECHO, y sin entrar en el camino rojo
   rehusar   impedir que pase. ES ESTO, y hay que entrar
```

⚠ **Y no se arregla poniendo `free_frame_de` en la purga sin pensarlo**: tocar
el camino de devolucion de marcos es ROJO, es el mismo sitio de la azul del
07-09, y el dueno tiene una reproduccion pendiente. **Primero se ejecuta la
1.4b, despues se toca.**

**Como se sabra que quedo hecha:** un marco neutro devuelto produce un `fault`
de CABINA nombrando al aparato, en vez de volver al asignador en silencio.

---

## R5 -- EL GUARDIAN DEL CENSO. Y son DOS mitades, no una

Al ir a escribirlo aparecio que el requisito estaba mal planteado: pedia que el
guardian del build comparase el censo *"con lo que el portero encuentra"*, y
**el build no puede ver el bus PCI**. Corre en el anfitrion.

Asi que R5 se parte, y las dos mitades son de verdad distintas:

```text
   R5a  codigo <-> censo     el build. Corre veinte veces al dia
   R5b  censo  <-> maquina   el portero. Hay que ARRANCAR para verla
```

### R5a -- [x] el guardian del build

**Hecho el 2026-09-07.** `toolchain/tools/censo-neutro/censo_neutro.py`,
enganchado en `build.ps1` como los otros cinco. Comprueba cuatro cosas:

```text
   1. todo fichero que etiqueta `Duenno::Neutro` tiene fila en el censo   (N1)
   2. toda fila que nombra un fichero, ese fichero existe y etiqueta      (N2)
   3. la cuenta `xN` de la fila es el numero de sitios que etiquetan
   4. y DICE cuantas filas se salto, para que la cobertura parcial no se
      disfrace de salud
```

★ **Y sabe decir que NO** (L4), probado en las tres formas antes de engancharlo:

```text
   la cuenta miente (x2 -> x3)     "el censo dice x3 y el codigo etiqueta 2"
   un fichero etiqueta sin fila    "... y NO tiene fila en el censo (N1)"
   el censo desaparece            "el censo NO EXISTE"
```

** La tercera importa mas de lo que parece: la leccion del propio `Guardian` de
`build.ps1` es que **un path mal escrito dejo un guardian muerto y el build dijo
COMPLETE igual**. Un censo que falta no puede leerse como un censo limpio.

Y el formato de la tabla queda escrito **dentro de `CENSO.txt`**, no solo en el
guardian: un formato que solo conoce quien lo comprueba es un formato que nadie
puede cumplir.

**Como se comprueba:** sale `clean: el censo del neutro cuadra con el codigo` en
cada build.

### R5b -- [ ] el censo contra la maquina de verdad

**Bloquea:** hace falta arrancar, y el censo no se puede leer desde Ring 0 (no
hay sistema de ficheros a esa altura del arranque).

Lo que ya existe es la mitad util: `dev/portero.rs` cuenta `sincodigo` -- los
aparatos de una clase que BMO-X podria querer y que no toca nadie. **Lo que
falta es cerrar el circulo**: que el portero sepa cuales de esos tienen DMA y
avise si no estan en el censo.

**Como se sabra que quedo hecha:** enchufar una tarjeta con DMA que no este en
el censo, y que el panel lo diga sin haber tocado el codigo.

---

## R6 -- ★★ LA MMU DE LOS APARATOS -- la unica que cierra el agujero

### ⚠ PRIMERO, EL MALENTENDIDO: esto NO espera a la GPU

El dueno lo pregunto asi: *"R6 la MMU, aunque eso es cuando llegue la GPU, no?"*.

**No.** La MMU de los aparatos protege de los aparatos que **ya estan dentro de
la maquina**: el AHCI, la tarjeta de red y el xHC. Los tres estan en el censo
desde la primera fila y los tres escriben en la RAM hoy.

```text
   lo que cierra R6 el dia que se haga    la pista 1.5, en esta maquina, HOY
   lo que NO tiene nada que ver           que llegue o no una tarjeta grafica
```

★ Aplazarlo *"hasta que llegue la GPU"* seria aplazar el arreglo del agujero por
el que ya se cayo la maquina el 07-09.

### R6.0 -- [x] ★★ QUE LA PLACA CONFIESE SI HAY IOMMU -- **ya estaba hecho**

Y esto se encontro al ir a planificar R6: **la maquina ya sabe contestarlo**.
`plat/placa.rs` lee la tabla **IVRS** del firmware desde antes de que NEUTRO
existiera, y dice tres cosas:

```text
   cuantos IOMMU declara el firmware
   donde viven sus registros              (la base de MMIO del primero)
   el IVinfo crudo, sin interpretar
```

Y si NO hay IVRS, avisa con la frase exacta -- escrita antes de esta carpeta y
diciendo lo mismo que ella:

> *"[!] sin IVRS: un aparato con DMA no tiene quien lo limite"*

⚠ **Pero esa linea NUNCA SE HA MIRADO.** Va a `CABINA` en el arranque y nadie ha
ido a leerla. Asi que la pregunta que decide si R6 es siquiera posible en esta
placa **ya tiene respuesta y esta sin recoger**.

**Como se comprueba:** arrancar y buscar en CABINA la linea `placa ... IOMMU`.
Entra en la hoja del metal.

### R6.1 -- [ ] Y LA RESPUESTA PUEDE SER QUE NO

Esta es la parte incomoda y va escrita antes de planificar nada:

```text
   si hay IVRS       R6 es posible, y se puede escribir su plan con un numero
   si NO hay IVRS    ** R6 esta MUERTO en esta placa
```

Una A320M barata con un firmware viejo puede no declarar IOMMU, o traerlo
apagado. **Y en ese caso NEUTRO se queda como esta para siempre en esta
maquina** -- se nombra, se cuenta y se culpa, y no se impide.

*** Saberlo cuesta un arranque. Planificar R6 sin saberlo cuesta semanas.

### R6.2 -- [ ] EL PROYECTO, si la placa dice que si

**Bloquea:** R6.0, y nada mas.

```text
   [ ] arrancar el propio IOMMU desde sus registros
   [ ] tablas de traduccion propias, y un dominio por aparato
   [ ] meter en cada dominio EXACTAMENTE los marcos de su fila del censo
   [ ] y que un fallo del IOMMU llegue a CABINA con el nombre del aparato
```

⚠ **Es un proyecto de verdad, no una casilla.** Se escribe aqui para que tenga
sitio, **no para prometerlo**.

★ Y fijate en la tercera linea: **el censo ya dice que marcos son de quien.**
R1, R3 y R5a no eran contabilidad por gusto -- son la lista con la que se
rellenan los dominios el dia que esto se haga.

**Como se sabra que quedo hecha:** un aparato programado con una direccion que
no es suya produce un fallo del IOMMU en vez de corromper memoria. Es decir:
**la pista 1.5 deja de poder ocurrir.**

---

## R7 -- [ ] LA GPU, CUANDO LLEGUE

**Bloquea:** no hay tarjeta.

El PSP es el neutro mas grande que va a entrar en esta maquina: ejecuta firmware
firmado que no se puede leer, y una vez arrancado **todo el demas firmware de la
GPU sube por su anillo**. Ver `platform/drivers/gpu/rdna4/PSP_MEDIDO.md`.

```text
   [ ] su fila en CENSO.txt, ANTES de que funcione   (N1)
   [ ] sus marcos con `Duenno::Neutro`               (N2)
   [ ] y comprobar que `neutros()` sube UNA vez      (N4)
```

**Como se sabra que quedo hecha:** el portero del bus deja de decir *"hay una
GRAFICA y BMO-X no tiene codigo para ella"*, y el censo tiene una fila mas.

---

## ★ EL ORDEN, y no es el de los numeros

```text
   1.  [x] R3   hecho. Y de regalo, `soltados` DETECTA la rotura de N3
   2.  R2   ya esta hecho: solo falta EJECUTARLO en el Ryzen
   3.  [x] R5a  hecho. R5b sigue: hay que arrancar para ver esa mitad
   4.  R4   ** despues de la 1.4b. Es ROJO y hay una azul sin reproducir
   5.  [x] R6.0  ya estaba hecho. Falta MIRARLO: la linea `placa ... IOMMU`
       R6.1  y la respuesta puede ser que NO, y eso cierra el tema
       R6.2  el proyecto, solo si la placa dice que si
   6.  R7   cuando haya tarjeta
```

⚠ **R4 va cuarto y no primero a proposito**, aunque sea el que mas suena: toca
el camino de devolucion de marcos, que es donde vive la azul que el dueno
todavia no ha reproducido con los numeros delante. **Arreglar un sitio antes de
haberlo medido es como se pierde la unica reproduccion que se tenia.**
