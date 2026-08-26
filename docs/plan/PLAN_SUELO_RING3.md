# PLAN SUELO RING 3 -- las tres cosas que hay que construir antes de bajar nada

> Escrito el **2026-08-26**, despues del censo de
> [`RING3_MAESTRO.md`](../maestro/RING3_MAESTRO.md) y con el visto bueno del
> dueno: *"si, tira por ahi, empieza con el suelo de Ring 3"*.
>
> El censo dijo QUE baja y en que orden. Esto dice **como se construye el suelo
> sobre el que puede bajar**, y las tres piezas no bajan ni una linea: son el
> mecanismo.
>
> ⚠ **Y la parte 2 es una decision de seguridad, no de diseno.** Va escrita
> antes de tocar codigo a proposito: es exactamente la clase de cosa que el
> dueno pidio que no le sorprendiera.

---

# 0. LAS TRES PIEZAS, Y EN QUE ESTADO ESTAN

| # | pieza | estado | quien la necesita |
|---|---|---|---|
| **S1** | MMIO por direccion fisica | molde en `obj::fb::claim` | todo driver |
| **S2** | memoria DMA con fisica conocida | molde en `MEM_OP_OFRECER` + `vmm::translate` | todo driver, y `RED_MAESTRO` paso 2 |
| **S3** | una IRQ que despierta a un proceso | **no existe** | todo driver que no quiera sondear |

★ Ninguna de las tres baja codigo a Ring 3. Construyen el suelo, y esa es justo
la parte que un plan optimista se salta.

---

# 1. S1 -- MMIO POR DIRECCION FISICA

## 1.1 -- El molde que ya existe

```rust
   // obj/fb.rs::claim
   vmm::map_page_wc(aspace, FRAMEBUFFER_VA_BASE + off, fisica + off, true, true)
```

Un proceso reclama la pantalla y el kernel le mapea memoria fisica de un
aparato en su espacio. **Eso es un driver de Ring 3 con el aparato clavado.**

Dos diferencias con lo que hace falta:

```text
   la fisica    viene de `info::FB_ADDR`, o sea CLAVADA -> tiene que ser un dato
   la cache     WC (write-combining), que es de pantallas
                un registro de control se mapea UNCACHEABLE o no funciona
```

## 1.2 -- ⚠⚠ LA DECISION QUE HAY QUE TOMAR ANTES DE ESCRIBIR NADA

> **Un proceso que puede decir *"mapeame la fisica 0x1000"* es un proceso que
> esta pidiendo ser el kernel.**

Y no es una exageracion. Con esa operacion, en tres pasos:

```text
   1. mapea la fisica donde viven las tablas de pagina
   2. se pone el bit U/S y quita el NX donde quiera
   3. ya no hay muro 2 ni muro 3
```

*** **Los siete muros de [`EL_AISLAMIENTO.md`](../identidad/EL_AISLAMIENTO.md)
se caen todos a la vez**, y no por un bug: por la propia operacion, funcionando
como se pidio.

### Por eso el proceso NO nombra una direccion. Nombra un APARATO.

```text
   MAL   mapear_fisica(0xF7A0_0000, 0x1000)     <- el proceso elige
   BIEN  aparato_mapear(handle_de_aparato)      <- el KERNEL elige la fisica
```

El proceso pide *"el aparato que me concedieron"*; el kernel mira **su propio
censo de PCI**, saca el BAR, lo juzga, y mapea. Es la misma forma que
`KIND_FRAMEBUFFER` y `KIND_AUDIO`: exclusivo, concedido, y con `soltar`.

★ Asi la superficie nueva sigue la regla `R-REX3` sin discusion: **conceder un
aparato es AUTORIDAD**, o sea que le toca operacion.

## 1.3 -- EL JUEZ: `bmo-mmio-juicio`

