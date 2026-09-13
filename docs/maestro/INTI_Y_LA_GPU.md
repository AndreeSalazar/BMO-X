# INTI Y LA GPU -- por que la unidad no es el REGISTRO, y que si lo es

> Escrito el **2026-09-10**, a partir de una idea del dueno:
>
> *"que potencial tiene INTI para que hablen como intermedio directo en
> registro de CPU y GPU, que es la que se hablan para orquestar por completo?"*
>
> Y con su propia condicion delante: *"ahora necesito que todo funcione bien en
> base en x86-64 para no tener choques luego cuando venga lo exterior"*. Este
> documento **no propone escribir nada de GPU todavia**. Propone decidir la
> FORMA, que es lo que evita el choque.

---

# 1. LA INTUICION ES CORRECTA, Y EL NOMBRE NO

La idea de fondo --*que haya UNA cosa que hable con los dos y reparta el
trabajo*-- es exactamente lo que le falta a este sistema. Lo que no encaja es la
palabra **registro**.

## Una GPU no se maneja por registros. Se maneja como el disco

```text
   CPU  <-> CPU     registros, y la instruccion siguiente ya los ve
   CPU  <-> GPU     un PAQUETE escrito en RAM, y un TIMBRE en MMIO
```

Escribir en un registro de la GPU no encola trabajo: **configura**. El trabajo
se encola escribiendo paquetes de comando en un anillo que vive en la RAM, y
tocando un timbre --un `doorbell`-- para decir *"hay algo nuevo"*. La tarjeta lo
lee **ella sola, por direccion fisica**.

*** Y esa frase ya se ha escrito tres veces en este arbol, para el AHCI, para el
xHCI y para la NIC. Es la misma forma:

```text
   AHCI   Command List + Command Table + PRDT   y una campana en PxCI
   xHCI   anillos de TRB + DCBAA + ERST         y una campana en el doorbell
   GPU    anillos de paquetes PM4               y una campana en el doorbell
```

** Lo que quiere decir que **la GPU no es una frontera nueva: es el cuarto
aparato del NEUTRO**. Y las ocho reglas de `NEUTRO/DMA/` le aplican enteras el
primer dia, sin escribir ninguna regla nueva. Eso ya es parte de la respuesta a
*"no quiero choques luego"*.

## Lo que SI es un registro, y es lo unico

El timbre. Una escritura de 32 o 64 bits a MMIO, sin datos dentro. Todo lo
demas --que hacer, con que, donde dejarlo-- viaja por RAM.

  > Con una GPU no se habla. Se le deja trabajo escrito y se llama a la puerta.

---

# 2. ENTONCES, QUE PUEDE HACER INTI

Si la unidad no es el registro, la unidad es **la PARTE**: un trozo de trabajo
que se puede describir sin decir quien lo ejecuta.

Y eso **ya existe en este arbol**, con ese nombre y ese argumento:
[`platform/shared/bmo-orquesta`](../../platform/shared/bmo-orquesta), *"LA
PARTITURA: que partes existen, y como se reparte una entre n atriles"*. Hoy
reparte entre **nucleos de CPU**, y su cabecera ya dice por que vive fuera del
kernel: repartir es aritmetica, y la aritmetica se prueba en el anfitrion.

## La propuesta, en una linea

> **Que INTI sepa escribir una PARTE, y que quien la ejecuta se decida despues.**

```text
   hoy      el programa dice: haz esto, en este bucle, en este nucleo
   la parte el programa dice: esto hay que hacerlo sobre ESTE rango
            y el repartidor dice: doce atriles -> doce trozos
                                  una GPU      -> un paquete
```

## Por que INTI y no C

Tres razones, y la tercera es la que decide:

1. **INTI ya tiene dos perfiles** (LLANO y PLENO). Una parte es un tercer
   perfil natural: lo que se puede repartir es un subconjunto de lo que se puede
   escribir, y INTI ya sabe tener subconjuntos con nombre.
