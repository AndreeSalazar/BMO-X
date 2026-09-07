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

## R3 -- [ ] EL NUMERO EN EL PANEL

**Bloquea:** nada. Es una fila.

`duenno::neutros()` existe y **no lo lee nadie**. N4 dice que su valor tiene que
estar QUIETO, y una regla sobre un numero que no se mira no se puede romper ni
cumplir.

**Como se sabra que quedo hecha:** aparece en el panel de CABINA junto a
`placa=` y `portero=`, y su valor no cambia entre dos miradas con la maquina en
marcha.

---

## R4 -- [ ] ⚠ QUE N3 SE HAGA CUMPLIR: un marco neutro que se devuelve

**Bloquea:** hay que decidir **quien** rehusa, y no es obvio.

Hoy `free_frame_de` sabe rehusar --devuelve `NoEsTuyo(tiene, quien)`-- pero **el
camino de la purga usa `free_frame` a secas**, sin declarar. Y esa era la
tercera fila de la regla de `duenno`: *"alguno NO declara -> SIN OPINION.
Adelante, y callado"*.

```text
   quien declara hoy    el desmontaje de tablas de paginas (`Duenno::Tabla`)
   quien NO declara     la purga, y las hojas de Ring 3
```

** Asi que N3 esta escrita y **no la vigila nadie todavia**. Se dice aqui en vez
de dejar la regla sonando a cumplida.

⚠ **Y no se arregla poniendo `free_frame_de` en la purga sin pensarlo**: tocar
el camino de devolucion de marcos es ROJO, es el mismo sitio de la azul del
07-09, y el dueno tiene una reproduccion pendiente. **Primero se ejecuta la
1.4b, despues se toca.**

**Como se sabra que quedo hecha:** un marco neutro devuelto produce un `fault`
de CABINA nombrando al aparato, en vez de volver al asignador en silencio.

---

## R5 -- [ ] QUE EL CENSO SE ALIMENTE SOLO

**Bloquea:** nada tecnico. Es trabajo.

`CENSO.txt` se escribe a mano y `dev/portero.rs` ya recorre el PCI y sabe quien
hay. **Dos listas de lo mismo que pueden separarse sin que nadie avise** -- que
es el `[riesgo] ESPEJO` de esta casa, otra vez.

**Como se sabra que quedo hecha:** el guardian del build compara el censo con lo
que el portero encuentra, y **falla** si un aparato de una clase con DMA no
tiene fila.

---

## R6 -- [ ] ★★ LA MMU DE LOS APARATOS -- la unica que cierra el agujero

**Bloquea:** todo lo demas es contabilidad; esto es el mecanismo.

Las cinco de arriba hacen que el neutro se pueda **nombrar, contar y culpar**.
Ninguna impide que un aparato escriba donde no debe. Lo unico que lo impide es
la MMU del lado de los aparatos, que en esta maquina se llama **AMD-Vi**.

```text
   lo que hay hoy   el aparato escribe, y nos enteramos DESPUES
   con la MMU       el aparato escribe donde se le permite, y punto
```

⚠ **Y es un proyecto de verdad**, no una casilla: tablas de traduccion propias,
un dominio por aparato, y el arranque del propio IOMMU desde ACPI. Se escribe
aqui para que tenga sitio, **no para prometerlo**.

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
   1.  R3   una fila en el panel. Barato, y hace visible N4
   2.  R2   ya esta hecho: solo falta EJECUTARLO en el Ryzen
   3.  R5   el guardian del censo. Impide que esto se pudra solo
   4.  R4   ** despues de la 1.4b. Es ROJO y hay una azul sin reproducir
   5.  R6   la MMU. Un proyecto, no una casilla
   6.  R7   cuando haya tarjeta
```

⚠ **R4 va cuarto y no primero a proposito**, aunque sea el que mas suena: toca
el camino de devolucion de marcos, que es donde vive la azul que el dueno
todavia no ha reproducido con los numeros delante. **Arreglar un sitio antes de
haberlo medido es como se pierde la unica reproduccion que se tenia.**