Aun eligiendo el kernel la direccion, hay rangos que **no se pueden ceder
aunque sean MMIO de verdad**. Esto es una funcion pura, sin `unsafe`, probada en
el anfitrion -- misma forma que `bmo-disco-juicio` y `bmo-bex-gate`.

| veto | por que |
|---|---|
| `PisaRam` | si el rango solapa memoria usable, Ring 3 gana una ventana a la RAM del kernel. **Es el veto que sostiene todo lo demas** |
| `EsElApic` | `0xFEC0_0000 .. +0x140_0000` es LAPIC / IO-APIC / HPET. Ceder el APIC es ceder el control de las interrupciones, o sea **ceder Ring 0 con otro nombre** |
| `DebajoDeUnMega` | el megabyte legacy (BIOS, VGA, la rampa de SMP) no es de nadie |
| `NoAlineado` | una pagina es la unidad minima. Ceder media pagina no existe |
| `MasPequenoQueUnaPagina` | ⚠ un BAR de 256 bytes ocupa una pagina, y **en esa pagina pueden vivir los registros de otro aparato**. Conceder eso es conceder dos cosas y nombrar una |
| `YaTieneDueno` | dos procesos con el mismo aparato es el bug de `KIND_AUDIO` del 09-08, otra vez |

★★ **Y la regla que ordena el juez, copiada de `bmo-disco-juicio`: ninguna
funcion contesta `true` por defecto. Cuando falta un dato, la respuesta es la
que no asume.**

[!] Lo que este juez **no** puede prometer: que el aparato mapeado no haga DMA a
donde no debe. Eso no lo para ningun bit de la CPU -- hace falta una IOMMU. Ver
la parte 4.

---

# 2. S2 -- MEMORIA DMA CON FISICA CONOCIDA

## 2.1 -- Lo que ya hay

`MEM_OP_OFRECER` / `TASK_OP_TOMAR` prestan un bloque entre procesos, y
`vmm::translate` da la fisica. Falta **exponerla** y **prometer que no se
mueve**.

## 2.2 -- La promesa, que hoy es gratis y hay que escribirla igual

Hoy nada mueve un marco despues de asignarlo: no hay swap, ni compactacion, ni
paginas grandes que se partan. **Asi que la promesa cuesta cero y hay que
escribirla precisamente por eso**: el dia que alguien anada cualquiera de esas
tres, el que la lea sabra que hay un contrato que respetar. Una propiedad
verdadera por accidente deja de serlo sin que nadie lo note.

## 2.3 -- [!] Y por que exponer una fisica es menos grave que mapear una

```text
   mapear una fisica    te da PODER sobre esa memoria
   saber una fisica     te da un NUMERO
```

Un numero solo es peligroso si algo lo acepta como orden. En este sistema lo
unico que lo aceptaria es un aparato haciendo DMA -- o sea el mismo problema de
la parte 4, y no uno nuevo. **Aun asi se concede por capability y no por
llamada libre**, porque saber donde vive la memoria del vecino no le hace falta
a nadie.

---

# 3. S3 -- LA INTERRUPCION QUE DESPIERTA A UN PROCESO

## 3.1 -- ★★ Y aqui BMO-X ya tiene ganado lo mas caro, sin buscarlo

`WAIT` es syscall congelado desde el principio. Un driver de Ring 3 esperando
una interrupcion **no necesita una llamada nueva**:

```text
   IRQ 11 --> manejador minimo en Ring 0
              enmascarar la linea + EOI + `reply_to` sobre un endpoint
                                  |
                                  v
                    el driver de Ring 3 vuelve de WAIT
```

El manejador de Ring 0 se queda en **decenas de lineas** y no sabe lo que es un
TRB. Eso es lo que permite bajar las 2.143 lineas del xHCI sin hundir la
latencia.

## 3.2 -- Las tres cosas que ese manejador tiene que hacer bien

