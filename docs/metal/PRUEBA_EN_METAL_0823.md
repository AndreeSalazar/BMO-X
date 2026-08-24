# PRUEBA EN METAL -- la tanda del 2026-08-23

> **La pregunta de la tanda: a que velocidad va el escritorio de verdad.**
>
> Nadie lo ha contado nunca, y tres sitios del DIRECTOR llevaban meses
> calibrados contra una cifra que alguien supuso -- *"~60 por segundo"*. Uno de
> los tres es el doble clic de los iconos, y por eso esta hoja empieza por DOOM:
> **si el gesto no entra, el icono no lanza nada y parece que el DIRECTOR no le
> da la pantalla.**

La regla que comparten estas hojas, y que se cumple aqui igual:

> *Cada prueba dice **que afirma** y **como se cae**. Una prueba que solo puede
> salir bien no prueba nada -- si no se sabe de antemano que aspecto tiene el
> fallo, cualquier cosa que aparezca en pantalla se lee como exito.*

Y la de orden: **lo que no toca nada va primero, lo que no se deshace va al
final.**

---

# 0 -- ⛔ ANTES DE NADA: EL BUILD NO PASA, Y NO ES POR ESTO

Medido el 2026-08-23 en el anfitrion, con el arbol limpio:

```powershell
Ultra_kernel_x86-64\build.ps1 -BuildOnly
```

```text
   => Validating source encoding          clean
   => Validating document citations       clean   819 citas
   => Validating L6a: no new module over the line
      [X] NUEVO   toolchain/lang/inti/emisor-x86_64/src/pruebas.rs      1972
      [X] NUEVO   toolchain/lang/inti/emisor-x86_64/src/lib.rs          1434
      [X] NUEVO   toolchain/lang/inti/src/ir/mod.rs                     1325
      [X] NUEVO   toolchain/lang/inti/src/disposicion/mod.rs            1235
      [X] CRECIO  toolchain/forge/bmo-lower/src/emu/mod.rs      1689 -> 1726
      [X] CRECIO  Ultra_kernel_x86-64/build.ps1                 1578 -> 1584
      [X] CRECIO  platform/abi/bmo-abi/src/bef/validator.rs     1416 -> 1435
   [X] L6a: un modulo nuevo pasa de las 1.000 lineas, o uno de la linea base crecio
```

★★★ **El guardian de MODULAR corta el build, o sea que hoy no sale imagen.** No
hay `.efi`, no hay `d.bex` nuevo y no hay `apps\doom.bex`: **ninguna de las
pruebas de abajo se puede hacer hasta que esto se resuelva.**

Y no se resuelve solo. Son dos caminos y **la decision es del dueno**, porque
los siete son de INTI y sellar una infraccion nueva es aceptar deuda:

```text
   PARTIR     los cuatro ficheros nuevos.  Los tres CAJON son mecanicos
              (media ~30 lineas por funcion: mover texto, demostrable byte a
              byte, L6d).  `emisor-x86_64/src/lib.rs` es `mixto` y va a mano.
   SELLAR     anotarlos en LINEA_BASE.txt con su motivo, como se hizo con
              `syscall/mod.rs` el 19-08 -- pero eso son CUATRO ficheros nuevos
              por encima de la linea, no un techo levantado de 14 lineas.
```

[!] Lo que **si** quedo arreglado el 23-08 y era el otro portico cerrado: el
guardian de ASCII paraba el build por **cinco caracteres** --una `i` con tilde y
tres enes en `INTI_MAESTRO.md`, y un signo de seccion en
`toolchain/lang/inti/ESTADO.md`--. Ese ya pasa.

---

# 1 -- LA CIFRA QUE NADIE HA VISTO: `escritorio` en F7

**Lo primero porque no toca nada**: se abre una ventana, se mira un numero y se
cierra. Y porque es la que explica las tres de abajo.

```text
   F7  ->  ventana CPU
           mide            ...
           va a            ... GHz de ... base
           gasta           ... W el paquete entero
           este nucleo     ...
           nucleos         ...
           planificador    N tareas, M listas
           escritorio      ????  vueltas/s   del bucle, no fotogramas   <- ESTA
```

