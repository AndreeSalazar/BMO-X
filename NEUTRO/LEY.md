# LA LEY DEL NEUTRO -- N1 a N6

> Seis reglas. Cada una nombra **su componente y su numero** (L1) y **trae su
> sacrificio** (L3): una regla que no cuesta nada no esta decidida, esta
> deseada.
>
> Y ninguna nombra un CPU concreto. Esa es N6, y es la que hace agnostica a la
> carpeta entera.

---

## N1 -- UN NEUTRO SE DECLARA ANTES DE EXISTIR

**Componente:** [`CENSO.txt`](CENSO.txt).

No hay aparato neutro sin su linea en el censo. Un aparato que alcanza la RAM y
no esta escrito **no es un descuido: es una parte del sistema que nadie sabe que
existe**, que es peor que una que se sabe rota.

```text
   quien alcanza la RAM sin permiso   -> tiene fila
   quien no la alcanza                -> no es neutro, no tiene fila
```

> **Sacrificio:** anadir un aparato cuesta escribir su fila **antes** de que
> funcione, con la maquina apagada y sin la satisfaccion de verlo andar. Es el
> momento en que menos apetece.

---

## N2 -- TODO MARCO QUE UN NEUTRO PUEDA ESCRIBIR SE ETIQUETA

**Componente:** `mm/titular.rs`, clase `Titular::Neutro`.
**Numero:** `titular::neutros()`.

Un marco que un aparato tiene programado como destino de DMA **lleva su
etiqueta**. No `Anonimo`, que significa *sin opinion*.

```text
   antes   los marcos de DMA salian `Anonimo`
   ahora   salen `Neutro`
```

*** El motivo es exacto y no es de orden: **los unicos marcos del sistema en los
que escribe alguien a quien no se puede parar eran, justamente, aquellos sobre
los que el juez se callaba.** El marco mas peligroso de la maquina era el que no
tenia etiqueta.

> **Sacrificio:** cada sitio que pide DMA tiene que decirlo -- `alloc_frames_
> contig_de` y no `alloc_frames_contig`. Son cuatro hoy, y cada aparato nuevo
> son cinco lineas mas que se pueden olvidar. Y un byte por marco de tabla, ya
> pagado por las otras ocho clases.

---

## N3 -- UN MARCO NEUTRO NO SE DEVUELVE

**Componente:** `mm/titular.rs`, `puede_soltar`.

Los cuatro aparatos piden sus marcos **una vez, en el arranque**, y viven lo que
vive el kernel. Devolver uno significaria que el asignador se lo da a otro
mientras el aparato sigue escribiendo -- que es la pista **1.5** de la hoja del
07-09, con nombre y sin misterio.

```text
   quien lo pidio    lo tiene hasta que se apaga la maquina
   quien lo devuelva  se encuentra un `NoEsTuyo(Neutro, ...)`
```

> **Sacrificio:** esa RAM **no vuelve nunca**, ni siquiera cuando el aparato
> deja de usarse. Se paga memoria a cambio de la certeza de que nadie escribe
> donde no debe. Y el dia que un aparato quiera soltar de verdad, esta regla
> hay que cambiarla entera -- no ampliarla.

---

## N4 -- LA CUENTA DEL NEUTRO TIENE QUE ESTAR QUIETA

**Componente:** `titular::neutros()`.
**Numero:** `neutro=vivos:soltados`, en la fila `sys` del panel.

En una maquina sana ese numero **sube en el arranque y no se mueve mas**. Si
sube con la maquina en marcha, alguien esta repartiendo DMA en caliente, y eso
es lo primero que hay que ir a mirar.

```text
   quieto    los aparatos pidieron al arrancar. Normal
   subiendo  ** algo reparte DMA en caliente. Ir a mirar ANTES que nada
   bajando   ** N3 ROTA. Ver abajo
```

### ★★ Y LA CUENTA VIGILA A N3, que es lo que no se buscaba