2. **INTI tiene CERO UB comprobado en el Ryzen** (22-08, `reglas = 0`). Repartir
   una cuenta entre doce atriles multiplica por doce cualquier ambiguedad de
   orden; un lenguaje donde el orden no es ambiguo no tiene ese problema.
3. *** **En C esto no se puede decir.** Un bucle de C dice COMO se recorre, no
   QUE se calcula, y de ahi no se saca un reparto sin adivinar si las vueltas
   son independientes. Y adivinar es justo lo que esta casa acaba de prohibir
   (`PLAN_NUNCA_ADIVINA.md`). **INTI puede exigir que lo digas.**

---

# 3. LO QUE HAY HOY, MEDIDO

Para que la propuesta no sea una promesa, esto es lo que existe y lo que no:

```text
   HECHO       plat/smp/crew.rs -- 12 nucleos, 11,27x MEDIDO, y duermen con MWAITX
   HECHO       bmo-orquesta -- el catalogo de partes y el reparto entre n
   HECHO       INTI con su emisor propio (bmo-inti-x86-64, 256 filas)
   A MEDIAS    la puerta desde Ring 3 al reparto: `crew` existe, la puerta no
   NO HAY      GPU. Ni tarjeta, ni anillos, ni ISA
   EL MURO     el PSP: doce mensajes con esperas de 20 ms y una de 3 s, y al
               otro lado firmware firmado que no se lee ni se depura
```

** `platform/drivers/gpu/rdna4/src/psp.rs` lo dice sin adornos: *"No hay
tarjeta. Escribir aqui lecturas y escrituras de MMIO seria escribir codigo que
nadie puede ejecutar ni comprobar"*. Por eso es una maquina de estados juzgable
y no un driver.

  > Un driver para hardware que no se tiene no es adelantar trabajo: es escribir
  > la respuesta antes de oir la pregunta.

---

# 4. EL ORDEN QUE EVITA EL CHOQUE

El dueno lo pidio asi, y es el orden correcto:

## Primero: x86-64 solido, y eso significa DOS cosas concretas

1. **La puerta de `crew` desde Ring 3.** El reparto entre doce nucleos ya
   funciona y da 11,27x; lo que no hay es como pedirlo desde una app. Es un
   `kind` y una operacion, no un mecanismo nuevo.
2. **`bmo-orquesta` como el UNICO sitio donde se decide un reparto.** Hoy su
   guardian ya compara el catalogo con las funciones de Ring 0 y rompe el build
   si divergen. Ese mismo guardian es el que evitara el choque el dia que haya
   un tercer ejecutor.

## Despues: la PARTE como forma de INTI

Y aqui esta la decision que hay que tomar **ahora** aunque no se escriba nada:

```text
   una parte declara    QUE calcula y sobre QUE rango
   una parte NO declara quien la ejecuta, ni en cuantos trozos
   una parte PROMETE    que sus trozos no se pisan
```

*** Esa tercera linea es la que hay que exigir en el lenguaje, no deducir. Es la
version de *"nunca adivina"* aplicada al paralelismo: **si el programa no puede
demostrar que sus trozos son independientes, no se reparte**.

## Y al final: la GPU como un EJECUTOR mas, no como un lenguaje mas

Si las partes existen y el reparto vive en un sitio, anadir la GPU es:

```text
   1. el PSP contesta                    (el muro de verdad, y es de firmware)
   2. un anillo de paquetes en RAM       -> NEUTRO, y las 8 reglas ya aplican
   3. un timbre en MMIO                  -> bmo-mmio-juicio ya juzga cesiones
   4. traducir una parte a un paquete    <- lo unico nuevo
```

** Tres de los cuatro pasos ya tienen su juez escrito. Eso es lo que se compra
decidiendo la forma antes: **cuando llegue la tarjeta, es un backend, no una
reescritura.**

