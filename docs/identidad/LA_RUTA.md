# LA RUTA -- se paga en la puerta una vez, y despues no hay puerta

> Pregunta del dueno, 2026-08-26:
>
> *"cuando la app pide permiso al gate, mi Ring 0 es burocracia -- segun los
> informes cuanto va a consumir, y dinamico. Pero luego de procesar en syscall
> ese mismo se elimina la burocracia y se es sin zonas de burocracia, pero
> CABINA esta viendo en tiempo real; si algo no cuadra el kernel lo detiene.
> Tiene sentido?"*
>
> **Si.** Y este documento existe porque la respuesta larga es mejor que la
> corta: **BMO-X ya lo hace en tres sitios, no lo ha nombrado nunca, y le falta
> exactamente la mitad que el dueno describio al final.**

---

# 0. LA FORMA, EN CUATRO TIEMPOS

```text
   1. DECLARAR    la app dice, en la puerta, que necesita
   2. JUZGAR      el kernel comprueba UNA VEZ y concede
   3. CORRER      no hay kernel en el camino. Ni un syscall
   4. VIGILAR     CABINA mira desde fuera; si no cuadra, se revoca
```

★★ **Los tiempos 1 y 2 son burocracia y se pagan enteros. El 3 no tiene ninguna.
Y el 4 es lo que permite que el 3 sea gratis sin ser ciego.**

Es la unica forma conocida de tener aislamiento y velocidad a la vez, y la frase
que la resume ya estaba escrita en este arbol antes de esta pregunta, en
`RED_MAESTRO.md`:

> **El kernel reparte el aparato una vez; despues se aparta.**

---

# 1. LOS TRES SITIOS DONDE YA VIVE

No es una idea nueva que haya que construir: es un patron que el arbol repitio
tres veces sin ponerle nombre.

| # | donde | se declara | se corre |
|---|---|---|---|
| 1 | **`KIND_FRAMEBUFFER`** | *"quiero la pantalla"* | el compositor escribe pixeles con `mov`. **Cero syscalls por pixel** |
| 2 | **`KIND_PRESTADO`** | *"ofrezco este trozo de MI memoria a ese TID"* | el otro lo lee con `mov`. El kernel no se entera |
| 3 | **el bufer del audio** | *"aqui van las muestras"* | el xHC lo lee por DMA. **El que lee no es el CPU** |

★ Y hay un cuarto planificado con la misma forma: los anillos de la NIC en
`RED_MAESTRO` paso 2. La tabla de aquel documento lo dice con numeros:

```text
                            syscall por trama   anillo compartido
   kernel en el camino             si, dos veces        no
   copias por trama                1-2                  0
   coste por trama                 un syscall           una lectura de memoria
```

## 1.1 -- ** Y el framebuffer lo dice mejor que ninguno

Su propio fichero lleva la frase desde que existe:

> *"Ese es el momento library-OS: **no se optimiza el cruce de frontera, se
> borra la frontera**."*

**Eso es la ruta.** Lo que el dueno describio no es una propuesta nueva: es la
generalizacion de lo que este sistema ya hace tres veces.

---

# 2. ⚠ LA MITAD QUE FALTA, Y ES LA QUE EL DUENO PUSO AL FINAL

Los tiempos 1, 2 y 3 estan. **El 4 no.**

```text
   hoy:    el kernel concede y se aparta -- Y YA NO MIRA
   falta:  el kernel concede, se aparta, Y SIGUE MIRANDO DESDE FUERA
```

## 2.1 -- Por que sin el 4 la ruta no se puede ensanchar

Con tres consumidores conocidos --el compositor, el audio, un prestamo entre dos
procesos que se eligieron-- que el kernel no mire despues **es aceptable**: lo
que se cede es pequeno y el que lo recibe es de la casa.

*** El dia que la ruta la pida un tercero, deja de serlo. Y no por malicia: **una
app que declara que va a leer un anillo y en su lugar lo llena sin parar no esta
atacando a nadie -- esta teniendo un bug**, y el efecto sobre el sistema es el
mismo. Sin el tiempo 4 no hay forma de notarlo hasta que algo se cae.

## 2.2 -- ★★ Y ya existe el primer ejemplo de ese cuarto tiempo

**La patada** (`core/emergencia.rs`, 26-08). Es literalmente esto:

```text
   el kernel ve que algo no cuadra   -> `emergencia::declarar`
   lo mira quien puede actuar         -> el hilo del bus, cada 4 ms
   y REVOCA                           -> se lleva la pantalla y lo explica
```

Lo que la patada vigila hoy es **el kernel a si mismo** --sus tablas de pagina--
y no a la app. Pero el mecanismo es el mismo, y el sitio donde mirar tambien:
una bandera que se levanta donde se ve el problema, y un hilo sin cerrojos que
la recoge.

★ O sea que el tiempo 4 no hay que inventarlo. Hay que **apuntarlo hacia
afuera**.

---

# 3. QUE HABRIA QUE DECLARAR, Y AQUI ESTA LA PARTE DIFICIL

