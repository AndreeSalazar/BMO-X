# LIENZO — cómo una app tiene su ventana sin robar la pantalla

> Escrito el **2026-08-07**, antes de tocar una línea de código. Idea del dueño:
> *"el `gui.bex` es el principal de escritorio, ahí se queda; otro que sea el
> reflejo de las apps para que convivan en pantalla, como en Windows abro Dota 2
> y me sale su ventana"*.
>
> La idea es correcta y es la que su propia hoja de ruta llevaba apuntada como
> desbloqueo. Este documento la corrige en una cosa —que ahorra un programa
> entero—, la mide contra lo que hay hoy, y dice qué **no** se va a construir.

---

# ★ 1. EL PROBLEMA, EN UNA LÍNEA

**La pantalla tiene un solo dueño.** `TASK_OP_FRAMEBUFFER_CLAIM` la entrega
entera, y `gui.bex` la reclama al arrancar. Cualquier otro programa que la pida
recibe un no — es lo que le pasó al raycaster el 07-08.

Y hay un dato que agrava esto y que conviene tener delante desde el principio:

> ⚠️ **No existe forma de SOLTAR la pantalla.** Se recupera sola cuando el
> proceso muere (`cap::revoke_all`), y no hay operación para devolverla en vida.
> O sea que hoy no hay "turnos": mientras el escritorio viva, es suya.

---

# ★★ 2. LA CORRECCIÓN QUE AHORRA UN PROGRAMA

La propuesta era *"otro `.bex` que refleje las apps"*. **Ese programa no debe
existir**, y el motivo es el de siempre en este proyecto.

Un intermediario que copia píxeles de un sitio a otro es **un cerebro**: un
proceso en medio que hay que arrancar, vigilar, matar y depurar, y que no aporta
ninguna decisión propia. Lo que hace falta es que una app pueda **decir** *"esta
memoria es mi ventana"*.

> **Eso es un CONTRATO, no un proceso.** Contratos y formatos, nunca cerebros.

El nombre del contrato es **lienzo**: quien pinta es dueño del lienzo, y el
marco lo pone otro. Ése es exactamente el reparto.

---

# ★ 3. LAS TRES PIEZAS, Y QUÉ HAY DE CADA UNA

| | Qué es | Estado |
|---|---|---|
| **La superficie** | un bloque de memoria del que el kernel sabe base y tamaño | ✅ **existe** |
| **El registro** | cómo la app le dice al compositor que ese bloque es su ventana | ❌ hay que hacerlo |
| **El volcado** | cómo los píxeles llegan a la pantalla | ❌ hay que hacerlo |

## 3.1 · La superficie ya existe, y ya está probada

`TASK_OP_MEMORIA_PEDIR` entrega un bloque **cuyos límites el kernel tiene
apuntados**. Ésa es la propiedad que lo hace todo posible, y no es teoría: es
exactamente el mecanismo que hizo posible `fread` esta misma semana —
`ARCH_OP_LEER_EN` no valida punteros, valida **lo que el kernel concedió**.

**Los números medidos**, que deciden el diseño:

```
  MAX_BYTES        64 MiB por bloque
  MAX_PETICIONES    4 bloques por proceso
  MAX_PROCS        16 procesos con memoria
```

Una ventana de 1920×1080 en 32 bits son **8,3 MiB**. Caben siete en un bloque, y
un proceso puede pedir cuatro bloques. **El techo no es la memoria.**

## 3.2 · El registro: y aquí hay que corregir la primera intuición

Lo natural sería un canal — `CHANNEL` y `ENDPOINT` están en el ABI desde el
principio. **Y sería un error usarlos**, por un motivo que sólo se ve mirando el
código:

> **El compositor no usa canales. No usa ninguno.** Lo único que se llama
> "canal" en `gui.bex` es el orden de los colores (RGB/BGR).

Meter IPC ahora significaría estrenar en el compositor un mecanismo entero
—colas, secuencias, despertar consumidores— para transportar **cuatro números**:
quién eres, dónde está tu lienzo, y cuánto mide.

★ **La forma que encaja con lo que ya hay es la del `klog`**: el kernel guarda
una tabla y el compositor **pregunta**. Es literalmente lo que ya hace sesenta
veces por segundo para la entrada, para la salida de los hijos y para el cursor
de ESTRATOS. *El kernel contesta preguntas y no concede nada.*

```
  la app      LIENZO_REGISTRAR(cap_bloque, ancho, alto)   → id de lienzo
  el kernel   apunta (pid, bloque, ancho, alto) en una tabla de 16
  el compositor  LIENZO_CUANTOS / LIENZO_INFO(i)          → pregunta, como el klog
```

Sin colas, sin secuencias, sin despertar a nadie. Una tabla y dos preguntas.

## 3.3 · El volcado: la única pieza cara

El compositor es Ring 3: **no puede leer la memoria de otro proceso**. Hay dos
caminos y hay que elegir a sabiendas.

### Camino A — el kernel copia *(el que propone este documento)*

```
  LIENZO_VOLCAR(id, x, y)   el kernel copia el lienzo dentro de la
                            ventana del compositor
```

- **Una llamada al sistema por ventana y por fotograma.** No por píxel.
- El kernel ya sabe llegar a las dos memorias: por el physmap alcanza cualquier
  dirección física, y ésa es exactamente la vía que usan `disk.rs` y `timer.rs`.