## Que afirma

Que el bucle principal del DIRECTOR da **N vueltas por segundo**, contadas con
`rdtsc` contra `INFO_TSC_HZ` -- no estimadas. Es la primera medida de este
numero en toda la vida del proyecto.

## Como se cae, y que significa cada respuesta

| Sale | Que significa |
|---|---|
| **un numero de tres o cuatro cifras** (~1.000-50.000) | lo previsto. El bucle no tiene freno y una vuelta muda son unas pocas puertas. **Confirma que las 24 vueltas del doble clic viejo eran milisegundos**, no 400 |
| **algo cerca de 60** | la prediccion era FALSA y el doble clic viejo estaba bien calibrado. Entonces el icono no lanzaba por otro motivo, y hay que volver a mirar |
| `-- (aun sin un segundo entero)` **y se queda ahi** | `Tick::pulse` no esta corriendo, o `INFO_TSC_HZ` no llega. La ventana lleva mas de un segundo abierta: si no ha cerrado un segundo, algo no cuenta |
| `-- (sin reloj de referencia)` | `INFO_TSC_HZ` contesta 0. Eso rompe ademas `lend_screen`, asi que seria una noticia mucho mas grande que esta fila |

★ Y de paso se contesta una segunda cosa **sin hacer nada mas**: las filas
`gasta` y `este nucleo` son **diferencias entre dos lecturas**, y hasta hoy se
refrescaban cada 15 vueltas creyendo que eran 250 ms. Si el bucle va rapido,
antes temblaban. **Ahora se refrescan por reloj**: si los vatios se leen
QUIETOS, esa es la prueba de que el cuarto de segundo llego de verdad.

---

# 2 -- EL ICONO DE DOOM: **DOS** CLICS, NO UNO

## Lo que cambio el 19-08 y nadie volvio a la hoja

`cc5e6922` --*"un clic ya SENALA, no lanza"*-- convirtio el lanzamiento en doble
clic. La hoja pendiente desde el 14-08 dice *"arrancar, **CLIC** en el icono de
DOOM"*, y esa instruccion lleva cinco dias sin ser cierta. **Un clic solo escribe
`run apps/doom.bex` en la caja de Ejecutar y la deja preparada.**

## Que afirma

Que el gesto de abrir entra: dos clics sobre el icono, **a menos de 400 ms**,
lanzan DOOM y el escritorio le presta pantalla y entrada.

## Como se cae

| Sintoma | Que significa |
|---|---|
| **arranca DOOM** | el gesto entra y `lend_screen` hace su trabajo. Sigue en el punto 5 |
| el icono se realza y **no pasa nada**, por rapido que se pulse | el gesto sigue sin entrar. El arreglo de ciclos no basto: mirar `escritorio` en F7 y `DoubleClick::window()` |
| queda `run apps/doom.bex` escrito y **no lanza** | es lo que hace UN clic. Fueron dos clics que el gesto no unio, o el segundo cayo en otra celda |
| `DOOM: no hay pantalla (la tiene otro proceso)` | el gesto SI entro y el fallo es del prestamo. Es otro sitio: `wants_screen` / `lend_screen` en `main.rs` |
| se lanza pero al morir se cae al **panel del kernel** | `lend_screen` no recupero. La fila que interesa es la de "pantalla devuelta" |

★ **La salida a mano que separa las dos mitades, y no hace falta el raton**:
teclear `run apps/doom.bex` y ENTER en la caja de Ejecutar. Ese camino **no pasa
por el gesto**. Si por ahi arranca y por el icono no, el fallo es el doble clic;
si por ahi tampoco, el fallo es el prestamo de pantalla y el gesto es inocente.

[!] Comprobado en el anfitrion el 23-08, para que no se busque donde ya se
miro: `doom.bex` compila hoy (**880.250 B**) y su cabecera lleva la bandera --
`BEF1`, flags `0x00000701`, **bit 10 `WANTS_SCREEN` puesto**--. La bandera llega.