---

# 5. LO QUE ESTE DOCUMENTO DECIDE, Y LO QUE DEJA ABIERTO

## Decidido

- La unidad es **la parte**, no el registro. El registro es solo el timbre.
- La GPU es **el cuarto aparato del NEUTRO**, y hereda las ocho reglas del DMA.
- El reparto vive en **`bmo-orquesta`** y en ningun otro sitio.
- Una parte **declara** su independencia; no se deduce.

## Abierto, y a proposito

- **Como se escribe una parte en INTI.** Sintaxis, y si es un perfil o una
  forma dentro de PLENO.
- **Quien decide el reparto en ejecucion**: el programa, el kernel, o una
  politica. Tiene que ver con el FOCO --*"orquestar es ser celoso"*-- y esa
  conversacion todavia no se ha tenido.
- **Si la GPU llega antes que las partes.** Si pasa, se hace al reves: un anillo
  a mano, y las partes despues. No seria un desastre; seria mas trabajo.

## [!] Y lo que NO se hace hoy, dicho para que no se espere

No se escribe codigo de GPU. No hay tarjeta, el PSP es firmware que no se puede
depurar, y este arbol tiene una regla sobre eso que lleva escrita desde agosto:
**no se escribe lo que nadie puede ejecutar**.

  > La forma se decide antes porque es gratis. El codigo se escribe despues
  > porque no lo es.

---

# 6. ★★ INDEPENDIENTES Y DEPENDIENTES A LA VEZ (2026-09-12)

> El dueno, jugando Left 4 Dead 2: *"mi GPU trabaja independiente de la CPU si
> es frame... necesito que cambies los planes para que la CPU y la GPU sean
> independientes y al mismo tiempo dependientes"*.

La observacion es exacta, y cambia el plan en un punto concreto: **la seccion 4
decia COMO se le da trabajo a la GPU (la parte, el anillo, el timbre), y no decia
nada de RITMO.** Esta seccion es el ritmo.

## 6.1 Lo que pasa de verdad en un juego como L4D2

```text
   CPU   logica del juego -> prepara el fotograma N -> lo ENCOLA -> sigue con N+1
   GPU                          <- toma el N de la cola -> lo pinta -> lo presenta
```

Son **dos relojes**. La CPU no espera a que la GPU pinte el N para empezar el
N+1, y la GPU no sabe nada de la logica del juego. Eso es la INDEPENDENCIA.

Y la DEPENDENCIA esta en tres sitios, y solo en tres:

```text
   1. la COLA        la CPU puede adelantar como mucho K fotogramas. Si la
                     llena, espera. (El "fotogramas pre-renderizados" de los
                     drivers es esa K.)
   2. el TESTIGO     la GPU publica "termine el N". Quien necesita saberlo lo
                     mira; nadie mas espera
   3. los DATOS      lo que viaja en el fotograma N no se puede tocar mientras
                     la GPU lo lee
```

★ De ahi sale el dato que todo jugador ve sin nombrarlo: **los fps son el MINIMO
de los dos relojes**. Si la CPU es la lenta, la GPU espera con la cola vacia
(*CPU-bound*). Si la GPU es la lenta, la cola se llena y la CPU espera
(*GPU-bound*). Una maquina bien orquestada sabe **cual de las dos es** en cada
momento, y lo dice.

## 6.2 ** Y BMO-X ya tiene la mitad, sin GPU

`<bmo/superficie.h>` resolvio el punto 2 el mes pasado, entre dos procesos de
CPU, y con la misma forma:

```text
   la app          pinta en SU memoria y sube SECUENCIA cuando el dibujo
                   esta entero                              (productor)
   el DIRECTOR     compone a SU ritmo, y solo repinta si ve una SECUENCIA
                   distinta de la ultima                    (consumidor)
```