El dueno lo dijo: *"segun los informes cuanto va a consumir, y dinamico"*. Y ahi
esta el problema que hay que resolver antes de escribir una linea.

## 3.1 -- Lo que una app declara HOY

`R-APP2`: *declara lo que necesita, y no puede mentir.* El `.bex` ya trae en su
cabecera lo que pide, y el cargador lo juzga: version del ABI, extensiones del
CPU que necesita, banderas. **Todo eso es estatico**: se comprueba una vez, al
admitir, y no cambia.

## 3.2 -- ⚠ Lo dinamico NO se puede declarar igual, y confundirlo seria caro

```text
   estatico    "necesito AVX2"          se comprueba UNA VEZ y es cierto siempre
   dinamico    "voy a usar 40 MB/s"     no es un hecho: es una INTENCION
```

*** Una intencion no se puede comprobar en la puerta. Solo se puede **comparar
despues** con lo que de verdad paso -- y por eso el tiempo 4 no es un adorno del
diseno: **es lo unico que hace que declarar algo dinamico signifique algo.**

## 3.3 -- La regla que sale de ahi

> **Lo estatico se juzga en la puerta. Lo dinamico se declara en la puerta y se
> juzga MIRANDO.**

Y las dos van con nombre, como todo lo demas de esta casa: un `.bex` rechazado
en la puerta dice por que; una ruta revocada por no cuadrar tiene que decir
**que declaro y que hizo**, los dos lados. Un `bool` frena sin decir como
arreglarlo.

## 3.4 -- ★★ LA MAQUINA YA ESTA CONSTRUIDA, Y NO SE LLAMA RAYOS X

> Segunda idea del dueno, el mismo dia: *"la maquina del aeropuerto no necesita
> verificar ni estorbar, sino que esta viendo como rayos X todo el tiempo, en
> categorias diferentes"*.

**La forma es correcta y la palabra hay que corregirla**, porque la diferencia
decide lo que este sistema puede prometer.

```text
   RAYOS X    esta EN EL CAMINO. La maleta pasa por dentro. Ve ANTES,
              y por eso puede parar la maleta -- pero estorba, aunque poco
   RADAR      NO esta en el camino. Ve DESPUES, y a todos a la vez,
              sin tocar a ninguno
```

*** BMO-X no puede tener rayos X en la ruta: estar en el camino **es** la
burocracia que se acaba de quitar. Lo que si puede tener --y ya tiene-- es un
radar.

### Y la palabra ya estaba elegida en el arbol

`cabina/radar.rs` existe desde el 25-08, y no es una metafora suelta: es
**exactamente la maquina** que el dueno describe.

| lo que hace | por que sirve aqui |
|---|---|
| cuenta en el **ORIGEN**, antes del cerrojo del anillo | un `fetch_add`. No estorba a nadie |
| una matriz de **capa x severidad** (8 x 5 = 40) | son las *"categorias diferentes"* |
| **ninguna cuenta gira jamas** | lo que paso sigue contado aunque el detalle se pierda |

Y trae escrita la frase que explica por que un vigilante que filtra no vale:

> *"Un radar que pierde un contacto y dibuja la pantalla vacia no es un radar con
> menos alcance. Es una pantalla."*

★ O sea que la respuesta a *"tiene sentido?"* es mas fuerte que un si: **la
maquina esta hecha, funciona, y hoy solo mira a una cosa -- al kernel.** Lo que
falta es una segunda matriz con la misma forma, mirando a los procesos.

## 3.5 -- LAS CINCO CATEGORIAS, Y LO QUE CUESTA CADA UNA

El dueno dijo cinco. Salen cinco, y **cuatro ya se cuentan hoy**: no hay que
anadir trabajo al camino, hay que **leer lo que ya se cuenta**.

| # | categoria | quien lo cuenta YA | que significa que no cuadre |
|---|---|---|---|
| 1 | **puertas** | `syscall/meter.rs`, histograma por clase | declaro que iba a ir por la ruta y esta cruzando la frontera igual |
| 2 | **memoria** | `phys::stats()` + `obj::memory` | pidio un bloque y sigue pidiendo |
| 3 | **tiempo** | el quantum del planificador | no cede: se come su turno entero, siempre |
| 4 | **aparatos** | los atomicos de `fb`, `input`, `audio`, `mmio` | tiene la pantalla y no la suelta |
| 5 | **la ruta** | ver abajo -- **y es la unica que parecia imposible** | le dieron un anillo y no lo esta atendiendo |

### *** LA QUINTA: no se cuentan los bytes, se leen los DOS INDICES

Esta es la que parecia obligar a estar en el camino, y no obliga.

En todo anillo hay ya **dos numeros en memoria compartida** que dicen hasta
donde llego cada lado. Leerlos son dos cargas y **no toca un solo byte del
dato**:

```rust
   // dev/usb/audio.rs, y ya existe
   pub fn pendientes() -> u64 { p.escrito.saturating_sub(p.leido) }
```

★★ **Eso es el radar aplicado a la ruta.** No se escanea el equipaje: se mira la
silueta. Y no es un plan -- el tubo del audio ya expone `escrito`, `leido`,
`pendientes` y `huecos`, que son exactamente las cuatro caras de esa silueta.

