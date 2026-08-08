# LIENZO -- como una app tiene su ventana sin robar la pantalla

---

# ✅ ESTADO AL 2026-08-07 -- LEASE ESTO PRIMERO

El diseno de abajo se **construyo, y por el camino cambio dos veces por
preguntas del dueno**. Las dos veces salio mas pequeno. Lo que hay hoy en el
codigo es esto:

## Lo que esta HECHO y compila

**En el kernel** (`ring0/obj/prestamo.rs`), y **no sabe que es un lienzo**:

```
MEM_OP_OFRECER (0x03 sobre KIND_MEMORIA)   presto un trozo de MI bloque a un tid
TASK_OP_TOMAR  (0x1C)                      tomo lo que me ofrecieron
KIND_PRESTADO  (0x51)                      lo prestado != lo propio
```

**En userland** (`bmo::ofrecer` / `bmo::tomar_prestado`).

## Los dos giros, y por que importan mas que el codigo

**Giro 1 -- *"por que copiar? Que sea un reflejo."*** Copiar no era una
ineficiencia heredada: **en un escritorio con ventanas que se solapan, la copia
ES la composicion** -- decide quien tapa a quien e impide que una app pinte fuera
de lo suyo. Pero para una ventana **de ancho completo** las filas si son
contiguas y el reflejo es exacto. De ahi los dos modos: *reflejo* (uno, a
pantalla completa, cero copias) y *ventana* (varios, con copia).

**Giro 2 -- *"Ring 3 no puede administrar eso el?"*** La primera version metia
`KIND_LIENZO` y dos operaciones de escritorio **dentro del kernel**. Eso era
ensenarle a Ring 0 un concepto que no es suyo. Ahora el kernel solo presta
memoria y **no sabe para que**: quien, cuanto y cuando lo decide el compositor.
Es el patron de **seL4**, y de regalo sirve para audio, captura de video y
bloques grandes entre procesos.

> **El kernel salio mas pequeno que antes de empezar: -309 lineas, +265.**

## Lo que FALTA, y es todo de Ring 3

1. El compositor **ofrece** la parte de abajo de su lienzo al tid que lanza.
2. El compositor **no limpia** esa zona mientras este prestada.
3. El raycaster **toma** en vez de reclamar la pantalla (cuatro lineas).

Ninguna de las tres toca memoria virtual. Es la parte tranquila.

## Decisiones ya cerradas, para no volver a discutirlas

- **El formato lo declara la app**, no lo fija el kernel -- DOOM pinta en 8 bits
  con paleta.
- **Se ofrece y se toma**, no se empuja: mapear en el espacio de otro exigiria
  el `CR3` de un proceso que no esta corriendo, y esa infraestructura no existe.
- **`vmm::unmap_page` devuelve el marco y NO lo libera.** Es lo que hace segura
  la devolucion: los marcos son del que presto, y liberarlos entregaria su
  memoria a un tercero.
- **`KIND_PRESTADO` != `KIND_MEMORIA`** aunque las dos sean memoria: al morir,
  una se libera y la otra solo se desmapea.

---


> Escrito el **2026-08-07**, antes de tocar una linea de codigo. Idea del dueno:
> *"el `gui.bex` es el principal de escritorio, ahi se queda; otro que sea el
> reflejo de las apps para que convivan en pantalla, como en Windows abro Dota 2
> y me sale su ventana"*.
>
> La idea es correcta y es la que su propia hoja de ruta llevaba apuntada como
> desbloqueo. Este documento la corrige en una cosa --que ahorra un programa
> entero--, la mide contra lo que hay hoy, y dice que **no** se va a construir.

---

# ★ 1. EL PROBLEMA, EN UNA LINEA

**La pantalla tiene un solo dueno.** `TASK_OP_FRAMEBUFFER_CLAIM` la entrega
entera, y `gui.bex` la reclama al arrancar. Cualquier otro programa que la pida
recibe un no -- es lo que le paso al raycaster el 07-08.

Y hay un dato que agrava esto y que conviene tener delante desde el principio:

> ⚠ **No existe forma de SOLTAR la pantalla.** Se recupera sola cuando el
> proceso muere (`cap::revoke_all`), y no hay operacion para devolverla en vida.
> O sea que hoy no hay "turnos": mientras el escritorio viva, es suya.

