# EL ORDEN DE ESTA CARPETA -- donde va lo que llegue, decidido ANTES

> Peticion del dueno, **2026-09-09**: *"no olvides de mover carpetas y
> organizar, pero en NEUTRO alli viviran los archivos segun sus categorias, y
> las GPU que lleguen y otros mas, por esa razon."*
>
> La razon es correcta: **hoy cabe todo plano y en cuanto llegue la tarjeta deja
> de caber.** Este fichero decide el sitio antes de que haga falta, que es la
> unica hora a la que se puede decidir sin prisa.

---

# 1. [!] LO PRIMERO: HOY NO SE MUEVE NADA, Y ESO ES EL RESULTADO

Se miraron los seis ficheros de la raiz de `NEUTRO/` uno a uno:

```text
   LEY.md           las seis reglas N1..N6            LEY de la frontera
   REQUISITOS.md    que falta, R1..R5b                LEY de la frontera
   FRONTERA.txt     que ES y que NO es un neutro      LEY de la frontera
   ARQUITECTURAS.md N6: la misma categoria en 3 CPU   LEY de la frontera
   CENSO.txt        quien es neutro HOY               el CENSO
   README.md        la puerta                         la puerta
```

*** **Los seis son de nivel frontera. Ninguno pertenece a un aparato ni a un
mecanismo, asi que ninguno baja.** Mover uno para que la carpeta pareciera mas
organizada seria dejar la ley dentro de un caso concreto -- que es como una ley
deja de aplicarse a los demas.

> Reorganizar es mover lo que esta en el sitio equivocado. Cuando no hay nada
> en el sitio equivocado, reorganizar es estropear algo.

Lo que si hacia falta era **la regla**, y es el resto de este fichero.

---

# 2. *** LA REGLA: SE ORDENA POR MECANISMO, NO POR APARATO

Y no es una eleccion de gusto: **el censo ya lo dice** y hasta separa sus filas
con un parrafo. Hay exactamente DOS formas de ser neutro:

```text
   1. PIDE Y ESCRIBE     pide memoria a este asignador y luego escribe en ella
                         por el bus, sin pasar por el orquestador
                         -> AHCI, NIC, xHCI, y la GPU cuando llegue

   2. YA ESTA DENTRO     no pide nada. Corre en el mismo silicio o por debajo,
                         antes que nosotros, y la memoria se la toma
                         -> el microcodigo del CPU, el modo SMM del firmware
```

** `CENSO.txt` lo escribe asi de sus dos ultimas filas: *"cumplen las tres
condiciones y NO SE PUEDEN ETIQUETAR: no piden memoria a este asignador"*. Esa
frase es la frontera entre los dos mecanismos, y ya estaba escrita.

## Por que por MECANISMO y no por aparato

Porque una carpeta por aparato duplicaria lo que ya tiene sitio:

```text
   el driver          platform/drivers/storage/ahci/
   que EXIGE          docs/componente/EL_DISCO_EXIGE.md
   como se le vigila  <- ESTO es lo unico que no tiene sitio
```

*** Y "como se le vigila" **es el mismo para los cuatro que piden y escriben**:
el juez de descriptores, el bit en vuelo, la arena, la IOMMU. Una carpeta por
aparato repetiria cuatro veces el mismo material y lo dejaria desincronizado a
la tercera.

> Cuatro aparatos, un mecanismo, una carpeta. Ese es el ahorro.

---

# 3. EL MAPA, CON LA GPU YA COLOCADA

```text
   NEUTRO/
     README.md  LEY.md  CENSO.txt  FRONTERA.txt  REQUISITOS.md
     ARQUITECTURAS.md  ORDEN.md            <- la frontera. No baja nada de aqui
     |
     +-- DMA/          MECANISMO 1: pide y escribe        [EXISTE]
     |     README.md   la puerta y las dos correcciones
     |     EMBUDO.txt  el censo de N0: 14 sitios, y tiene que llegar a 1
     |
     +-- DENTRO/       MECANISMO 2: ya esta dentro        [NO EXISTE TODAVIA]
```

