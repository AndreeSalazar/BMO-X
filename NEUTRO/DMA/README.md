# DMA -- el camino por el que el NEUTRO llega a la RAM

> Peticion del dueno, **2026-09-09**: *"aplica pero en carpeta DMA por completo,
> eso en categorias maestro para facilitar [...] con IOMMU y DMA pero ambos
> maestros, el NEUTRO es que si va a llegar lo que necesite con DMA pues es
> logico, pero con DMA con subcarpetas para tener todo lo necesario."*
>
> **Tiene sentido, y con dos correcciones.** Estan en la seccion 1, porque saber
> POR QUE esta carpeta esta aqui y no en la raiz es la mitad de lo que hace.

---

# 1. [!] LAS DOS CORRECCIONES, Y LA CONFIRMACION

## Correccion 1: DMA no es una frontera, asi que no va en la RAIZ

La raiz de BMO-X tiene tres carpetas que **no son codigo, son FRONTERAS**:
`VALKYRIE-ABI` arriba, `PERFIL` abajo y `NEUTRO` al lado. Una frontera dice
**donde acaba la ley**.

Y `FRONTERA.txt` define al neutro con tres condiciones, de las que **la tercera
es la que manda**: *"alcanza la RAM por DMA, sin capability y sin pedir turno"*.

```text
   NEUTRO   QUIEN escribe sin permiso          -> es la frontera
   DMA      COMO llega ese escribir a la RAM   -> es el MECANISMO
```

*** O sea que el DMA **no es hermano del neutro: es su tercera condicion**.
Ponerlo en la raiz al lado suyo diria que son dos cosas comparables, y entonces
`FRONTERA.txt` --que existe justo para que la palabra no se estire-- perderia su
sentido el mismo dia. Por eso esta carpeta vive **DENTRO** de `NEUTRO/`.

## Correccion 2: un `maestro` no es "el documento grande de un tema"

`docs/README.md` declara las categorias, y **no son por tema: son por
PREGUNTA**.

```text
   componente/   que EXIGE esta pieza de quien la use
   maestro/      que COPIAR del mundo, y que seria un error copiar
   plan/         que casillas faltan y como se sabe que quedo hecha
   identidad/    por que BMO-X hace esto distinto en vez de copiar
```

** Un `MAESTRO` es un estudio de lo que hay hecho ahi fuera. `SMP_MAESTRO.md`
no es *"todo sobre SMP"*: es *"que mitad de Cell copiar y que mitad seria un
error"*. Asi que los dos maestros van a `docs/maestro/`, que es donde viven los
maestros -- y esta carpeta los CITA en vez de guardarlos.

## Confirmacion: **si**, son DOS maestros, y por mejor razon de la que parecia

No porque sean del mismo tamano --la IOMMU es UNO de los cinco escalones-- sino
porque **contestan dos preguntas distintas**, y esta casa ordena por pregunta:

```text
   DONDE puede escribir un aparato    -> IOMMU_MAESTRO
   CUANDO puede escribir              -> DMA_MAESTRO
```

*** Y esa separacion no es academica: **un aparato con DMA en vuelo sobre un
bufer ya liberado escribe en una direccion que la IOMMU considera legitima.** El
mapeo es valido, el permiso existe, y el dato aterriza encima de otra cosa. Dos
ejes, dos cuerpos de trabajo previo, dos maestros.

---

# 2. EL MAPA: donde vive cada cosa, y que pregunta contesta

```text
   NEUTRO/                          QUIEN alcanza la RAM sin permiso
     FRONTERA.txt                   que ES y que NO es un neutro
     CENSO.txt                      la lista, con su guardian en el build
     LEY.md  REQUISITOS.md          N1..N3, y R5a hecho / R5b abierto
     ARQUITECTURAS.md               x86-64, ARM64, RISC-V: el mismo agujero

     DMA/                           <- aqui: el MECANISMO
       README.md                    esta puerta
       EMBUDO.txt                   el censo de N0: quien le da una direccion
                                    fisica a un aparato, y cuantas veces
       INTELIGENTE.txt              las TRES formas de darle memoria a un
                                    aparato, y cuando vale cada una

   docs/maestro/DMA_MAESTRO.md      que copiar del mundo sobre disciplina DMA
   docs/maestro/IOMMU_MAESTRO.md    que copiar sobre AMD-Vi / VT-d / SMMU
   docs/plan/PLAN_EL_NEUTRO_VIGILADO.md   las casillas: N0..N7
   docs/identidad/EL_NEUTRO.md      por que esto es una categoria y no un bug
```

** Y no hay una subcarpeta por escalon, a proposito. Una carpeta vacia esperando
a que alguien la llene es una promesa, y esta casa no guarda promesas en el
arbol: `EMBUDO.txt` esta porque **hoy tiene datos**. Lo demas entra cuando
tenga contenido, no antes.

*** Y **esta no es la unica carpeta que va a colgar de `NEUTRO/`**: la regla que
decide cual es cual esta en [`ORDEN.md`](../ORDEN.md), y se ordena **por
MECANISMO, no por aparato**. Esta es el mecanismo 1 --*pide memoria y escribe
por el bus*--, y por eso la GPU que llegue entra AQUI y no en una carpeta suya.
Su PSP, en cambio, es el mecanismo 2 y no vive aqui: **la tarjeta se parte en
dos**, y esa es la prueba de que la regla es la buena.

---

# 3. ★★★ EL HALLAZGO DEL CENSO: LA NIC YA LO HACE BIEN

Contar los sitios dio algo que no se buscaba:

```text
   aparato   sitios   como consigue la direccion fisica
   -------   ------   ----------------------------------------------------
   AHCI         4     `buf_phys` LLEGA DE FUERA. Solo comprueba la paridad
   xHCI         9     anillos propios, pero el bufer de transferencia
                      tambien llega de fuera
   NIC          1     ** DE UNA ARENA: un corral contiguo, entero
                      `Titular::Neutro`, del que salen TODOS sus bufers
```

*** **El escalon N-C del plan --la ventana-- ya existe, y lleva meses
funcionando en la tarjeta de red** (`drivers/net/src/anillo.rs:102`,
`dev/net/mod.rs:325`).

> No hay que inventar el patron. Hay que **llevarlo hacia dentro**, a los otros
> dos.

Y eso cambia el orden del trabajo: N-C deja de ser *"un escalon caro que a lo
mejor no compensa"* y pasa a ser *"lo que uno de los tres ya hace, y hay que
medir si a los otros dos les sale a cuenta"*. Es una pregunta con precedente en
casa, que es la clase de pregunta barata.

---

# 4. POR DONDE SE EMPIEZA

El plan entero esta en
[`PLAN_EL_NEUTRO_VIGILADO.md`](../../docs/plan/PLAN_EL_NEUTRO_VIGILADO.md), y
su primer paso es **N0: que solo haya UN sitio por donde salga una direccion
fisica hacia un aparato.** Hoy son **catorce**, en dos crates, con dos
disciplinas distintas y ninguna comprobacion comun.

```text
   sin embudo   un juez es un juez al que se puede rodear
   con embudo   una linea lo convierte en guardian
```

[!] Y N0 **no es refactorizar por gusto**: es la unica forma de que el numero
`14` se pueda vigilar. Un guardian que cuenta *"cuantos sitios escriben una
direccion fisica a un aparato"* y exige que sea **1** es lo que impide que el
decimoquinto entre sin que nadie lo vea -- exactamente como `censo-neutro`
impide que un aparato nuevo etiquete sin fila.