- La app pinta en su RAM **sin pedir permiso a nadie**, que es el 99 % del
  trabajo.

### Camino B — el mismo bloque, dos procesos

Mapear el bloque de la app también en el espacio del compositor. Cero copias:
el compositor lee directamente.

⚠️ Y por eso mismo es el peligroso: **dos procesos escribiendo la misma memoria
sin nada que los ordene**. Aparece el problema del *tearing* —el compositor
leyendo un fotograma a medio pintar— y con él la necesidad de doble búfer y de
sincronización entre procesos. Es más rápido y es **otro proyecto**.

> **Decisión: camino A.** Y no por prudencia: porque el B no se puede evaluar
> hasta que el A esté funcionando y se haya medido cuánto cuesta la copia. Ir al
> B primero es optimizar un número que nadie ha visto.

---

# ★★ 4. EL NÚMERO INCÓMODO, Y ADÓNDE LLEVA

Copiar 1920×1080×4 son **8,3 MB por ventana y por fotograma**. A 60 fps, medio
gigabyte por segundo. Y el `memcpy` que hay hoy **copia de byte en byte** — su
propia cabecera lo dice: *"para mover el framebuffer de DOOM eso se va a notar,
y cuando se note se cambia por copias de 8 bytes con cola, medido primero"*.

Tres respuestas, en orden de coste:

1. **Copiar sólo lo sucio.** El compositor ya lleva una caja envolvente de lo
   escrito (`Pantalla::sucio`). Una app que cambia un cuarto de su ventana copia
   un cuarto. Barato y probablemente suficiente.
2. **Copias de 8 bytes.** Está anotado en `memoria.rs` como pendiente y
   medido-primero. Ocho veces menos vueltas.
3. **★ SDMA.** Y aquí se cierra un círculo: **esto es exactamente la "meta A" de
   `PLAN_VULKAN.md`** — usar el motor de copia de la GPU para mover rectángulos.
   No para juegos: para esto. Del tamaño del driver de AHCI, no del proyecto.

O sea que el lienzo no sólo no compite con el plan de la GPU: **le da su primer
motivo real**.

---

# ★ 5. QUIÉN ES DUEÑO DE QUÉ, Y QUÉ PASA CUANDO ALGO MUERE

Esto hay que decidirlo antes de escribir, porque es donde se rompen los sistemas
de ventanas:

- **El lienzo es de la app.** Si la app muere, `cap::revoke_all` ya le quita el
  bloque — la maquinaria existe y se probó el 07-08, cuando el compositor
  panicó y el kernel recuperó la pantalla solo.
- **El marco es del compositor.** Posición, tamaño, z-order, el botón de cerrar.
  La app no decide dónde está su ventana, igual que en cualquier escritorio
  serio.
- **Una app que muere deja su entrada en la tabla como muerta**, y el
  compositor la borra al preguntar. No hay que avisarle: se entera solo.

⚠️ **Y lo que NO se va a hacer**: que la app pueda pedir foco, moverse sola, o
ponerse encima. Eso es política de escritorio y vive en el compositor. Una app
que puede ponerse encima de las demás es un anuncio emergente.

---

# ★★ 6. EL PRIMER PASO, Y ES UNO SOLO

> **Una app. Un lienzo. Un marco. Sin z-order, sin foco, sin redimensionar.**

Si eso se ve en pantalla, lo demás es repetir una fila de una tabla. Si no se ve,
el fallo está en tres operaciones y no en un sistema de ventanas entero.

El programa de prueba se escribe solo: el **raycaster** ya existe, ya pinta, y
hoy no arranca exactamente por esto. Cambiarle `PANTALLA_RECLAMAR` por
`LIENZO_REGISTRAR` son cuatro líneas.

Y el payoff está a la vista: **DOOM tendría una ventana en vez de robar la
pantalla.**

---

# ★ 7. LO QUE HAY QUE DECIDIR ANTES DE TECLEAR

Cuatro preguntas abiertas. Ninguna es técnica: las cuatro son de contrato, y por
eso van aquí y no en el código.

1. **¿Cuántos lienzos por proceso?** Uno es suficiente para todo lo que existe
   hoy y hace la tabla trivial. Más de uno es para apps con varias ventanas, y
   eso no hay ninguna.
2. **¿El formato lo fija el kernel o lo declara la app?** Fijarlo (XRGB de 32
   bits, como el framebuffer) evita un conversor. Declararlo abre la puerta a
   una app que pinte en 8 bits con paleta — **que es justo lo que hace DOOM**.
3. **¿El tamaño es fijo al registrar, o puede cambiar?** Fijo es una tabla;
   variable es renegociar el bloque en caliente, con la app pintando.
4. **¿Cuántas operaciones nuevas se aceptan?** Este documento propone **tres**:
   registrar, preguntar, volcar. Es la primera vez en toda la semana que se le
   añaden operaciones permanentes a la superficie, y conviene que sean pocas y
   que cada una se gane el sitio.

---

# El resumen en una frase

> **No hace falta un programa que refleje las apps: hace falta que una app pueda
> decir "esta memoria es mi ventana".** Lo que eso cuesta son tres operaciones y
> una tabla de dieciséis filas — y lo que compra es que el escritorio deje de
> ser lo único que puede pintar.

Ver [`SMP_MAESTRO.md`](SMP_MAESTRO.md) para el mismo método aplicado a los
núcleos, y `PLAN_VULKAN.md` para el motor de copia que este documento acaba de
justificar.