## Donde cae cada cosa que puede llegar

| lo que llegue | donde va | por que |
|---|---|---|
| **la GPU** (DMA de texturas y anillos) | `DMA/` | pide marcos y escribe por el bus, igual que los otros tres |
| **el PSP de AMD** (dentro de esa GPU) | `DENTRO/` | corre ANTES que el CPU y no pide nada. Es otro mecanismo aunque venga en la misma tarjeta |
| una NVMe, una segunda NIC, una capturadora | `DMA/` | mismo mecanismo, y **NO** una carpeta cada una |
| una actualizacion de microcodigo | `DENTRO/` | |
| un aparato que solo MANDA bytes (un teclado) | **ninguna** | `FRONTERA.txt` ya lo excluye: no escribe en memoria |

★★ **La tarjeta grafica se parte en dos filas, y esa es la prueba de que la
regla es por mecanismo.** Si fuera por aparato, la GPU y su PSP compartirian
carpeta -- y no comparten NADA: uno se vigila con una IOMMU y el otro **no se
vigila**.

---

# 4. ** CUANDO SE CREA UNA CARPETA

```text
   cuando tiene contenido. NUNCA antes.
```

Por eso `DENTRO/` **no existe** aunque su sitio este decidido y aunque el censo
ya tenga dos filas suyas. Una carpeta vacia esperando a que alguien la llene es
una promesa, y esta casa no guarda promesas en el arbol -- el mismo motivo por
el que `DMA/` no tiene una subcarpeta por escalon.

** Y por eso este fichero existe: **decidir el sitio y crear la carpeta son dos
cosas distintas.** Aqui esta la decision; la carpeta llega con el primer
documento que tenga algo que decir.

---

# 5. [!] LO QUE ESTA REGLA SACRIFICA (L3)

```text
   1. UN APARATO NO TIENE UN SITIO SUYO
      Quien busque "todo sobre el xHC" no lo encuentra en una carpeta: esta
      repartido entre el driver, `EL_TECLADO_EXIGE.md` y `DMA/`. Se acepta
      porque lo contrario --repetir el mecanismo cuatro veces-- se
      desincroniza a la tercera, y eso ya paso en esta casa (patron 47)

   2. UN APARATO QUE SEA DE LOS DOS MECANISMOS SE PARTE
      La GPU es el primero: su DMA en `DMA/` y su PSP en `DENTRO/`. Es
      incomodo y es CIERTO -- se vigilan de formas que no se parecen en nada

   3. Y SI LLEGA UN TERCER MECANISMO, ESTA REGLA SE QUEDA CORTA
      Hoy son dos porque el censo tiene dos clases. El dia que aparezca algo
      que ni pida ni este dentro --un aparato que hable con otro sin tocar la
      RAM, peer-to-peer PCIe-- **hay que volver aqui**, no inventarle un hueco
      en `DMA/`. Queda dicho para que se note cuando pase
```

---

# 6. LO QUE NO VIVE AQUI, Y SIGUE SIN VIVIR

Esta carpeta contesta *"quien alcanza la RAM sin permiso"*. Las otras preguntas
tienen sus sitios y **no se mudan**, porque `docs/README.md` ordena por PREGUNTA:

```text
   que COPIAR del mundo    ->  docs/maestro/DMA_MAESTRO.md    (el CUANDO)
                           ->  docs/maestro/IOMMU_MAESTRO.md  (el DONDE)
   que casillas faltan     ->  docs/plan/PLAN_EL_NEUTRO_VIGILADO.md
   por que es una categoria->  docs/identidad/EL_NEUTRO.md
   que EXIGE cada aparato  ->  docs/componente/EL_DISCO_EXIGE.md, EL_TECLADO_...
   el codigo               ->  platform/drivers/, y la etiqueta en mm/duenno.rs
```

*** Traerse los maestros aqui seria ordenar por tema en un sitio y por pregunta
en el resto -- y entonces nadie sabe donde mirar. La regla de la casa gana a la
comodidad de tenerlo todo junto.
