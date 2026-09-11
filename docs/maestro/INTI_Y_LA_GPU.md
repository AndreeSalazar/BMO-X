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