| | por que |
|---|---|
| **enmascarar antes de despertar** | si no, la linea vuelve a dispararse mientras el driver de Ring 3 todavia no ha corrido, y el kernel se ahoga en IRQs |
| **el EOI lo manda RING 0** | el APIC no se cede (ver el veto `EsElApic`) |
| **desenmascarar es una operacion del driver** | *"ya termine, vuelve a avisarme"*. Es el equivalente de *"el evento ES el permiso para volver a encolar"* que ya costo el teclado |

⚠ **Y el fallo que hay que impedir por diseno: un driver de Ring 3 que muere con
su linea enmascarada deja el aparato mudo para siempre.** Al morir el proceso,
el kernel tiene que desenmascarar o dejar la linea marcada como huerfana. Es
`R-APP6` --*muere sin llevarse a nadie*-- aplicado a una IRQ.

## 3.3 -- El precio, que ya esta medido

**969 ciclos por puerta.** A 250 latidos/s (el audio) son 242.000 ciclos/s de un
CPU que da miles de millones: no se nota. Para la red a gigabit habria que
mirarlo, y por eso `RED_MAESTRO` no mete el syscall en el camino del dato -- el
anillo se comparte.

---

# 4. ⚠ LO QUE ESTE SUELO **NO** ARREGLA, DICHO ANTES DE EMPEZAR

## 4.1 -- El DMA sigue sin muro

Un driver en Ring 3 que construye mal un descriptor hace que **la tarjeta**
escriba donde no debe. La tarjeta no tiene anillo: el DMA no pasa por la MMU del
CPU.

```text
   bajar un driver a Ring 3   ->  su PARSER ya no puede corromper el kernel
                              ->  su DESCRIPTOR todavia si
```

Eso solo lo cierra una IOMMU (AMD-Vi en esta maquina), y es otro proyecto.
**Decirlo aqui es la diferencia entre un plan y un folleto.**

## 4.2 -- Y no habria evitado la pantalla azul del 25-08

El unico `#GP` de Ring 0 que se ha pagado estaba en `mm/vmm.rs`, o sea en la
clase que **no puede bajar**. Este suelo hace el sistema mas dificil de romper
desde fuera; no hace el kernel mas correcto por dentro.

---

# 5. EL ORDEN, Y COMO SE PRUEBA CADA UNO

### Paso 1 -- el juez, con sus pruebas  (`bmo-mmio-juicio`)

Funcion pura, en el anfitrion, cero riesgo. **Y se cablea en el mismo commit que
su llamante** -- la regla de la casa es *cablear o borrar*, y una libreria
huerfana ya costo once crates en este arbol.

### Paso 2 -- `KIND_MMIO`: conceder, mapear, soltar

**Como se prueba en metal**: un `.bex` que mapea el registro de version del xHC
y lo imprime. **Cero escrituras.** Si lee lo mismo que dice `cabina`, el camino
esta vivo.

★ Y la prueba de que el juez vale es la contraria: pedir un rango que pise RAM
tiene que salir con **su nombre**, no con un fault.

### Paso 3 -- la fisica del bloque, y la promesa por escrito

### Paso 4 -- la IRQ sobre `WAIT`

**Como se prueba**: un `.bex` que hace `WAIT` sobre el timer y cuenta 250
despertares en un segundo. Sin driver, sin aparato, sin riesgo.

### Paso 5 -- el primero que baja de verdad: `usb/uaudio`

1.135 lineas, cero `unsafe`, y **se elige precisamente porque NO es el
peligroso**: come un buffer y devuelve numeros. Es la prueba del mecanismo con
la pieza mas barata de equivocarse.

---

# 6. LO QUE ESTE PLAN NO PROMETE

- **Que el suelo sea rapido de construir.** Son cuatro pasos y ninguno se puede
  probar del todo sin arrancar la maquina.
- **Que baje `xhci`.** Es el que mas `unsafe` tiene y el que mas DMA hace: baja
  al final o no baja.
- **Que esto cierre el DMA.** No lo cierra. Ver la parte 4.
- **Y ninguna fecha.** El orden es lo unico que este documento afirma.
