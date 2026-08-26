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
- **Que el tiempo 4 sea barato.** Vigilar sin estorbar es dificil: un vigilante
  que mira en el camino del dato **es la burocracia que se acaba de quitar**.
  Tiene que mirar desde fuera, y eso significa que ve tarde. Cuanto tarde es la
  pregunta que ese paso tendra que contestar con un numero.
