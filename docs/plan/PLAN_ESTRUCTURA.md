# PLAN DE ESTRUCTURA -- el taller de BMO-X, en F1

> **Lo que afirma**: que se pulsa `F1` en el escritorio, se abre una ventana con
> historial, se teclea `compilar hola.ada`, y aparece un `.bex` en el disco de
> BMO-X. Sin instalar nada, porque no hay nada que instalar.
>
> **Como se cae**: la ventana no abre, o abre y no recibe teclas, o compila y el
> `.bex` que escribe no lo admite el cargador.
>
> Escrito el **2026-09-06**, cuando el dueno pidio *"un terminal que se convierta
> compilador, que llames como F1 en Escritorio, pero que sea APP"*.

---

## 0. EL REPARTO DEL NOMBRE, PARA QUE NO SE CONFUNDA NUNCA

```text
   VALKYRIE-ABI   JUZGA      no ejecuta jamas. No tiene anillo
   ESTRUCTURA     FABRICA    un `.bex` de Ring 3, y apunta a V-ABI
```

** No son dos capas de lo mismo: son el estandar y una herramienta que lo
cumple. `gcc` apunta a POSIX y no es POSIX. Si algun dia ESTRUCTURA se
llamara VALKYRIE, la frase de `VALKYRIE-ABI/README.md` --*"no tiene anillo
porque no tiene ni una instruccion en la maquina de destino"*-- seria falsa el
mismo dia.

---

## 1. LO QUE YA ESTA, Y ES MAS DE LO QUE PARECE

`F1` **esta libre**. En `Ultra_userspace/services/director/src/desktop/keys/app.rs`
la constante `SC_F1 = 0x3B` existe solo como frontera del rango reservado
(`SC_F1..=SC_F10`), y **nada la usa**. Las teclas de funcion las retiene el
escritorio y no bajan a la app de delante, asi que atar `F1` es una linea en el
sitio donde ya se deciden `F11` y `F12`.

Y REX ya trae las seis piezas que un terminal necesita:

```text
   superficie.h   172   dibujar en TU memoria y ofrecerla al DIRECTOR
   entrada.h      372   teclado y raton por buzon
   scroll.h       140   una ventana que se mueve sobre un historial
   archivo.h       65   leer el fuente, escribir el .bex
   paquete.h      261   las tablas que viajan DENTRO del propio .bex
   monton.h       109   malloc sobre un bloque del kernel
```

★ `scroll.h` merece decirse aparte: es **scrollback de terminal**, escrito como
funciones puras sin heap ni buffer escondido. Se prueba entero sin encender la
maquina.

---

## 2. ** LA FRONTERA DECIDE QUE COMANDOS EXISTEN, Y NO ES NEGOCIABLE

El terminal de hoy **no es una app**: son 5.761 lineas en
`Ultra_userspace/services/director/src/commands/` mas 978 de
`scene/consola.rs`, todo dentro del DIRECTOR. Y cuando se mira que hacen:

```text
   reports.rs  1.199    system.rs  908    disco.rs  420    red.rs  309
```

son en su mayoria **lectores de instrumentos del kernel**. Y
`VALKYRIE-ABI/FRONTERA.txt` deja fuera de la superficie de app, por prefijo:

```text
   DISCO_  ES_  CABINA_  USB_  AUTOPSIA_  KLOG_  SYSCALL_  MAQ_
```

> **Asi que ESTRUCTURA no puede correr `cpu`, `mem`, `disco`, `red` ni
> `cabina`.** No por falta de trabajo: **por diseno**, y el diseno esta escrito.

### La consecuencia, que es buena: son DOS terminales, no uno

```text
   la consola del DIRECTOR   el panel de INSTRUMENTOS   cpu, mem, red, cabina
   ESTRUCTURA (F1)           el TALLER                  compilar, leer, escribir
```