Nadie espera a nadie: una app colgada no se lleva el escritorio, y un escritorio
ocupado no frena a la app. **Es el modelo de L4D2 con la app haciendo de CPU y el
DIRECTOR haciendo de GPU.** Cuando llegue una tarjeta no hay que inventar el
contrato: hay que ponerle un tercer participante.

Lo que NO tiene todavia, y es exactamente lo que la pregunta del dueno pide:

```text
   la COLA acotada   hoy la "cola" es de 1 y se pisa: el DIRECTOR ve el ultimo
                     entero. Correcto para ventanas; un reproductor de video
                     necesita K > 1 y SABER cuantos se perdieron
   el TESTIGO DE     la app no sabe cuando se PRESENTO su fotograma, solo que
   VUELTA            lo publico. Sin eso no puede medir su latencia ni
                     esperar sin girar
   el VEREDICTO      nadie dice hoy si una app va limitada por su propio calculo
                     o por el compositor
```

## 6.3 El contrato: dos relojes, tres puntos de encuentro, cuatro reglas

```text
   R-RITMO-1  NADIE ESPERA SIN PLAZO. Quien espera al otro usa WAIT con plazo
              (ver PLAN_EL_PLAZO). Un participante colgado se acusa; no para
              al resto
   R-RITMO-2  LA COLA TIENE UNA K DECLARADA. Ni infinita (latencia sin techo)
              ni implicita. Quien produce la declara; quien consume la cumple
   R-RITMO-3  LO QUE VIAJA ESTA CONGELADO. Un fotograma publicado no se escribe
              hasta que el consumidor lo suelte -- o se escribe OTRO
   R-RITMO-4  EL MAS LENTO SE NOMBRA. Toda tuberia publica cuanto espero cada
              reloj al otro, y quien marca el ritmo sale en CABINA
```

*** R-RITMO-3 es donde **INTI paga lo que la seccion 2 prometia**: el valor
CONGELADO de INTI (el `IMMORTAL` de `bmo_abi::dynobj`) es justo "esto ya no se
toca". En C, que un fotograma en vuelo no se escriba es disciplina; en INTI se
puede hacer que no compile. Y el testigo de vuelta es otra SECUENCIA, en la
direccion contraria: una escritura de 32 bits que el otro lee con un `mov`, sin
puerta -- el mismo patron que el buzon.

## 6.4 El nuevo orden, y los tres primeros escalones NO necesitan GPU

```text
   E0  MEDIR los dos relojes que ya hay       app contra DIRECTOR: fotogramas
                                              publicados, presentados y
                                              PERDIDOS, y cual espero a cual.
                                              Sin tocar el formato
   E1  el TESTIGO DE VUELTA                   el DIRECTOR escribe "presente la
                                              secuencia N" en la superficie.
                                              La app mide su latencia y puede
                                              ESPERAR con WAIT, sin girar
   E2  la COLA con K                          K anillos de superficie en vez de
                                              uno; el reproductor de video es
                                              su primer cliente de verdad
   E3  la GPU como TERCER participante        el anillo PM4 y el timbre de la
                                              seccion 4, con los MISMOS tres
                                              puntos de encuentro
```

★★ **E0, E1 y E2 se hacen con la maquina que hay y se prueban con DOOM y el
cubo**, que ya son productores reales. Y cuando llegue E3, la tarjeta no trae un
modelo nuevo de sincronizacion: trae un participante mas en uno que ya se midio.

[!] Lo que esto NO cambia: sigue sin escribirse codigo de GPU (seccion 5), la
unidad sigue siendo la parte y no el registro, y la GPU sigue siendo el cuarto
aparato del NEUTRO. Lo que cambia es que **el plan deja de ser solo "como se le
manda trabajo" y pasa a ser tambien "a que ritmo, y quien espera a quien"**.

  > Independientes en el trabajo, dependientes en tres sitios con nombre. Todo
  > lo que no sea uno de esos tres sitios es un cerrojo que nadie pidio.