La primera version de esta regla contaba recorriendo la tabla entera, y su
sacrificio era ese recorrido. **Al llevar la cuenta a `marcar` --el unico sitio
por el que un marco cambia de dueno-- aparecio algo mejor:**

Si la cuenta puede SUBIR cuando un marco pasa a `Neutro`, tambien puede BAJAR
cuando deja de serlo. Y eso es **exactamente lo que N3 prohibe**. Asi que
`soltados` cuenta las veces que se rompio, y tiene que ser **cero**.

[!] Lo importante es COMO: **desde el lado del marcado, sin tocar el camino de
devolucion de marcos**. Ese camino es ROJO, es donde vive la azul del 07-09, y
[`REQUISITOS.md`](REQUISITOS.md) R4 dice que no se toca hasta reproducirla.

> Se puede saber que una regla se rompio sin ponerse delante de ella.

> **Sacrificio:** dos comparaciones y una rama **en cada marcado de marco**, que
> es uno de los caminos mas transitados del kernel. Es barato, pero se paga
> siempre y no solo cuando alguien mira. Y hay un precio de diseno mayor: la
> cuenta ya **no es una medida independiente de la tabla** -- si `marcar` se
> equivoca, el numero se equivoca con el, y no queda quien lo desmienta.

---

## N5 -- UN NEUTRO NO OBTIENE CAPABILITY, Y ESO SE DICE EN VOZ ALTA

**Componente:** la ley del celo, en `docs/identidad/EL_ORQUESTAL.md`.

No es que se le deniegue el permiso: **es que la puerta no le aplica**. Una
capability es una promesa entre el orquestador y algo que le pide turno, y un
aparato neutro no pide turno.

> *"El orquestador es celoso con Ring 3 y CIEGO con el neutro."*

> **Sacrificio:** hay que escribir que el celo --que es la identidad de este
> sistema-- **tiene un limite**, y escribirlo en el mismo sitio donde se
> presume de el. Una ley que solo publica donde gana es publicidad.

---

## N6 -- NINGUNA REGLA DEL NEUTRO PUEDE NOMBRAR UN CPU

**Componente:** este fichero, y [`ARQUITECTURAS.md`](ARQUITECTURAS.md).

Las cinco de arriba no dicen *x86-64*, ni *PCIe*, ni *VT-d*. Y no es estilo: es
lo que permite que la carpeta valga el dia que BMO-X arranque en otra
arquitectura **sin reescribir una linea de la ley**.

```text
   la LEY dice     "la MMU de los aparatos"
   el PERFIL dice  VT-d, AMD-Vi, SMMU o IOMMU, segun donde arranque
```

** Es la LEY 24 --*el hardware se PERFILA*-- aplicada a una ley en vez de a un
driver.

> **Sacrificio:** se pierde el atajo. Escribir *"activar VT-d"* seria mas corto,
> mas concreto y mas facil de comprobar; escribir *"la MMU de los aparatos"*
> obliga a un nivel mas de indireccion **y a que alguien rellene el perfil**.
> Se paga claridad inmediata a cambio de que esto no caduque.

---

## ⚠ LA REGLA QUE NO EXISTE, Y ES LA IMPORTANTE

No hay una **N7** que diga *"un neutro solo escribe donde se le permita"*, y no
la hay por un motivo que no es pereza:

```text
   no se puede escribir una regla que no se puede hacer cumplir
```

Hacerla cumplir necesita **la MMU de los aparatos**, y no esta puesta. Escribir
N7 hoy seria poner en la ley una frase que ningun juez puede comprobar -- que es
exactamente lo que la ley 14 llama *"redactado en vez de escrito"*.

★ **El hueco se deja abierto y con su nombre**, que es lo que hace que un dia se
pueda cerrar en vez de descubrirlo con una pantalla azul delante. Ver
[`REQUISITOS.md`](REQUISITOS.md), requisito **R6**.