Y eso **no es una limitacion que rodear**: es el primer caso en el que la
frontera se cobra, y contesta sola una pregunta de diseno que si no habria que
discutir. La consola de instrumentos se queda donde esta porque **es un
instrumento**; el taller sale porque es una app.

---

## 3. ⚠ EL CELO OBLIGA A QUE SEA UN SOLO FICHERO

`EJECUTAR` pide autoridad, se fija al nacer y solo desde Ring 0
(`Ultra_kernel_x86-64/kernel/src/ring0/task/autoridad.rs`). Un `.bex` **no puede
lanzar otro**.

Eso descarta el diseno obvio --un terminal que invoca al compilador-- y deja el
correcto:

```text
   estructura.bex   el terminal Y el compilador, en el MISMO fichero,
                    con sus tablas en la seccion 0x0B
   el ESCRITORIO    lanza lo que ESTRUCTURA compilo, cuando el dueno hace clic
```

** Y eso convierte "sin instalar" en algo literal en vez de en un eslogan: **un
fichero que trae dentro lo que necesita**, y que se lee con `paquete.h` sin
copiar nada. La cabecera ya cita al dueno diciendo la idea:

> *"es un bef pero ese bex es el mismo que abre la caja: no lo duplica, lo lee y
> punto."*

---

## 4. ★★ ESTRUCTURA Y EL AUTOHOSPEDAJE SON EL MISMO TRABAJO

El frontend de Ada es **Rust**. Si ESTRUCTURA tiene que contenerlo, ESTRUCTURA es
un `.bex` de Rust -- y ahi se topa con lo mismo que
[`PLAN_AUTOHOSPEDAJE.md`](PLAN_AUTOHOSPEDAJE.md) seccion 2b ya midio:

```text
   REX (C)   archivo bloque bmo entrada monton musica pantalla
             paquete prestado scroll sonido superficie          12
   bmo-rt    syscall heap string fmt crt0 ffi                    6
```

La cara de C tiene las seis piezas del terminal. **La de Rust tiene cero.**

| lo que necesita ESTRUCTURA | REX (C) | `bmo-rt` (Rust) |
|---|---|---|
| `archivo` -- leer el fuente, escribir el `.bex` | ✅ | ❌ |
| `paquete` -- las tablas dentro del fichero | ✅ | ❌ |
| `superficie` -- dibujar en su memoria | ✅ | ❌ |
| `entrada` -- teclas y raton por buzon | ✅ | ❌ |
| `scroll` -- el historial | ✅ | ❌ |
| `monton` | ✅ | ✅ `heap` |

> **El escalon 3 del autohospedaje deja de ser de un plan y pasa a ser de los
> dos.** `bmo-rt` necesitaba `archivo` y `paquete` para compilar a bordo;
> necesita `superficie`, `entrada` y `scroll` ademas para tener ventana. Cinco
> modulos, y ninguno es investigacion: cada uno tiene su gemelo en C, escrito y
> probado, del que copiar la forma.

---

## 5. LOS ESCALONES

Ordenados por la regla de la casa: **lo que no toca nada va primero.**

```text
   [ ] 1  F1 abre una ventana VACIA   en `Ultra_userspace/services/director/
                                      src/desktop/keys/app.rs`, donde ya se
                                      deciden F11 y F12. Sin compilador y sin
                                      terminal: solo que la tecla llegue

   [ ] 2  bmo-rt gana la SUPERFICIE   `archivo` y `paquete` (los pide tambien
                                      PLAN_AUTOHOSPEDAJE), mas `superficie`,
                                      `entrada` y `scroll`. Cinco modulos en
                                      `toolchain/lang/base/bmo-rt/src`, cada
                                      uno con su gemelo en C del que copiar

   [ ] 3  estructura.bex DIBUJA       una ventana con su rejilla y su cursor,
                                      sin leer una tecla. Se compara contra
                                      `scene/consola.rs`, que ya lo hace

   [ ] 4  y LEE TECLAS                por el buzon de `entrada`, con el
                                      historial de `scroll`. Ya es un terminal,
                                      y todavia no compila nada

   [ ] 5  los comandos que la         `ls`, `cat`, `escribir`. Y los que la
          FRONTERA permite            frontera deja fuera SE DICEN, con el
                                      motivo: "eso es un instrumento, esta en
                                      la consola del DIRECTOR"

   [ ] 6  `compilar hola.ada`         el frontend de Ada dentro del mismo
                                      .bex, con sus tablas en la seccion 0x0B.
                                      Es el escalon 6 de PLAN_AUTOHOSPEDAJE
                                      visto desde aqui

   [ ] 7  exportar donde le digan     a ESTRATOS o a FAT32, y que la ventana
                                      de datos lo ensene
```