---

# 3 -- LA REJILLA DE ESTRATOS: EL MISMO GESTO, EL MISMO ARREGLO

**F12** abre la ventana de Datos. Doble clic en una carpeta de la rejilla.

## Que afirma

Que el gesto es **uno solo** en toda la casa: las dos rejillas llaman al mismo
`scene::double_click`, y lo que se aprende en una vale en la otra.

## Como se cae

| Sintoma | Que significa |
|---|---|
| entra en la carpeta | verde. Y confirma el punto 2 desde el otro lado |
| solo se senala la fila | el gesto no entra **en ninguna de las dos**, o sea que el sospechoso es el modulo compartido y no una rejilla |
| entra en la rejilla y **no** en los iconos (o al reves) | seria raro: es el mismo codigo. Entonces lo que difiere es el llamante, no el gesto |

---

# 4 -- LA LUZ DEL BUS USB SIGUE VIVA

El testigo de la barra --E6 de `../componente/EL_TECLADO_EXIGE.md`-- espaciaba
sus dos lecturas de `OP_INFO` cada 15 vueltas creyendo que eran 250 ms. Ahora las
espacia con `Tick::quarter_cycles()`.

## Que afirma

Que la luz **sigue cambiando** cuando el bus cambia, y que no se ha quedado
mirando una sola vez.

## Como se cae

Desenchufar el teclado USB y volver a enchufarlo. **La luz tiene que cambiar en
menos de un segundo.** Si se queda congelada en el color de antes, el espaciado
salio infinito y hay que mirar `ciclos_de` -- que devuelve `0` a proposito
cuando no hay reloj, o sea *"mira siempre"*, nunca *"no mires"*.

---

# 5 -- DOOM ENTERO, y ya se sabe donde muere

**Va el ultimo porque es el largo**, y porque los cuatro de arriba se contestan
en el primer minuto.

## Que afirma

Nada nuevo del juego. **Lo que se prueba aqui es el CAMINO DE VUELTA**: que al
morir DOOM se cae en el escritorio pintado ENTERO --degradado, barra e iconos--
y no en el panel del kernel ni en un fondo liso.

## Lo que va a pasar, escrito por delante para que no se lea como sorpresa

DOOM **va a morir**, y no es culpa de esta tanda: `#PF NULO en 0+0x2c ->
R_SortVisSprites+0x2c6`. La causa esta acotada y es de **BMO C**, no del
DIRECTOR ni del kernel -- dos sondas rojas, verificadas otra vez el 23-08:

```bash
cargo test -p bmo-c-front sonda_resta_de_punteros -- --ignored
```

```text
   la_lista_circular_con_centinela_local  ->  "8 101"     esperado "8 8"
   la_resta_al_reves_sale_negativa        ->  "-679168"   esperado "-5"
```

La primera es la que mata: el recorrido de la lista circular **no vuelve nunca
al centinela**, y en DOOM no hay tope. Ver `../plan/PLAN_DOOM.md`.

## Como se cae

| Sintoma | Que significa |
|---|---|
| muere y sale **el escritorio entero** | ✅ el camino de vuelta funciona. Es todo lo que esta tanda le pide a DOOM |
| muere y sale un **fondo liso** con la caja de Ejecutar flotando | el repintado de vuelta esta a medias. Es el sintoma que el 11-08 se leyo como *"el escritorio se bugeo"* |
| muere y sale el **panel del kernel** | `lend_screen` no recupero la pantalla, o el escritorio murio con el |

---

---

# 6 -- ★★★ `ray.bex` EN UNA VENTANA, Y SE PUEDE TOCAR

**La prueba de app + ventanas entera, y no hace falta que DOOM juegue para
hacerla.** Anadida el 23-08 con el paso 2c construido.

```text
   run c/ray.bex          desde la caja de Ejecutar, o doble clic en su icono
```

Aparece en una caja con su marco y sus tres botones. Hasta hoy ahi se acababa:
se miraba y no se tocaba.

## Que afirma

