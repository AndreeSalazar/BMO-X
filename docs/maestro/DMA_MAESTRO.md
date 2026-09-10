# DMA MAESTRO -- la disciplina del CUANDO, y que mitad del mundo copiar

> Escrito el **2026-09-09**. Peticion del dueno: *"prepara la estrategia con
> DMA [...] con IOMMU y DMA pero ambos maestros"*.
>
> Este contesta **CUANDO** puede escribir un aparato. El **DONDE** lo contesta
> [`IOMMU_MAESTRO.md`](IOMMU_MAESTRO.md), y son dos preguntas de verdad
> distintas: un aparato con DMA en vuelo sobre un bufer ya liberado escribe en
> una direccion que la IOMMU considera **legitima**.

---

# 1. QUE HACE EL MUNDO, Y SON TRES ESCUELAS

| | quien | la idea central |
|---|---|---|
| **Traspaso de propiedad** | Linux (`dma_map_*`), BSD (`bus_dma`) | un bufer es del CPU **o** del aparato, nunca de los dos |
| **Rebote** | `swiotlb` de Linux | si la direccion no vale, se copia a una que si |
| **Memoria sin tipo** | seL4 | el kernel no sabe que es DMA: da marcos y el usuario se apana |

## ★ La primera es la que hay que copiar, y BMO-X no la tiene

Linux no llama a su API *"dame una direccion fisica"*. La llama **mapear**, y
tiene un gemelo obligatorio:

```text
   dma_map_single()          el bufer pasa a ser DEL APARATO
   dma_sync_for_cpu()        vuelve a ser del CPU, para mirarlo
   dma_sync_for_device()     y otra vez del aparato
   dma_unmap_single()        se acabo: ya no es de nadie
```

*** El valor no esta en la traduccion de direcciones --en x86-64 coherente
muchas veces no traduce nada-- **esta en que el codigo tiene que DECIR de quien
es el bufer en cada momento**. Un `unmap` que falta es un fallo que se ve
leyendo, y un `free` antes del `unmap` lo caza un depurador.

** Eso es exactamente el escalon N-D del plan --el bit EN VUELO-- y aqui esta la
prueba de que no es un invento de esta casa: es lo que hacen los dos sistemas
grandes, y por el mismo motivo.

> El aparato no necesita permiso para escribir. Necesita que nadie le quite el
> sitio mientras escribe. Eso no es un permiso: es **propiedad con horario**.

---

# 2. [!] Y QUE SERIA UN ERROR COPIAR

## Error 1: la superficie de `dma_map_*` entera

La API de Linux tiene ~40 funciones. No porque el problema sea complicado, sino
porque sirve a **treinta arquitecturas**, con cinco clases de IOMMU, con caches
NO coherentes con el DMA, con memoria alta que un aparato de 32 bits no
alcanza, y con maquinas donde la direccion del bus no es la direccion fisica.

```text
   en x86-64 con DMA coherente y una sola placa PERFILADA:
      la traduccion         no hace falta: fisica == bus
      la invalidacion de cache   no hace falta: el DMA es coherente
      el `dma_mask` de 32 bits   SI hace falta: el PRDT de AHCI es 32+32
```

*** De cuarenta funciones sobreviven **dos ideas**: quien es el dueno ahora, y
cabe la direccion en el ancho que el aparato acepta. Copiar la forma en vez de
la idea seria traerse el precio de veintiocho arquitecturas que esta maquina no
es -- que es lo que LEY 24 prohibe por escrito.

## Error 2: el rebote como cimiento

`swiotlb` existe porque en 2003 habia aparatos de 32 bits en maquinas con mas de
4 GB. Es una **red de seguridad**, no una arquitectura: cada rebote es un
`memcpy` de ida y otro de vuelta.

** En BMO-X el rebote solo tiene sentido para lo que ya se copia igualmente. Y
la medida que lo decide ya se sabe hacer: `c/blit.bex` mide `memcpy` a RAM
contra `memcpy` al framebuffer. **Un rebote en el camino de la mano al pixel es
una regresion, y en el del disco puede que ni se note.**

## Error 3: el modelo de seL4, aunque sea el mas elegante

seL4 dice: *"el DMA no es asunto del kernel; toma marcos sin tipo y organizate"*.
Es coherente con un microkernel minimo y **es exactamente lo que BMO-X no puede
permitirse**, porque aqui los drivers son de la casa. Delegar la disciplina en
un usuario que somos nosotros es no tener disciplina.

---

# 3. ** LO QUE BMO-X YA HIZO BIEN SIN PROPONERSELO

```text
   drivers/net/anillo.rs:102   una ARENA: un corral contiguo entero
   dev/net/mod.rs:325          pedido como `Titular::Neutro`
```

La tarjeta de red no valida sus direcciones fisicas **porque no pueden estar
mal**: se calculan como `base + i * tamano` dentro de un corral que es suyo.

*** Eso es la tercera escuela, la que ningun sistema grande usa como norma
porque no puede --Linux tiene que servir a aparatos que no controla--. **BMO-X
si puede**, porque los tres aparatos con DMA de esta maquina son tres y estan
censados.

> Comprobar es lo que se hace cuando algo puede estar mal. Una arena hace que no
> pueda.

Y esa es la diferencia entre copiar y aprender: el mundo comprueba porque no
tiene mas remedio. Aqui se puede elegir el diseno en el que no hace falta.

---

# 4. LO QUE ESTE MAESTRO NO CONTESTA

```text
   [ ] no dice si un aparato PUEDE tocar una pagina. Eso es la IOMMU
   [ ] no dice cuanto tarda un rebote en esta placa. Eso se mide (LEY 24)
   [ ] no dice cuanto se espera a un aparato colgado antes de dar por perdido
       un DMA en vuelo. Es el paso N5 del plan, y es una decision con numero
```

Ver [`PLAN_EL_NEUTRO_VIGILADO.md`](../plan/PLAN_EL_NEUTRO_VIGILADO.md) para las
casillas, y [`EL_NEUTRO.md`](../identidad/EL_NEUTRO.md) para por que esto es una
categoria y no un fallo suelto.