---

# ★★ 2. LA CORRECCION QUE AHORRA UN PROGRAMA

La propuesta era *"otro `.bex` que refleje las apps"*. **Ese programa no debe
existir**, y el motivo es el de siempre en este proyecto.

Un intermediario que copia pixeles de un sitio a otro es **un cerebro**: un
proceso en medio que hay que arrancar, vigilar, matar y depurar, y que no aporta
ninguna decision propia. Lo que hace falta es que una app pueda **decir** *"esta
memoria es mi ventana"*.

> **Eso es un CONTRATO, no un proceso.** Contratos y formatos, nunca cerebros.

El nombre del contrato es **lienzo**: quien pinta es dueno del lienzo, y el
marco lo pone otro. Ese es exactamente el reparto.

---

# ★ 3. LAS TRES PIEZAS, Y QUE HAY DE CADA UNA

| | Que es | Estado |
|---|---|---|
| **La superficie** | un bloque de memoria del que el kernel sabe base y tamano | ✅ **existe** |
| **El registro** | como la app le dice al compositor que ese bloque es su ventana | ❌ hay que hacerlo |
| **El volcado** | como los pixeles llegan a la pantalla | ❌ hay que hacerlo |

## 3.1 - La superficie ya existe, y ya esta probada

`TASK_OP_MEMORIA_PEDIR` entrega un bloque **cuyos limites el kernel tiene
apuntados**. Esa es la propiedad que lo hace todo posible, y no es teoria: es
exactamente el mecanismo que hizo posible `fread` esta misma semana --
`ARCH_OP_LEER_EN` no valida punteros, valida **lo que el kernel concedio**.

**Los numeros medidos**, que deciden el diseno:

```
  MAX_BYTES        64 MiB por bloque
  MAX_PETICIONES    4 bloques por proceso
  MAX_PROCS        16 procesos con memoria
```

Una ventana de 1920x1080 en 32 bits son **8,3 MiB**. Caben siete en un bloque, y
un proceso puede pedir cuatro bloques. **El techo no es la memoria.**

## 3.2 - El registro: y aqui hay que corregir la primera intuicion

Lo natural seria un canal -- `CHANNEL` y `ENDPOINT` estan en el ABI desde el
principio. **Y seria un error usarlos**, por un motivo que solo se ve mirando el
codigo:

> **El compositor no usa canales. No usa ninguno.** Lo unico que se llama
> "canal" en `gui.bex` es el orden de los colores (RGB/BGR).

Meter IPC ahora significaria estrenar en el compositor un mecanismo entero
--colas, secuencias, despertar consumidores-- para transportar **cuatro numeros**:
quien eres, donde esta tu lienzo, y cuanto mide.

★ **La forma que encaja con lo que ya hay es la del `klog`**: el kernel guarda
una tabla y el compositor **pregunta**. Es literalmente lo que ya hace sesenta
veces por segundo para la entrada, para la salida de los hijos y para el cursor
de ESTRATOS. *El kernel contesta preguntas y no concede nada.*

```
  la app      LIENZO_REGISTRAR(cap_bloque, ancho, alto)   -> id de lienzo
  el kernel   apunta (pid, bloque, ancho, alto) en una tabla de 16
  el compositor  LIENZO_CUANTOS / LIENZO_INFO(i)          -> pregunta, como el klog
```

Sin colas, sin secuencias, sin despertar a nadie. Una tabla y dos preguntas.

## 3.3 - El volcado: la unica pieza cara

El compositor es Ring 3: **no puede leer la memoria de otro proceso**. Hay dos
caminos y hay que elegir a sabiendas.

### Camino A -- el kernel copia *(el que propone este documento)*

```
  LIENZO_VOLCAR(id, x, y)   el kernel copia el lienzo dentro de la
                            ventana del compositor
```

- **Una llamada al sistema por ventana y por fotograma.** No por pixel.
- El kernel ya sabe llegar a las dos memorias: por el physmap alcanza cualquier
  direccion fisica, y esa es exactamente la via que usan `disk.rs` y `timer.rs`.
- La app pinta en su RAM **sin pedir permiso a nadie**, que es el 99 % del
  trabajo.