Que una app en una ventana **recibe teclas**, que el escritorio **no las
pierde**, y que las dos cosas pasan a la vez. Es el modelo general de app --lo
que el dueno pidio-- y lo que separa un compositor de un relevo.

## Los cuatro gestos, en orden

```text
   1  W A S D             se anda y se gira DENTRO de la caja
   2  M                   se abre el menu: tres filas de barras
   3  flechas             arriba/abajo cambian de fila; izq/der cambian el
                          valor, y el mundo de detras cambia EN EL ACTO
                          (vision, velocidad, tema)
   4  ESC                 cierra el menu.  NO cierra la ventana: para eso
                          esta el boton del marco
   5  CLIC en una casilla del menu   la pone directamente, sin navegar
```

★★ **El gesto 5 es el del RATON, y prueba dos cosas de una vez**: que el clic
llega, y que las coordenadas se tradujeron bien. Si el valor que cambia **no es
el de la casilla que se pulso**, no es que el clic no llegue -- es que el origen
de la ventana no se resto igual al pintar que al golpear. Pulsar cerca de un
borde es donde eso se ve antes.

★ Y mientras tanto, **sin tocar la ventana**: `F7` tiene que abrir las vitales y
`Alt+Tab` tiene que conmutar. Esas son del escritorio y estan en una lista
cerrada.

## Como se cae

| Sintoma | Que significa |
|---|---|
| **se anda y el menu responde** | ✅ 2c entero. El buzon funciona entre dos procesos |
| la ventana se ve y **no reacciona a nada** | el buzon no llego. Mirar si `ray.bex` se compilo con `crear_con_buzon`, y la aduana de `Header::read` |
| reacciona pero **cada tecla cuenta dos veces** (se anda el doble, el menu salta dos filas) | se esta atendiendo tambien el SOLTAR. Es el `BMO_EVENTO_PULSADA` del bucle de entrada |
| responde la ventana **y ademas se escribe en Ejecutar** | las dos colas se estan cocinando las dos. El reparto esta en `desktop::keys::app` |
| responde la ventana y **F7 ya no abre** | la lista cerrada del escritorio no se esta respetando: una app se quedo con las F |
| el escritorio va **a tirones** mientras la ventana esta delante | no es el buzon: es el reparto de turno. `ray.bex` duerme 16 ms en ventana desde `bc3c018b` |
| el clic **mueve al personaje** en vez de pulsar la casilla | el bit 63 no se esta mirando: los botones se estan leyendo como scancode. Es el fallo que `bmo_sup_es_raton` existe para impedir |
| un clic en la **barra de titulo** llega a la app | `Table::golpe` tenia que haber contestado `None` fuera del contenido |
| se arrastra la ventana **al pulsar dentro** | el clic esta llegando a `chrome.grab` y a la app a la vez |

[!] El menu es de BARRAS y no de texto a proposito: REX no trae fuente para una
app de C. Tres filas, tres segmentos cada una, y la fila senalada con su marca a
la izquierda. **Si se ven las barras pero no se mueve la marca**, el menu se
esta pintando y las teclas no llegan -- que es un fallo distinto de no ver nada.

## Y lo que esta prueba deja abierto, para no buscarlo

```text
   el raton todavia NO viaja por el buzon      solo teclas
   el foco se le da a cualquier app            aunque no tenga buzon
   la app ve tambien los atajos que no estan   las dos colas son independientes
   en la lista cerrada
```

---

# Los arrastres, que no son de esta tanda pero se contestan en el mismo arranque

No se repiten aqui enteros: viven donde se escribieron.

| que | donde esta escrito |
|---|---|
| **TRIM**: reiniciar y comprobar que no se llevo nada (414,5 GiB) | pendientes de hardware |
| **El `.bex` con `Manifest 0x09` carga igual** -- la unica fila de P1 que no se pudo cerrar en el anfitrion | `../plan/PLAN_EL_SILICIO.md`, P1 |
| Las seis preguntas abiertas de la segunda vuelta del 12-08 | `PRUEBA_EN_METAL.md` (que es del **12-08**: el nombre no lo dice) |