### Como se cae cada uno

| escalon | si esta bien | si falla |
|---|---|---|
| 1 | aparece un rectangulo al pulsar F1 | la tecla no llega: se la come el rango reservado |
| 2 | `cargo test` de `bmo-rt` cubre los cinco | un modulo compila y no hace lo que su gemelo en C |
| 3 | la rejilla se ve igual que la del DIRECTOR | el DIRECTOR no compone su superficie |
| 4 | se teclea y sale, y la rueda sube | el buzon se llena y se pierden teclas |
| 5 | `cpu` contesta **por que** no esta | contesta "no existe", que es una respuesta peor |
| 6 | sale un `.bex` que `bmo-verify` admite | se queda sin monton, o el `.bex` sale distinto |
| 7 | el fichero aparece en la ventana de datos | -- |

★ El escalon 5 no es de relleno. *"Ese comando es un instrumento y vive en la
consola del DIRECTOR"* es una respuesta que ensena el sistema; *"comando
desconocido"* deja al que lo teclea creyendo que falta trabajo.

---

## 6. LO QUE **NO** ENTRA, Y SE DICE PARA QUE NO CREZCA SOLO

* **Un editor.** ESTRUCTURA compila lo que hay en el disco. Editar es otra app
  y otro plan.
* **Los comandos de instrumentos.** Seccion 2. Si algun dia uno hace falta de
  verdad, se borra su prefijo de `VALKYRIE-ABI/FRONTERA.txt` **y se escribe por
  que** -- que es el momento en que la decision se toma a la vista.
* **Lanzar lo que compila.** Seccion 3. Eso es del escritorio, y es el celo.
* **Los otros tres lenguajes.** Ada primero por lo que mide
  `PLAN_AUTOHOSPEDAJE` seccion 1; C, COBOL e INTI arrastran `toml`.

---

## 7. LO QUE SERIA UN ERROR

* **Sacar los 5.761 de `commands/` del DIRECTOR para "reaprovecharlos".** La
  mayoria no puede cruzar la frontera, asi que lo que se moveria es codigo que
  despues hay que devolver.
* **Darle autoridad a ESTRUCTURA** para que lance lo que compila. Es el tercer
  bit, y `autoridad.rs` ya dejo escrita la pregunta que va antes.
* **Llamarlo VALKYRIE.** Seccion 0.
* **Empezar por el escalon 6** porque es el que se ve. Sin el 2, no hay ventana
  que lo ensene.

---

Ver [`PLAN_AUTOHOSPEDAJE.md`](PLAN_AUTOHOSPEDAJE.md) (el mismo trabajo desde el
otro lado, y la medida que elige Ada),
[`VALKYRIE-ABI/FRONTERA.txt`](../../VALKYRIE-ABI/FRONTERA.txt) (los ocho
prefijos que deciden la seccion 2),
[`EL_ORQUESTAL.md`](../identidad/EL_ORQUESTAL.md) (por que la autoridad no
viaja) y [`PLAN_REX.md`](PLAN_REX.md) (las cabeceras de las que se copia la
forma).