### Camino B -- el mismo bloque, dos procesos

Mapear el bloque de la app tambien en el espacio del compositor. Cero copias:
el compositor lee directamente.

⚠ Y por eso mismo es el peligroso: **dos procesos escribiendo la misma memoria
sin nada que los ordene**. Aparece el problema del *tearing* --el compositor
leyendo un fotograma a medio pintar-- y con el la necesidad de doble bufer y de
sincronizacion entre procesos. Es mas rapido y es **otro proyecto**.

### Camino C -- **REFLEJO**: la app pinta donde se va a ver

Pregunta del dueno, y es la buena: *"por que copiar? Que sea un reflejo. Copiar
era el sistema de antes, no el de BMO-X"*.

La idea: el bloque de la app **no es una copia de su ventana, ES su ventana** --
un trozo del lienzo del compositor, mapeado en el espacio de la app. La app
escribe ahi y ya esta en la pantalla. Cero copias.

★ **Y funciona. Para UN caso, y ese caso importa.**

Hay un detalle de hardware que lo decide, y no se ve hasta que lo dibujas:

```
  una ventana es un RECTANGULO dentro de un bufer mas ancho

  fila n     [-------|=== ventana ===|-------]
  fila n+1   [-------|=== ventana ===|-------]
             +- 7680 bytes de distancia entre una fila y la siguiente
```

Las filas de la ventana **no son contiguas en memoria**, y la unidad con la que
el kernel puede repartir memoria es **la pagina de 4 KiB**. Un rectangulo de
enmedio no esta alineado a pagina: para dar acceso a la ventana habria que dar
acceso a **bandas horizontales enteras** -- o sea, a los pixeles de los vecinos.

> **Una app podria pintar encima de la ventana de al lado. Y no por malicia: por
> un indice mal calculado.**

Salvo en un caso: **si la ventana ocupa el ancho completo**, las filas SI son
contiguas, la region si es un bloque de paginas, y el reflejo es exacto y
seguro. Pantalla completa, o una banda de arriba abajo.

★★ Y aqui esta lo que la pregunta destapa, que es mas importante que la
respuesta:

> **En un escritorio con ventanas que se solapan, LA COPIA ES LA COMPOSICION.**
> No es una ineficiencia heredada: es el mecanismo que decide **quien tapa a
> quien** y el que impide que una app pinte fuera de lo suyo. Quitar la copia no
> ahorra trabajo -- **quita el aislamiento**.

⚠ Y el dato historico va justo al reves de lo que parece: **el modelo sin copia
es el ANTIGUO**. En X11 los clientes dibujaban directamente sobre la pantalla
compartida, y Wayland se invento para dejar de hacerlo -- precisamente por el
aislamiento y por el *tearing*. Lo que hoy si es moderno y si evita la copia son
los **planos de superposicion de la GPU**, que leen directamente del bufer del
cliente... y eso es hardware, no arquitectura.

## La decision: los dos, y en este orden

| | Cuando | Copias |
|---|---|---|
| **Reflejo** | ventana a pantalla completa o de ancho completo | **cero** |
| **Ventana** | cualquier rectangulo, solapado, con z-order | una por fotograma |

**El reflejo se hace primero**, y no por ser el rapido: porque es **el mas
simple** --no hay recorte, no hay z-order, no hay nada que decidir-- y porque es
justo lo que necesita el primer inquilino de verdad. **DOOM va a pantalla
completa.** El raycaster tambien.

> **Decision: reflejo primero, ventana despues.** Y el camino B --bloque
> compartido para un rectangulo cualquiera-- se descarta con motivo: da acceso a
> los pixeles del vecino y trae *tearing*, doble bufer y sincronizacion entre
> procesos. No es mas rapido: es otro proyecto con otros problemas.

---

# ★★ 4. EL NUMERO INCOMODO, Y ADONDE LLEVA

Copiar 1920x1080x4 son **8,3 MB por ventana y por fotograma**. A 60 fps, medio
gigabyte por segundo. Y el `memcpy` que hay hoy **copia de byte en byte** -- su
propia cabecera lo dice: *"para mover el framebuffer de DOOM eso se va a notar,
y cuando se note se cambia por copias de 8 bytes con cola, medido primero"*.