[!] Y por eso las categorias son cinco y no cincuenta. **Una categoria solo vale
si alguien ya la cuenta**, o si contarla es leer dos numeros que ya existen. Todo
lo demas es un vigilante en el camino con otro nombre.

---

# 4. ⚠ Y LO QUE ESTA RUTA **NO** VA A HACER MAS RAPIDO

Va aparte porque la pregunta venia con un objetivo pegado: **token/s**.

## 4.1 -- La respuesta corta: no la toca

`PLAN_EL_ASISTENTE.md` parte 8 ya lo demostro, y la formula no es la que la
intuicion espera:

```text
                     ancho de memoria (GB/s)
   tokens/s  =  ---------------------------------
                  lo que ocupa el modelo (GB)
```

Generar texto de uno en uno **no esta limitado por el calculo**: esta limitado
por lo rapido que la memoria entrega bytes. Para producir UN token hay que leer
los pesos enteros del modelo, una vez, todos.

## 4.2 -- Los dos numeros, uno al lado del otro

```text
   por el lado del CALCULO    ~30 tokens/s   (6 nucleos, 4,49 GHz, AVX2+FMA)
   por el lado de la MEMORIA  ~10 tokens/s   (SI son 2 canales DDR4-3200)
```

*** **El CPU sobra tres veces.** Un camino sin burocracia que ahorra ciclos de
CPU no mueve un numero que decide la DRAM. La ruta es correcta como
arquitectura, y **para este objetivo concreto no es la palanca**.

## 4.3 -- ★★ La palanca de verdad, y es una tarde

**El ancho de memoria de este Ryzen no se ha medido nunca.** Se sabe que son
15.178 MiB; no se sabe a que velocidad ni en cuantos canales.

```text
   si sale ~45 GB/s   un 7B en Q4 da ~10 tok/s  -> banda util, se puede charlar
   si sale ~20 GB/s   hay que bajar a un 3B
```

Sin ese numero, todo lo de arriba es aritmetica sobre un supuesto -- y este
proyecto tiene una ley para eso: **se pregunta, no se supone.**

## 4.4 -- Y el hallazgo del 26-08, que estaba escondido en el censo

`cpu_vendor/features/usage.rs` declaraba AVX, AVX2 y FMA **imposibles**:
*"sem-asm no sabe VEX"*. Dejo de ser cierto el **23-08**: hay cinco filas VEX en
`intrinsics.toml`, y una de ellas es

```text
   avx_funde4  =  vfmadd231pd   cuatro flotante64, multiplicar y acumular
```

que es, con las palabras de esa misma tabla, *"la operacion de la que esta hecho
un producto de matrices"*.

★ **Estado real: construido, probado, y con CERO clientes en todo el arbol.**

[!] Y la leccion es la del propio modulo, con el espejo que le faltaba. Decia
*"un `Yes` sin sitio nombrado es un `Yes` que miente"*. **Un `No` cuyo motivo
dejo de ser cierto es peor**: no dice *"todavia no"*, dice *"no se puede"* -- y
quien lo lea deja de mirar, con la instruccion ya escrita a dos carpetas.
Corregido.

---

# 5. EL ORDEN QUE PROPONE ESTE DOCUMENTO

### Paso 1 -- ★ MEDIR EL ANCHO DE MEMORIA

Una tarde. Leer un bloque grande y cronometrarlo. **Decide si el asistente cabe
en esta maquina y con que modelo**, y hasta que exista, la parte 4 es aritmetica
sobre un supuesto.

### Paso 2 -- El tiempo 4 apuntado hacia afuera

La patada ya es el mecanismo. Lo que falta es que lo que vigile no sea solo el
kernel a si mismo. **Y antes de escribirlo hay que decidir que se declara**, que
es la parte 3 de este documento.

### Paso 3 -- Nombrar la ruta en el contrato

Hoy son tres casos que se parecen. El dia que sea un concepto con nombre, un
tercero puede pedirla -- y ese es el dia en que el tiempo 4 pasa de conveniente
a obligatorio.

---

# 6. LO QUE ESTE DOCUMENTO NO PROMETE

- **Que la ruta haga nada mas rapido hoy.** Sus tres instancias ya corren; esto
  las nombra y dice que les falta.
- **Que medir la memoria mejore el numero.** Lo que hace es convertir una
  estimacion en un hecho, y puede salir peor de lo esperado.
- **Que el radar llegue a tiempo para todo.** Mira desde fuera, o sea que **ve
  TARDE**. El hilo del bus late cada 4 ms, y esa es su resolucion.

```text
   una app que abusa de un recurso     4 ms de retraso no cambia nada
   una tarjeta escribiendo por DMA     4 ms es una eternidad
```

  *** Y esa segunda fila **no la arregla ningun radar**, por bueno que sea: la
  tarjeta no pasa por la MMU, asi que cuando el dato aparece ya esta escrito.
  Eso solo lo cierra una IOMMU. **El radar cubre el abuso de recursos y no
  cubre la corrupcion**, y confundir las dos seria vender lo primero como si
  fuera lo segundo.