Tres respuestas, en orden de coste:

1. **Copiar solo lo sucio.** El compositor ya lleva una caja envolvente de lo
   escrito (`Pantalla::sucio`). Una app que cambia un cuarto de su ventana copia
   un cuarto. Barato y probablemente suficiente.
2. **Copias de 8 bytes.** Esta anotado en `memoria.rs` como pendiente y
   medido-primero. Ocho veces menos vueltas.
3. **★ SDMA.** Y aqui se cierra un circulo: **esto es exactamente la "meta A" de
   `PLAN_VULKAN.md`** -- usar el motor de copia de la GPU para mover rectangulos.
   No para juegos: para esto. Del tamano del driver de AHCI, no del proyecto.

O sea que el lienzo no solo no compite con el plan de la GPU: **le da su primer
motivo real**.

---

# ★ 5. QUIEN ES DUENO DE QUE, Y QUE PASA CUANDO ALGO MUERE

Esto hay que decidirlo antes de escribir, porque es donde se rompen los sistemas
de ventanas:

- **El lienzo es de la app.** Si la app muere, `cap::revoke_all` ya le quita el
  bloque -- la maquinaria existe y se probo el 07-08, cuando el compositor
  panico y el kernel recupero la pantalla solo.
- **El marco es del compositor.** Posicion, tamano, z-order, el boton de cerrar.
  La app no decide donde esta su ventana, igual que en cualquier escritorio
  serio.
- **Una app que muere deja su entrada en la tabla como muerta**, y el
  compositor la borra al preguntar. No hay que avisarle: se entera solo.

⚠ **Y lo que NO se va a hacer**: que la app pueda pedir foco, moverse sola, o
ponerse encima. Eso es politica de escritorio y vive en el compositor. Una app
que puede ponerse encima de las demas es un anuncio emergente.

---

# ★★ 6. EL PRIMER PASO, Y ES UNO SOLO

> **Una app. Un lienzo. Un marco. Sin z-order, sin foco, sin redimensionar.**

Si eso se ve en pantalla, lo demas es repetir una fila de una tabla. Si no se ve,
el fallo esta en tres operaciones y no en un sistema de ventanas entero.

El programa de prueba se escribe solo: el **raycaster** ya existe, ya pinta, y
hoy no arranca exactamente por esto. Cambiarle `PANTALLA_RECLAMAR` por
`LIENZO_REGISTRAR` son cuatro lineas.

Y el payoff esta a la vista: **DOOM tendria una ventana en vez de robar la
pantalla.**

---

# ★ 7. LO QUE HAY QUE DECIDIR ANTES DE TECLEAR

Cuatro preguntas abiertas. Ninguna es tecnica: las cuatro son de contrato, y por
eso van aqui y no en el codigo.

1. **Cuantos lienzos por proceso?** Uno es suficiente para todo lo que existe
   hoy y hace la tabla trivial. Mas de uno es para apps con varias ventanas, y
   eso no hay ninguna.
2. **El formato lo fija el kernel o lo declara la app?** Fijarlo (XRGB de 32
   bits, como el framebuffer) evita un conversor. Declararlo abre la puerta a
   una app que pinte en 8 bits con paleta -- **que es justo lo que hace DOOM**.
3. **El tamano es fijo al registrar, o puede cambiar?** Fijo es una tabla;
   variable es renegociar el bloque en caliente, con la app pintando.
4. **Cuantas operaciones nuevas se aceptan?** Este documento propone **tres**:
   registrar, preguntar, volcar. Es la primera vez en toda la semana que se le
   anaden operaciones permanentes a la superficie, y conviene que sean pocas y
   que cada una se gane el sitio.

---

# El resumen en una frase

> **No hace falta un programa que refleje las apps: hace falta que una app pueda
> decir "esta memoria es mi ventana".** Lo que eso cuesta son tres operaciones y
> una tabla de dieciseis filas -- y lo que compra es que el escritorio deje de
> ser lo unico que puede pintar.

Ver [`SMP_MAESTRO.md`](SMP_MAESTRO.md) para el mismo metodo aplicado a los
nucleos, y `PLAN_VULKAN.md` para el motor de copia que este documento acaba de
justificar.
